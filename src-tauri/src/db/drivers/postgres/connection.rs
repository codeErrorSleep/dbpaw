use crate::error::AppError;
use crate::models::ConnectionForm;
use crate::network::NetworkTunnel;
use std::fs;
use std::path::{Path, PathBuf};

fn validation_error(message: impl Into<String>) -> AppError {
    AppError::validation(message)
}

pub struct PostgresConnection {
    pub pool: sqlx::PgPool,
    pub network_tunnel: Option<NetworkTunnel>,
    pub ca_cert_path: Option<PathBuf>,
}

fn write_temp_cert_file(prefix: &str, pem: &str) -> Result<PathBuf, AppError> {
    let dir = std::env::temp_dir().join("dbpaw_certs");
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_with("Failed to create cert directory", e))?;
    let path = dir.join(format!("{prefix}_{}.pem", uuid::Uuid::new_v4()));
    fs::write(&path, pem).map_err(|e| AppError::internal_with("Failed to write cert file", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, perm)
            .map_err(|e| AppError::internal_with("Failed to set cert file permissions", e))?;
    }
    Ok(path)
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for b in value.bytes() {
        let is_unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if is_unreserved {
            encoded.push(b as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", b));
        }
    }
    encoded
}

fn build_verify_ca_query_param(ca_path: &Path) -> String {
    format!(
        "?sslmode=verify-ca&sslrootcert={}",
        percent_encode_query_value(&ca_path.to_string_lossy())
    )
}

pub fn build_dsn_and_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), AppError> {
    let host = form
        .host
        .clone()
        .ok_or_else(|| validation_error("host cannot be empty"))?;
    let port = form.port.unwrap_or(5432);
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let username = form
        .username
        .clone()
        .ok_or_else(|| validation_error("username cannot be empty"))?;
    let password = form
        .password
        .clone()
        .ok_or_else(|| validation_error("password cannot be empty"))?;
    let username = percent_encode_query_value(&username);
    let password = percent_encode_query_value(&password);
    let mut dsn = format!(
        "postgres://{}:{}@{}:{}/{}",
        username, password, host, port, database
    );

    let mut ca_cert_path = None;
    if form.ssl.unwrap_or(false) {
        let ssl_mode = form
            .ssl_mode
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("require");
        if ssl_mode == "verify_ca" {
            let ca_cert = form
                .ssl_ca_cert
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| validation_error("sslCaCert cannot be empty in verify_ca mode"))?;
            let ca_path = write_temp_cert_file("pg_ca", ca_cert)?;
            dsn.push_str(&build_verify_ca_query_param(&ca_path));
            ca_cert_path = Some(ca_path);
        } else {
            dsn.push_str("?sslmode=require");
        }
    }

    Ok((dsn, ca_cert_path))
}

#[cfg(test)]
pub fn build_dsn(form: &ConnectionForm) -> Result<String, AppError> {
    Ok(build_dsn_and_ca_path(form)?.0)
}

fn build_dsn_with_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), AppError> {
    build_dsn_and_ca_path(form)
}

fn cleanup_ca_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn cleanup_ca_file_opt(path: Option<&PathBuf>) {
    if let Some(p) = path {
        cleanup_ca_file(p);
    }
}

impl PostgresConnection {
    pub async fn new(form: &ConnectionForm) -> Result<Self, AppError> {
        let mut dsn_form = form.clone();
        let mut network_tunnel = None;

        if crate::network::tunnel_enabled(form) {
            let tunnel = crate::network::start_network_tunnel(form).await?;
            dsn_form.host = Some("127.0.0.1".to_string());
            dsn_form.port = Some(tunnel.local_port as i64);
            network_tunnel = Some(tunnel);
        }

        let (dsn, ca_cert_path) = build_dsn_with_ca_path(&dsn_form)?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(&dsn)
            .await
            .map_err(|e| super::super::conn_failed_error(&e))?;

        Ok(Self {
            pool,
            network_tunnel,
            ca_cert_path,
        })
    }

