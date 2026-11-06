use super::{decode_string_list, encode_string_list, LocalDb};
use crate::error::AppError;
use crate::models::{Connection, ConnectionForm};
use sqlx::Row as _;

impl LocalDb {
    pub async fn create_connection(&self, form: ConnectionForm) -> Result<Connection, AppError> {
        let uuid = uuid::Uuid::new_v4().to_string();
        let name = form
            .name
            .clone()
            .or_else(|| form.host.clone())
            .or_else(|| form.cloud_id.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM connections WHERE name = ?)")
                .bind(&name)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| AppError::internal_with("Database existence check failed", e))?;

        if exists {
            return Err(AppError::already_exists(format!(
                "Connection with name '{}' already exists",
                name
            )));
        }

        let proxy = encode_proxy(form.proxy.as_ref())?;
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO connections (uuid, type, name, host, port, database, username, password, ssl, ssl_mode, ssl_ca_cert, file_path, ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path, mode, seed_nodes, sentinels, connect_timeout_ms, service_name, sentinel_password, auth_mode, api_key_id, api_key_secret, api_key_encoded, cloud_id, auth_source, proxy)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
        )
        .bind(&uuid)
        .bind(&form.driver)
        .bind(&name)
        .bind(&form.host.unwrap_or_default())
        .bind(&form.port.unwrap_or(0))
        .bind(&form.database.unwrap_or_default())
        .bind(&form.username.unwrap_or_default())
        .bind(&form.password.unwrap_or_default())
        .bind(form.ssl.unwrap_or(false))
        .bind(form.ssl_mode)
        .bind(form.ssl_ca_cert)
        .bind(form.file_path)
        .bind(form.ssh_enabled.unwrap_or(false))
        .bind(form.ssh_host)
        .bind(form.ssh_port)
        .bind(form.ssh_username)
        .bind(form.ssh_password)
        .bind(form.ssh_key_path)
        .bind(form.mode)
        .bind(encode_string_list(form.seed_nodes))
        .bind(encode_string_list(form.sentinels))
        .bind(form.connect_timeout_ms)
        .bind(form.service_name)
        .bind(form.sentinel_password)
        .bind(form.auth_mode)
        .bind(form.api_key_id)
        .bind(form.api_key_secret)
        .bind(form.api_key_encoded)
        .bind(form.cloud_id)
        .bind(form.auth_source)
        .bind(proxy)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal(format!("[INSERT_ERROR] {e}")))?;

        self.get_connection_by_id(id).await
    }

    pub async fn update_connection(
        &self,
        id: i64,
        form: ConnectionForm,
    ) -> Result<Connection, AppError> {
        let mut form = form;
        let saved = self.get_connection_form_by_id(id).await?;
        crate::network::merge_proxy_password(&mut form, &saved);
        let proxy = encode_proxy(form.proxy.as_ref())?;
        sqlx::query(
            "UPDATE connections SET name = COALESCE(NULLIF(?, ''), name), type = ?, host = ?, port = ?, database = ?, username = ?, password = COALESCE(NULLIF(?, ''), password), ssl = ?, ssl_mode = ?, ssl_ca_cert = ?, file_path = ?, ssh_enabled = ?, ssh_host = ?, ssh_port = ?, ssh_username = ?, ssh_password = ?, ssh_key_path = ?, mode = ?, seed_nodes = ?, sentinels = ?, connect_timeout_ms = ?, service_name = ?, sentinel_password = COALESCE(NULLIF(?, ''), sentinel_password), auth_mode = ?, api_key_id = ?, api_key_secret = COALESCE(NULLIF(?, ''), api_key_secret), api_key_encoded = COALESCE(NULLIF(?, ''), api_key_encoded), cloud_id = ?, auth_source = ?, proxy = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(form.name)
        .bind(&form.driver)
        .bind(&form.host.unwrap_or_default())
        .bind(&form.port.unwrap_or(0))
        .bind(&form.database.unwrap_or_default())
        .bind(&form.username.unwrap_or_default())
        .bind(form.password)
        .bind(form.ssl.unwrap_or(false))
        .bind(form.ssl_mode)
        .bind(form.ssl_ca_cert)
        .bind(form.file_path)
        .bind(form.ssh_enabled.unwrap_or(false))
        .bind(form.ssh_host)
        .bind(form.ssh_port)
        .bind(form.ssh_username)
        .bind(form.ssh_password)
        .bind(form.ssh_key_path)
        .bind(form.mode)
        .bind(encode_string_list(form.seed_nodes))
        .bind(encode_string_list(form.sentinels))
        .bind(form.connect_timeout_ms)
        .bind(form.service_name)
        .bind(form.sentinel_password)
        .bind(form.auth_mode)
        .bind(form.api_key_id)
        .bind(form.api_key_secret)
        .bind(form.api_key_encoded)
        .bind(form.cloud_id)
        .bind(form.auth_source)
        .bind(proxy)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::internal(format!("[UPDATE_ERROR] {e}")))?;

        self.get_connection_by_id(id).await
    }

    pub async fn delete_connection(&self, id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::internal(format!("[DELETE_ERROR] {e}")))?;
        Ok(())
    }

    pub async fn list_connections(&self) -> Result<Vec<Connection>, AppError> {
        let rows = sqlx::query(
            r#"SELECT
                id, uuid, name, type as db_type, host, port, database, username, ssl, ssl_mode, ssl_ca_cert, file_path,
                ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path,
                mode, seed_nodes, sentinels, connect_timeout_ms, service_name, NULL as sentinel_password,
                auth_mode, api_key_id, NULL as api_key_secret, NULL as api_key_encoded, cloud_id, auth_source, proxy,
                created_at, updated_at
               FROM connections
               ORDER BY created_at ASC, id ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::query_failed(format!("{e}")).to_string())?;
        Ok(rows
            .into_iter()
            .map(|row| Connection {
                id: row.try_get("id").unwrap_or_default(),
                uuid: row.try_get("uuid").unwrap_or_default(),
                name: row.try_get("name").unwrap_or_default(),
                db_type: row.try_get("db_type").unwrap_or_default(),
                host: row.try_get("host").unwrap_or_default(),
                port: row.try_get("port").unwrap_or_default(),
                database: row.try_get("database").unwrap_or_default(),
                username: row.try_get("username").unwrap_or_default(),
                ssl: row.try_get("ssl").unwrap_or(false),
                ssl_mode: row.try_get("ssl_mode").ok(),
                ssl_ca_cert: row.try_get("ssl_ca_cert").ok(),
                file_path: row.try_get("file_path").ok(),
                ssh_enabled: row.try_get("ssh_enabled").unwrap_or(false),
                ssh_host: row.try_get("ssh_host").ok(),
                ssh_port: row.try_get::<Option<i64>, _>("ssh_port").ok().flatten(),
                ssh_username: row.try_get("ssh_username").ok(),
                ssh_password: row.try_get("ssh_password").ok(),
                ssh_key_path: row.try_get("ssh_key_path").ok(),
                mode: row.try_get("mode").ok(),
                seed_nodes: decode_string_list(row.try_get("seed_nodes").ok()),
                sentinels: decode_string_list(row.try_get("sentinels").ok()),
                connect_timeout_ms: row.try_get("connect_timeout_ms").ok(),
                service_name: row.try_get("service_name").ok(),
                sentinel_password: row.try_get("sentinel_password").ok(),
                auth_mode: row.try_get("auth_mode").ok(),
                api_key_id: row.try_get("api_key_id").ok(),
                api_key_secret: row.try_get("api_key_secret").ok(),
                api_key_encoded: row.try_get("api_key_encoded").ok(),
                cloud_id: row.try_get("cloud_id").ok(),
                auth_source: row.try_get("auth_source").ok(),
                proxy: public_proxy(row.try_get::<Option<String>, _>("proxy").ok().flatten()),
                created_at: row.try_get("created_at").unwrap_or_default(),
                updated_at: row.try_get("updated_at").unwrap_or_default(),
            })
            .collect())
    }

    pub async fn get_connection_by_id(&self, id: i64) -> Result<Connection, AppError> {
        let row = sqlx::query(
            r#"SELECT
                id, uuid, name, type as db_type, host, port, database, username, ssl, ssl_mode, ssl_ca_cert, file_path,
                ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path,
                mode, seed_nodes, sentinels, connect_timeout_ms, service_name, NULL as sentinel_password,
                auth_mode, api_key_id, NULL as api_key_secret, NULL as api_key_encoded, cloud_id, auth_source, proxy,
                created_at, updated_at
               FROM connections
               WHERE id = ?"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::query_failed(format!("{e}")).to_string())?;

        Ok(Connection {
            id: row.try_get("id").unwrap_or_default(),
            uuid: row.try_get("uuid").unwrap_or_default(),
            name: row.try_get("name").unwrap_or_default(),
            db_type: row.try_get("db_type").unwrap_or_default(),
            host: row.try_get("host").unwrap_or_default(),
            port: row.try_get("port").unwrap_or_default(),
            database: row.try_get("database").unwrap_or_default(),
            username: row.try_get("username").unwrap_or_default(),
            ssl: row.try_get("ssl").unwrap_or(false),
            ssl_mode: row.try_get("ssl_mode").ok(),
            ssl_ca_cert: row.try_get("ssl_ca_cert").ok(),
            file_path: row.try_get("file_path").ok(),
            ssh_enabled: row.try_get("ssh_enabled").unwrap_or(false),
            ssh_host: row.try_get("ssh_host").ok(),
            ssh_port: row.try_get::<Option<i64>, _>("ssh_port").ok().flatten(),
            ssh_username: row.try_get("ssh_username").ok(),
            ssh_password: row.try_get("ssh_password").ok(),
            ssh_key_path: row.try_get("ssh_key_path").ok(),
            mode: row.try_get("mode").ok(),
            seed_nodes: decode_string_list(row.try_get("seed_nodes").ok()),
            sentinels: decode_string_list(row.try_get("sentinels").ok()),
            connect_timeout_ms: row.try_get("connect_timeout_ms").ok(),
            service_name: row.try_get("service_name").ok(),
            sentinel_password: row.try_get("sentinel_password").ok(),
            auth_mode: row.try_get("auth_mode").ok(),
            api_key_id: row.try_get("api_key_id").ok(),
            api_key_secret: row.try_get("api_key_secret").ok(),
            api_key_encoded: row.try_get("api_key_encoded").ok(),
            cloud_id: row.try_get("cloud_id").ok(),
            auth_source: row.try_get("auth_source").ok(),
            proxy: public_proxy(row.try_get::<Option<String>, _>("proxy").ok().flatten()),
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
        })
    }

    pub async fn get_connection_form_by_id(&self, id: i64) -> Result<ConnectionForm, AppError> {
        let row = sqlx::query(
            "SELECT type as db_type, name, host, port, database, username, password, ssl, ssl_mode, ssl_ca_cert, file_path, ssh_enabled, ssh_host, ssh_port, ssh_username, ssh_password, ssh_key_path, mode, seed_nodes, sentinels, connect_timeout_ms, service_name, sentinel_password, auth_mode, api_key_id, api_key_secret, api_key_encoded, cloud_id, auth_source, proxy FROM connections WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::query_failed(format!("{e}")))?;

        Ok(ConnectionForm {
            driver: row.try_get("db_type").unwrap_or_default(),
            name: row.try_get("name").ok(),
            host: row.try_get("host").ok(),
            port: row.try_get("port").ok(),
            database: row.try_get("database").ok(),
            schema: None,
            username: row.try_get("username").ok(),
            password: row.try_get("password").ok(),
            ssl: row.try_get::<bool, _>("ssl").ok().map(|v| v),
            ssl_mode: row.try_get("ssl_mode").ok(),
            ssl_ca_cert: row.try_get("ssl_ca_cert").ok(),
            file_path: row.try_get("file_path").ok(),
            ssh_enabled: row.try_get::<bool, _>("ssh_enabled").ok().map(|v| v),
            ssh_host: row.try_get("ssh_host").ok(),
            ssh_port: row.try_get::<Option<i64>, _>("ssh_port").ok().flatten(),
            ssh_username: row.try_get("ssh_username").ok(),
            ssh_password: row.try_get("ssh_password").ok(),
            ssh_key_path: row.try_get("ssh_key_path").ok(),
            mode: row.try_get("mode").ok(),
            seed_nodes: decode_string_list(row.try_get("seed_nodes").ok()),
            sentinels: decode_string_list(row.try_get("sentinels").ok()),
            connect_timeout_ms: row.try_get("connect_timeout_ms").ok(),
            service_name: row.try_get("service_name").ok(),
            sentinel_password: row.try_get("sentinel_password").ok(),
            auth_mode: row.try_get("auth_mode").ok(),
            api_key_id: row.try_get("api_key_id").ok(),
            api_key_secret: row.try_get("api_key_secret").ok(),
            api_key_encoded: row.try_get("api_key_encoded").ok(),
            cloud_id: row.try_get("cloud_id").ok(),
            auth_source: row.try_get("auth_source").ok(),
            proxy: decode_proxy(row.try_get::<Option<String>, _>("proxy").ok().flatten())?,
        })
    }
}

fn encode_proxy(proxy: Option<&crate::models::NetworkProxy>) -> Result<Option<String>, AppError> {
    proxy
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| AppError::internal_with("Invalid proxy configuration", e))
}

fn decode_proxy(value: Option<String>) -> Result<Option<crate::models::NetworkProxy>, AppError> {
    value
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|e| AppError::internal_with("Invalid proxy configuration", e))
}

fn public_proxy(value: Option<String>) -> Option<crate::models::NetworkProxy> {
    decode_proxy(value).ok().flatten().map(|mut proxy| {
        proxy.password.clear();
        proxy
    })
}

#[cfg(test)]
mod tests;
