use crate::models::{
    AiConversation, AiMessage, AiProvider, AiProviderForm, Connection, ConnectionForm, SavedQuery,
};
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

        let has_ai_providers: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ai_providers')",
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("[MIGRATION_005_CHECK_ERROR] {e}"))?;

        if !has_ai_providers {
            sqlx::query(include_str!("../../migrations/005_ai_providers.sql"))
                .execute(&pool)
                .await
                .map_err(|e| format!("[MIGRATION_005_ERROR] {e}"))?;
        }

        let has_ai_conversations: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ai_conversations')",
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("[MIGRATION_006_CHECK_ERROR] {e}"))?;

        if !has_ai_conversations {
            sqlx::query(include_str!("../../migrations/006_ai_conversations.sql"))
                .execute(&pool)
                .await
                .map_err(|e| format!("[MIGRATION_006_ERROR] {e}"))?;
        }

        let has_ai_messages: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ai_messages')",
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("[MIGRATION_007_CHECK_ERROR] {e}"))?;

        if !has_ai_messages {
            sqlx::query(include_str!("../../migrations/007_ai_messages.sql"))
                .execute(&pool)
                .await
                .map_err(|e| format!("[MIGRATION_007_ERROR] {e}"))?;
        }

        let has_provider_type_unique_index: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_ai_providers_provider_type_unique')",
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("[MIGRATION_008_CHECK_ERROR] {e}"))?;

        if !has_provider_type_unique_index {
            sqlx::query(include_str!("../../migrations/008_ai_provider_vendor_unique.sql"))
                .execute(&pool)
                .await
                .map_err(|e| format!("[MIGRATION_008_ERROR] {e}"))?;
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

    pub async fn list_ai_providers(&self) -> Result<Vec<AiProvider>, String> {
        sqlx::query_as::<_, AiProvider>(
            "SELECT id, name, provider_type, base_url, model, api_key, is_default, enabled, extra_json, created_at, updated_at FROM ai_providers ORDER BY is_default DESC, updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[LIST_AI_PROVIDERS_ERROR] {e}"))
    }

    pub async fn get_ai_provider_by_id(&self, id: i64) -> Result<AiProvider, String> {
        sqlx::query_as::<_, AiProvider>(
            "SELECT id, name, provider_type, base_url, model, api_key, is_default, enabled, extra_json, created_at, updated_at FROM ai_providers WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[GET_AI_PROVIDER_ERROR] {e}"))
    }

    pub async fn get_default_ai_provider(&self) -> Result<AiProvider, String> {
        sqlx::query_as::<_, AiProvider>(
            "SELECT id, name, provider_type, base_url, model, api_key, is_default, enabled, extra_json, created_at, updated_at FROM ai_providers WHERE enabled = 1 ORDER BY is_default DESC, updated_at DESC LIMIT 1",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[GET_DEFAULT_AI_PROVIDER_ERROR] {e}"))
    }

    pub async fn create_ai_provider(&self, form: AiProviderForm) -> Result<AiProvider, String> {
        let provider_type = form.provider_type.unwrap_or_else(|| "openai".to_string());
        let has_default_provider: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ai_providers WHERE is_default = 1)",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[CREATE_AI_PROVIDER_DEFAULT_CHECK_ERROR] {e}"))?;
        let enabled = form.enabled.unwrap_or(true);

        let existing_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM ai_providers WHERE provider_type = ? LIMIT 1",
        )
        .bind(&provider_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("[CREATE_AI_PROVIDER_FIND_EXISTING_ERROR] {e}"))?;

        match existing_id {
            Some(id) => {
                let existing = self.get_ai_provider_by_id(id).await?;
                let is_default = form
                    .is_default
                    .unwrap_or(existing.is_default || !has_default_provider);
                if is_default {
                    sqlx::query("UPDATE ai_providers SET is_default = 0")
                        .execute(&self.pool)
                        .await
                        .map_err(|e| format!("[CREATE_AI_PROVIDER_DEFAULT_RESET_ERROR] {e}"))?;
                }
                sqlx::query(
                    "UPDATE ai_providers SET name = ?, base_url = ?, model = ?, api_key = ?, is_default = ?, enabled = ?, extra_json = ?, updated_at = datetime('now') WHERE id = ?",
                )
                .bind(form.name)
                .bind(form.base_url)
                .bind(form.model)
                .bind(form.api_key)
                .bind(is_default)
                .bind(enabled)
                .bind(form.extra_json)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| format!("[CREATE_AI_PROVIDER_UPSERT_UPDATE_ERROR] {e}"))?;

                self.get_ai_provider_by_id(id).await
            }
            None => {
                let is_default = form.is_default.unwrap_or(!has_default_provider);
                if is_default {
                    sqlx::query("UPDATE ai_providers SET is_default = 0")
                        .execute(&self.pool)
                        .await
                        .map_err(|e| format!("[CREATE_AI_PROVIDER_DEFAULT_RESET_ERROR] {e}"))?;
                }
                let id = sqlx::query_scalar::<_, i64>(
                    "INSERT INTO ai_providers (name, provider_type, base_url, model, api_key, is_default, enabled, extra_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
                )
                .bind(form.name)
                .bind(provider_type)
                .bind(form.base_url)
                .bind(form.model)
                .bind(form.api_key)
                .bind(is_default)
                .bind(enabled)
                .bind(form.extra_json)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| format!("[CREATE_AI_PROVIDER_INSERT_ERROR] {e}"))?;

                self.get_ai_provider_by_id(id).await
            }
        }
    }

    pub async fn update_ai_provider(&self, id: i64, form: AiProviderForm) -> Result<AiProvider, String> {
        let existing = self.get_ai_provider_by_id(id).await?;
        let provider_type = form
            .provider_type
            .clone()
            .unwrap_or(existing.provider_type.clone());
        let is_default = form.is_default.unwrap_or(existing.is_default);
        let enabled = form.enabled.unwrap_or(existing.enabled);
        let extra_json = form.extra_json.clone().or(existing.extra_json.clone());

        if is_default {
            sqlx::query("UPDATE ai_providers SET is_default = 0")
                .execute(&self.pool)
                .await
                .map_err(|e| format!("[UPDATE_AI_PROVIDER_DEFAULT_RESET_ERROR] {e}"))?;
        }

        sqlx::query(
            "UPDATE ai_providers SET name = ?, provider_type = ?, base_url = ?, model = ?, api_key = ?, is_default = ?, enabled = ?, extra_json = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(form.name)
        .bind(provider_type)
        .bind(form.base_url)
        .bind(form.model)
        .bind(form.api_key)
        .bind(is_default)
        .bind(enabled)
        .bind(extra_json)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("[UPDATE_AI_PROVIDER_ERROR] {e}"))?;

        self.get_ai_provider_by_id(id).await
    }

    pub async fn delete_ai_provider(&self, id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM ai_providers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[DELETE_AI_PROVIDER_ERROR] {e}"))?;
        Ok(())
    }

    pub async fn set_default_ai_provider(&self, id: i64) -> Result<(), String> {
        sqlx::query("UPDATE ai_providers SET is_default = 0")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[SET_DEFAULT_AI_PROVIDER_RESET_ERROR] {e}"))?;
        sqlx::query("UPDATE ai_providers SET is_default = 1, updated_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[SET_DEFAULT_AI_PROVIDER_ERROR] {e}"))?;
        Ok(())
    }

    pub async fn create_ai_conversation(
        &self,
        title: String,
        scenario: String,
        connection_id: Option<i64>,
        database: Option<String>,
    ) -> Result<AiConversation, String> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO ai_conversations (title, scenario, connection_id, database) VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(title)
        .bind(scenario)
        .bind(connection_id)
        .bind(database)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[CREATE_AI_CONVERSATION_ERROR] {e}"))?;
        self.get_ai_conversation(id).await
    }

    pub async fn list_ai_conversations(
        &self,
        connection_id: Option<i64>,
        database: Option<String>,
    ) -> Result<Vec<AiConversation>, String> {
        let mut query = "SELECT id, title, scenario, connection_id, database, created_at, updated_at FROM ai_conversations".to_string();
        let mut has_where = false;
        if connection_id.is_some() {
            query.push_str(" WHERE connection_id = ?");
            has_where = true;
        }
        if database.is_some() {
            if has_where {
                query.push_str(" AND database = ?");
            } else {
                query.push_str(" WHERE database = ?");
            }
        }
        query.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query_as::<_, AiConversation>(&query);
        if let Some(id) = connection_id {
            q = q.bind(id);
        }
        if let Some(db) = database {
            q = q.bind(db);
        }
        q.fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[LIST_AI_CONVERSATIONS_ERROR] {e}"))
    }

    pub async fn get_ai_conversation(&self, id: i64) -> Result<AiConversation, String> {
        sqlx::query_as::<_, AiConversation>(
            "SELECT id, title, scenario, connection_id, database, created_at, updated_at FROM ai_conversations WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[GET_AI_CONVERSATION_ERROR] {e}"))
    }

    pub async fn delete_ai_conversation(&self, id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM ai_messages WHERE conversation_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[DELETE_AI_CONVERSATION_MESSAGES_ERROR] {e}"))?;
        sqlx::query("DELETE FROM ai_conversations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[DELETE_AI_CONVERSATION_ERROR] {e}"))?;
        Ok(())
    }

    pub async fn touch_ai_conversation(&self, id: i64) -> Result<(), String> {
        sqlx::query("UPDATE ai_conversations SET updated_at = datetime('now') WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[TOUCH_AI_CONVERSATION_ERROR] {e}"))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_ai_message(
        &self,
        conversation_id: i64,
        role: String,
        content: String,
        prompt_version: Option<String>,
        model: Option<String>,
        token_in: Option<i64>,
        token_out: Option<i64>,
        latency_ms: Option<i64>,
    ) -> Result<AiMessage, String> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO ai_messages (conversation_id, role, content, prompt_version, model, token_in, token_out, latency_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(prompt_version)
        .bind(model)
        .bind(token_in)
        .bind(token_out)
        .bind(latency_ms)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[CREATE_AI_MESSAGE_ERROR] {e}"))?;

        self.get_ai_message(id).await
    }

    pub async fn get_ai_message(&self, id: i64) -> Result<AiMessage, String> {
        sqlx::query_as::<_, AiMessage>(
            "SELECT id, conversation_id, role, content, prompt_version, model, token_in, token_out, latency_ms, created_at FROM ai_messages WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("[GET_AI_MESSAGE_ERROR] {e}"))
    }

    pub async fn list_ai_messages(&self, conversation_id: i64) -> Result<Vec<AiMessage>, String> {
        sqlx::query_as::<_, AiMessage>(
            "SELECT id, conversation_id, role, content, prompt_version, model, token_in, token_out, latency_ms, created_at FROM ai_messages WHERE conversation_id = ? ORDER BY created_at ASC, id ASC",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[LIST_AI_MESSAGES_ERROR] {e}"))
    }
}