    pub async fn test_connection(&self) -> Result<(), AppError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::query_failed(e.to_string()))?;
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
        self.cleanup_ca_file();
    }

    fn cleanup_ca_file(&self) {
        cleanup_ca_file_opt(self.ca_cert_path.as_ref());
    }
}

impl Drop for PostgresConnection {
    fn drop(&mut self) {
        cleanup_ca_file_opt(self.ca_cert_path.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conn_string_generation() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ..Default::default()
        };
        let dsn = build_dsn(&form).unwrap();
        assert_eq!(dsn, "postgres://postgres:password@localhost:5432/mydb");
    }

    #[test]
    fn test_conn_string_encodes_credentials() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("user@name".to_string()),
            password: Some("p@ss:word#?".to_string()),
            database: Some("mydb".to_string()),
            ..Default::default()
        };

        let dsn = build_dsn(&form).unwrap();
        assert_eq!(
            dsn,
            "postgres://user%40name:p%40ss%3Aword%23%3F@localhost:5432/mydb"
        );
    }

    #[test]
    fn test_conn_string_encodes_credentials_when_ssh_rewrites_target_host() {
        let mut form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(5432),
            username: Some("user@name".to_string()),
            password: Some("p#ss*@)".to_string()),
            database: Some("mydb".to_string()),
            ssh_enabled: Some(true),
            ssh_host: Some("bastion.internal".to_string()),
            ssh_port: Some(22),
            ssh_username: Some("jump".to_string()),
            ssh_password: Some("ssh#pass".to_string()),
            ..Default::default()
        };

        form.host = Some("127.0.0.1".to_string());
        form.port = Some(55432);

        let dsn = build_dsn(&form).unwrap();
        assert_eq!(
            dsn,
            "postgres://user%40name:p%23ss%2A%40%29@127.0.0.1:55432/mydb"
        );
    }

    #[test]
    fn test_conn_string_missing_fields() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: None,
            ..Default::default()
        };
        assert!(build_dsn(&form).is_err());
    }

    #[test]
    fn test_conn_string_with_ssl() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ssl: Some(true),
            ..Default::default()
        };
        let dsn = build_dsn(&form).unwrap();
        assert_eq!(
            dsn,
            "postgres://postgres:password@localhost:5432/mydb?sslmode=require"
        );
    }

    #[test]
    fn test_conn_string_with_ssl_false_does_not_explicitly_disable_tls() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ssl: Some(false),
            ..Default::default()
        };
        let dsn = build_dsn(&form).unwrap();
        assert_eq!(dsn, "postgres://postgres:password@localhost:5432/mydb");
        assert!(!dsn.contains("sslmode="));
        assert!(!dsn.contains("sslmode=disable"));
    }

    #[test]
    fn test_conn_string_with_ssl_verify_ca_requires_ca() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ssl: Some(true),
            ssl_mode: Some("verify_ca".to_string()),
            ssl_ca_cert: None,
            ..Default::default()
        };
        assert!(build_dsn(&form).is_err());
    }

    #[test]
    fn test_verify_ca_query_param_encodes_path() {
        let path = PathBuf::from("/tmp/a b&c#d?.pem");
        let query = build_verify_ca_query_param(&path);
        assert_eq!(
            query,
            "?sslmode=verify-ca&sslrootcert=%2Ftmp%2Fa%20b%26c%23d%3F.pem"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_temp_cert_file_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = write_temp_cert_file("pg_ca_perm_test", "pem-data").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = fs::remove_file(&path);
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_cleanup_ca_file_opt_removes_file() {
        let path = write_temp_cert_file("pg_ca_cleanup_test", "pem-data").unwrap();
        assert!(path.exists());
        cleanup_ca_file_opt(Some(&path));
        assert!(!path.exists());
    }
}
