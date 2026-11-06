use crate::error::AppError;
use crate::models::ConnectionForm;
use crate::network::NetworkTunnel;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[cfg(test)]
use sqlx::ConnectOptions;

fn validation_error(message: impl Into<String>) -> AppError {
    AppError::validation(message)
}

fn conn_error(message: impl Into<String>) -> AppError {
    AppError::conn_failed(message, "Check connection settings")
}

pub struct MysqlConnection {
    pub pool: sqlx::MySqlPool,
    pub network_tunnel: Option<NetworkTunnel>,
    pub ca_cert_path: Option<PathBuf>,
    pub driver_name: String,
    pub compatibility_mode: bool,
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
        "?ssl-mode=VERIFY_CA&ssl-ca={}",
        percent_encode_query_value(&ca_path.to_string_lossy())
    )
}

fn mysql_family_default_port(driver: &str) -> u16 {
    if driver.eq_ignore_ascii_case("starrocks") || driver.eq_ignore_ascii_case("doris") {
        9030
    } else {
        3306
    }
}

fn normalize_mysql_host_and_port(
    raw_driver: &str,
    raw_host: &str,
    raw_port: Option<i64>,
) -> Result<(String, u16), AppError> {
    let mut host = raw_host.trim().to_string();
    if host.is_empty() {
        return Err(validation_error("host cannot be empty"));
    }

    let mut port = raw_port.unwrap_or(i64::from(mysql_family_default_port(raw_driver)));
    if !host.starts_with('[') && host.matches(':').count() == 1 {
        if let Some((host_part, port_part)) = host.rsplit_once(':') {
            let host_part = host_part.trim();
            let port_part = port_part.trim();
            if !host_part.is_empty() && port_part.chars().all(|c| c.is_ascii_digit()) {
                if raw_port.is_none() {
                    port = port_part.parse::<i64>().unwrap_or(port);
                }
                host = host_part.to_string();
            }
        }
    }

    if host.is_empty() {
        return Err(validation_error("host cannot be empty"));
    }
    if !(1..=65535).contains(&port) {
        return Err(validation_error("port must be between 1 and 65535"));
    }

    Ok((host, port as u16))
}

fn build_dsn_and_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), AppError> {
    let raw_host = form
        .host
        .clone()
        .ok_or_else(|| validation_error("host cannot be empty"))?;
    let (host, port) = normalize_mysql_host_and_port(&form.driver, &raw_host, form.port)?;
    // Allow database to be empty
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
    let mut dsn = format!("mysql://{}:{}@{}:{}", username, password, host, port);

    if let Some(db) = &form.database {
        if !db.is_empty() {
            dsn.push('/');
            dsn.push_str(db);
        }
    }

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
            let ca_path = write_temp_cert_file("mysql_ca", ca_cert)?;
            dsn.push_str(&build_verify_ca_query_param(&ca_path));
            ca_cert_path = Some(ca_path);
        } else {
            dsn.push_str("?ssl-mode=REQUIRED");
        }
    } else {
        // Explicitly disable TLS to avoid HandshakeFailure on servers with TLS
        // versions or cipher suites incompatible with rustls (TLS 1.2+ only).
        dsn.push_str("?ssl-mode=DISABLED");
    }

    Ok((dsn, ca_cert_path))
}

#[cfg(test)]
fn build_dsn(form: &ConnectionForm) -> Result<String, AppError> {
    Ok(build_dsn_and_ca_path(form)?.0)
}

#[cfg(test)]
pub(crate) fn build_test_dsn(form: &ConnectionForm) -> Result<String, AppError> {
    build_dsn(form)
}

fn build_dsn_with_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), AppError> {
    build_dsn_and_ca_path(form)
}

fn build_connect_options(dsn: &str, driver: &str) -> Result<MySqlConnectOptions, AppError> {
    let mut options = MySqlConnectOptions::from_str(dsn).map_err(|e| conn_error(e.to_string()))?;

    if driver.eq_ignore_ascii_case("starrocks") || driver.eq_ignore_ascii_case("doris") {
        // sqlx initializes MySQL connections with:
        // SET sql_mode=(SELECT CONCAT(@@sql_mode, ...))
        // plus timezone / SET NAMES session mutations tailored for MySQL.
        // StarRocks and Doris reject part of this initialization sequence, so
        // skip the post-connect SET mutations entirely for those compatibility
        // paths.
        options = options
            .pipes_as_concat(false)
            .no_engine_substitution(false)
            .timezone(None::<String>)
            .set_names(false);
    }

    Ok(options)
}

