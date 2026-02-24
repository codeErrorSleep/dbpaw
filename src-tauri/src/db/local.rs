use crate::models::{Connection, ConnectionForm, SavedQuery};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use std::fs;
use tauri::Manager;

pub struct LocalDb {
    pool: Pool<Sqlite>,
}

impl LocalDb {
    pub async fn init(app_handle: &tauri::AppHandle) -> Result<Self, String> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        }
        let db_path = app_dir.join("dbpaw.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .map_err(|e| format!("[LOCAL_DB_INIT] {e}"))?;

        // Run migrations
        sqlx::query(include_str!("../../migrations/001_initial.sql"))
            .execute(&pool)
            .await
            .map_err(|e| format!("[MIGRATION_001_ERROR] {e}"))?;

        sqlx::query(include_str!("../../migrations/002_saved_queries.sql"))
            .execute(&pool)
            .await
            .map_err(|e| format!("[MIGRATION_002_ERROR] {e}"))?;

        // Check if database column exists in saved_queries to avoid duplicate column error
        let has_database_column: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('saved_queries') WHERE name='database')"
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("[MIGRATION_003_CHECK_ERROR] {e}"))?;

        if !has_database_column {
            sqlx::query(include_str!("../../migrations/003_add_database_to_saved_queries.sql"))
                .execute(&pool)
                .await
                .map_err(|e| format!("[MIGRATION_003_ERROR] {e}"))?;
        }

        // Check if ssh_enabled column exists in connections
        let has_ssh_column: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('connections') WHERE name='ssh_enabled')"
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("[MIGRATION_004_CHECK_ERROR] {e}"))?;

        if !has_ssh_column {
            // Split migration because sqlite doesn't support multiple ALTER TABLE in one query usually via sqlx wrapper sometimes
            // But let's try execute multiple or one by one.
            // Safe bet: run the script which has multiple statements if supported, or read line by line.
            // SQLx execute support multiple statements for sqlite? Yes usually.
            sqlx::query(include_str!("../../migrations/004_add_ssh_fields.sql"))
                .execute(&pool)
                .await
                .map_err(|e| format!("[MIGRATION_004_ERROR] {e}"))?;
        }

        Ok(Self { pool })
    }

    pub async fn create_connection(&self, form: ConnectionForm) -> Result<Connection, String> {
        let uuid = uuid::Uuid::new_v4().to_string();
        // Use provided name or fallback to host or "Unknown"
        let name = form.name.clone()
            .or_else(|| form.host.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        // Check if connection with same name already exists
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM connections WHERE name = ?)")
            .bind(&name)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("[CHECK_EXIST_ERROR] {e}"))?;
            
        if exists {
            return Err(format!("Connection with name '{}' already exists", name));
        }

        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO connections (uuid, type, name, host, port, database, username, password, ssl, ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
        )
        .bind(&uuid)
        .bind(&form.driver)
        .bind(&name)
        .bind(&form.host.unwrap_or_default())
        .bind(&form.port.unwrap_or(0))
        .bind(&form.database.unwrap_or_default())
        .bind(&form.username.unwrap_or_default())
        .bind(&form.password.unwrap_or_default()) // TODO: Encrypt password
        .bind(form.ssl.unwrap_or(false))
        .bind(form.ssh_enabled.unwrap_or(false))
        .bind(form.ssh_host)
        .bind(form.ssh_port)
        .bind(form.ssh_username)
        .bind(form.ssh_password)
        .bind(form.ssh_key_path)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[INSERT_ERROR] {e}"))?;

        self.get_connection_by_id(id).await
    }

    pub async fn update_connection(
        &self,
        id: i64,
        form: ConnectionForm,
    ) -> Result<Connection, String> {
        sqlx::query(
            "UPDATE connections SET name = COALESCE(NULLIF(?, ''), name), type = ?, host = ?, port = ?, database = ?, username = ?, password = COALESCE(NULLIF(?, ''), password), ssl = ?, ssh_enabled = ?, ssh_host = ?, ssh_port = ?, ssh_username = ?, ssh_password = ?, ssh_key_path = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(form.name)
        .bind(&form.driver)
        .bind(&form.host.unwrap_or_default())
        .bind(&form.port.unwrap_or(0))
        .bind(&form.database.unwrap_or_default())
        .bind(&form.username.unwrap_or_default())
        .bind(form.password) // TODO: Encrypt
        .bind(form.ssl.unwrap_or(false))
        .bind(form.ssh_enabled.unwrap_or(false))
        .bind(form.ssh_host)
        .bind(form.ssh_port)
        .bind(form.ssh_username)
        .bind(form.ssh_password)
        .bind(form.ssh_key_path)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("[UPDATE_ERROR] {e}"))?;

        self.get_connection_by_id(id).await
    }

    pub async fn delete_connection(&self, id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[DELETE_ERROR] {e}"))?;
        Ok(())
    }

    pub async fn list_connections(&self) -> Result<Vec<Connection>, String> {
        let rows = sqlx::query_as::<_, Connection>(
            r#"SELECT 
                id, uuid, name, type as db_type, host, port, database, username, ssl, file_path, 
                ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path,
                created_at, updated_at 
               FROM connections 
               ORDER BY updated_at DESC"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(rows)
    }

    pub async fn get_connection_by_id(&self, id: i64) -> Result<Connection, String> {
        sqlx::query_as::<_, Connection>(
            r#"SELECT 
                id, uuid, name, type as db_type, host, port, database, username, ssl, file_path, 
                ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path,
                created_at, updated_at 
               FROM connections 
               WHERE id = ?"#
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))
    }

    pub async fn get_connection_form_by_id(&self, id: i64) -> Result<ConnectionForm, String> {
        let row = sqlx::query(
            "SELECT type as db_type, name, host, port, database, username, password, ssl, ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path FROM connections WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        // Manual extraction since we don't have a struct for this specific query or macros
        Ok(ConnectionForm {
            driver: row.try_get("db_type").unwrap_or_default(),
            name: row.try_get("name").ok(),
            host: row.try_get("host").ok(),
            port: row.try_get("port").ok(),
            database: row.try_get("database").ok(),
            schema: None, // Schema is not stored in connection config usually
            username: row.try_get("username").ok(),
            password: row.try_get("password").ok(),
            ssl: row.try_get::<bool, _>("ssl").ok().map(|v| v), // bool mapping
            file_path: None,
            ssh_enabled: row.try_get::<bool, _>("ssh_enabled").ok().map(|v| v),
            ssh_host: row.try_get("ssh_host").ok(),
            ssh_port: row.try_get("ssh_port").ok(),
            ssh_username: row.try_get("ssh_username").ok(),
            ssh_password: row.try_get("ssh_password").ok(),
            ssh_key_path: row.try_get("ssh_key_path").ok(),
        })
    }

    pub async fn create_saved_query(
        &self,
        name: String,
        query: String,
        description: Option<String>,
        connection_id: Option<i64>,
        database: Option<String>,
    ) -> Result<SavedQuery, String> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO saved_queries (name, query, description, connection_id, database) VALUES (?, ?, ?, ?, ?) RETURNING id"
        )
        .bind(&name)
        .bind(&query)
        .bind(description)
        .bind(connection_id)
        .bind(database)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[CREATE_QUERY_ERROR] {e}"))?;

        self.get_saved_query_by_id(id).await
    }

    pub async fn update_saved_query(
        &self,
        id: i64,
        name: String,
        query: String,
        description: Option<String>,
        connection_id: Option<i64>,
        database: Option<String>,
    ) -> Result<SavedQuery, String> {
        sqlx::query(
            "UPDATE saved_queries SET name = ?, query = ?, description = ?, connection_id = ?, database = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(&name)
        .bind(&query)
        .bind(description)
        .bind(connection_id)
        .bind(database)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("[UPDATE_QUERY_ERROR] {e}"))?;

        self.get_saved_query_by_id(id).await
    }

    pub async fn delete_saved_query(&self, id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM saved_queries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[DELETE_QUERY_ERROR] {e}"))?;
        Ok(())
    }

    pub async fn list_saved_queries(&self) -> Result<Vec<SavedQuery>, String> {
        let rows = sqlx::query_as::<_, SavedQuery>(
            "SELECT id, name, query, description, connection_id, database, created_at, updated_at FROM saved_queries ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[LIST_QUERIES_ERROR] {e}"))?;
        Ok(rows)
    }

    pub async fn get_saved_query_by_id(&self, id: i64) -> Result<SavedQuery, String> {
        sqlx::query_as::<_, SavedQuery>(
            "SELECT id, name, query, description, connection_id, database, created_at, updated_at FROM saved_queries WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[GET_QUERY_ERROR] {e}"))
    }
}
