use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite, Row};
use crate::models::{Connection, ConnectionForm};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

pub struct LocalDb {
    pool: Pool<Sqlite>,
}

impl LocalDb {
    pub async fn init(app_handle: &tauri::AppHandle) -> Result<Self, String> {
        let app_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        }
        let db_path = app_dir.join("quickdb.sqlite");
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
            .map_err(|e| format!("[MIGRATION_ERROR] {e}"))?;

        Ok(Self { pool })
    }

    pub async fn create_connection(&self, form: ConnectionForm) -> Result<Connection, String> {
        let uuid = uuid::Uuid::new_v4().to_string();
        let name = form.host.clone().unwrap_or_else(|| "Unknown".to_string()); // Default name logic can be improved
        
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO connections (uuid, type, name, host, port, database, username, password, ssl) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
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
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[INSERT_ERROR] {e}"))?;

        self.get_connection_by_id(id).await
    }

    pub async fn update_connection(&self, id: i64, form: ConnectionForm) -> Result<Connection, String> {
         sqlx::query(
            "UPDATE connections SET type = ?, host = ?, port = ?, database = ?, username = ?, password = ?, ssl = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(&form.driver)
        .bind(&form.host.unwrap_or_default())
        .bind(&form.port.unwrap_or(0))
        .bind(&form.database.unwrap_or_default())
        .bind(&form.username.unwrap_or_default())
        .bind(&form.password.unwrap_or_default()) // TODO: Encrypt
        .bind(form.ssl.unwrap_or(false))
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
                id, uuid, name, type as db_type, host, port, database, username, ssl, file_path, created_at, updated_at 
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
                id, uuid, name, type as db_type, host, port, database, username, ssl, file_path, created_at, updated_at 
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
            "SELECT type as db_type, host, port, database, username, password, ssl FROM connections WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        
        // Manual extraction since we don't have a struct for this specific query or macros
        Ok(ConnectionForm {
            driver: row.try_get("db_type").unwrap_or_default(),
            host: row.try_get("host").ok(),
            port: row.try_get("port").ok(),
            database: row.try_get("database").ok(),
            schema: None, // Schema is not stored in connection config usually
            username: row.try_get("username").ok(),
            password: row.try_get("password").ok(),
            ssl: row.try_get::<bool, _>("ssl").ok().map(|v| v), // bool mapping
            file_path: None,
        })
    }
}