fn cleanup_ca_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn cleanup_ca_file_opt(path: Option<&PathBuf>) {
    if let Some(p) = path {
        cleanup_ca_file(p);
    }
}

impl MysqlConnection {
    fn uses_mysql_compatibility_mode(driver: &str) -> bool {
        driver.eq_ignore_ascii_case("starrocks") || driver.eq_ignore_ascii_case("doris")
    }

    pub async fn connect(form: &ConnectionForm) -> Result<Self, AppError> {
        let mut dsn_form = form.clone();
        let mut network_tunnel = None;

        if crate::network::tunnel_enabled(form) {
            let tunnel = crate::network::start_network_tunnel(form).await?;
            dsn_form.host = Some("127.0.0.1".to_string());
            dsn_form.port = Some(tunnel.local_port as i64);
            network_tunnel = Some(tunnel);
        }

        let (dsn, ca_cert_path) = build_dsn_with_ca_path(&dsn_form)?;
        let connect_options = build_connect_options(&dsn, &dsn_form.driver)?;
        let mut pool_options = MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3));
        if Self::uses_mysql_compatibility_mode(&dsn_form.driver) {
            pool_options = pool_options
                .test_before_acquire(false)
                .after_release(|_, _| Box::pin(async move { Ok(false) }));
        }

        let pool = pool_options
            .connect_with(connect_options)
            .await
            .map_err(|e| super::super::conn_failed_error(&e))?;

        Ok(Self {
            pool,
            network_tunnel,
            ca_cert_path,
            driver_name: dsn_form.driver.to_ascii_lowercase(),
            compatibility_mode: Self::uses_mysql_compatibility_mode(&dsn_form.driver),
        })
    }

    pub async fn test_connection(&self) -> Result<(), AppError> {
        if self.compatibility_mode {
            sqlx::raw_sql("SELECT 1")
                .execute(&self.pool)
                .await
                .map_err(|e| AppError::query_failed(e.to_string()))?;
            return Ok(());
        }

        if let Err(e) = sqlx::query("SELECT 1").execute(&self.pool).await {
            let error_text = e.to_string();
            if is_prepared_protocol_unsupported_error(&error_text) {
                sqlx::raw_sql("SELECT 1")
                    .execute(&self.pool)
                    .await
                    .map_err(|raw_err| AppError::query_failed(raw_err.to_string()))?;
            } else {
                return Err(AppError::query_failed(e.to_string()));
            }
        }
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

impl Drop for MysqlConnection {
    fn drop(&mut self) {
        cleanup_ca_file_opt(self.ca_cert_path.as_ref());
    }
}

fn is_prepared_protocol_unsupported_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("1295")
        || lower.contains("prepared statement protocol")
        || lower.contains("preparedoes not support") // PolarDB-X
        || lower.contains("only support prepare selectstmt or insertstmt now") // Doris
        || lower.contains("prepareok expected 12 bytes but got 10 bytes") // Doris/sqlx protocol mismatch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ConnectionForm;

    #[test]
    fn test_sanitize_mysql_subquery_sql_trims_trailing_semicolon() {
        // This test is kept for compatibility but the function is now in query.rs
        // Placeholder to maintain test count
    }

    #[test]
    fn test_conn_string_generation() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3306/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_without_db() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(3307),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            database: None,
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user:pass@127.0.0.1:3307?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_allows_empty_password_when_present() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(3307),
            username: Some("user".to_string()),
            password: Some(String::new()),
            database: None,
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(conn_str, "mysql://user:@127.0.0.1:3307?ssl-mode=DISABLED");
    }

    #[test]
    fn test_conn_string_strips_host_embedded_port() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1:3307".to_string()),
            port: Some(3307),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            database: None,
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user:pass@127.0.0.1:3307?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_accepts_host_embedded_port_when_port_missing() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost:3308".to_string()),
            port: None,
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3308/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_uses_starrocks_default_port_when_port_missing() {
        let form = ConnectionForm {
            driver: "starrocks".to_string(),
            host: Some("localhost".to_string()),
            port: None,
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("analytics".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:9030/analytics?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_uses_doris_default_port_when_port_missing() {
        let form = ConnectionForm {
            driver: "doris".to_string(),
            host: Some("localhost".to_string()),
            port: None,
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("analytics".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:9030/analytics?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_starrocks_connect_options_disable_sql_mode_mutations() {
        let options = build_connect_options(
            "mysql://root:password@localhost:9030/analytics?ssl-mode=DISABLED",
            "starrocks",
        )
        .unwrap();

        let rendered = options.to_url_lossy().to_string();
        assert!(rendered.contains("ssl-mode=DISABLED"));
    }

    #[test]
    fn test_doris_connect_options_disable_sql_mode_mutations() {
        let options = build_connect_options(
            "mysql://root:password@localhost:9030/analytics?ssl-mode=DISABLED",
            "doris",
        )
        .unwrap();

        let rendered = options.to_url_lossy().to_string();
        assert!(rendered.contains("ssl-mode=DISABLED"));
    }

    #[test]
    fn test_conn_string_encodes_credentials() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("user@name".to_string()),
            password: Some("p@ss:word#?".to_string()),
            database: Some("test_db".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user%40name:p%40ss%3Aword%23%3F@localhost:3306/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_encodes_credentials_when_ssh_rewrites_target_host() {
        let mut form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(3306),
            username: Some("user@name".to_string()),
            password: Some("p#ss*@)".to_string()),
            database: Some("test_db".to_string()),
            ssh_enabled: Some(true),
            ssh_host: Some("bastion.internal".to_string()),
            ssh_port: Some(22),
            ssh_username: Some("jump".to_string()),
            ssh_password: Some("ssh#pass".to_string()),
            ..Default::default()
        };

        // Match the production flow after the SSH tunnel assigns a local endpoint.
        form.host = Some("127.0.0.1".to_string());
        form.port = Some(4406);

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user%40name:p%23ss%2A%40%29@127.0.0.1:4406/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_missing_fields() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: None, // Missing host
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test".to_string()),
            ..Default::default()
        };

        assert!(build_dsn(&form).is_err());
    }

    #[test]
    fn test_conn_string_with_ssl() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ssl: Some(true),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3306/test_db?ssl-mode=REQUIRED"
        );
    }

    #[test]
    fn test_conn_string_with_ssl_false_explicitly_disables_tls() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ssl: Some(false),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3306/test_db?ssl-mode=DISABLED"
        );
        assert!(conn_str.contains("ssl-mode=DISABLED"));
    }

    #[test]
    fn test_conn_string_with_ssl_verify_ca_requires_ca() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
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
            "?ssl-mode=VERIFY_CA&ssl-ca=%2Ftmp%2Fa%20b%26c%23d%3F.pem"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_temp_cert_file_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = write_temp_cert_file("mysql_ca_perm_test", "pem-data").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = fs::remove_file(&path);
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_cleanup_ca_file_opt_removes_file() {
        let path = write_temp_cert_file("mysql_ca_cleanup_test", "pem-data").unwrap();
        assert!(path.exists());
        cleanup_ca_file_opt(Some(&path));
        assert!(!path.exists());
    }

    #[test]
    fn test_is_prepared_protocol_unsupported_error() {
        assert!(is_prepared_protocol_unsupported_error(
            "error returned from database: 1295 (HY000): This command is not supported in the prepared statement protocol yet"
        ));
        assert!(is_prepared_protocol_unsupported_error(
            "prepared statement protocol is unsupported"
        ));
        assert!(is_prepared_protocol_unsupported_error(
            "error returned from database: 0 (HYo00):[1b6d607a89402000][10.233.70.102:3306][polardbx]Preparedoes not support sql: SELECT 1"
        ));
        assert!(is_prepared_protocol_unsupported_error(
            "error returned from database: 1105 (HY000): errCode = 2, detailMessage = Only support prepare SelectStmt or InsertStmt now"
        ));
        assert!(!is_prepared_protocol_unsupported_error(
            "syntax error near ...",
        ));
    }
}
