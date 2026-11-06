use super::super::{conn_failed_error, DriverResult};
use crate::error::AppError;
use crate::models::ConnectionForm;
use odbc_api::ConnectionOptions;

#[derive(Clone)]
pub struct Db2Config {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

pub struct Db2Connection {
    pub config: Db2Config,
    pub network_tunnel: Option<crate::network::NetworkTunnel>,
}

fn odbc_escape_value(v: &str) -> String {
    if v.contains(';') || v.contains('{') || v.contains('}') || v.contains('[') {
        format!("{{{}}}", v.replace('}', "}}"))
    } else {
        v.to_string()
    }
}

pub fn build_connection_string(cfg: &Db2Config) -> String {
    format!(
        "DRIVER={{IBM DB2 ODBC DRIVER}};DATABASE={};HOSTNAME={};PORT={};PROTOCOL=TCPIP;UID={};PWD={};",
        cfg.database, cfg.host, cfg.port, odbc_escape_value(&cfg.username), odbc_escape_value(&cfg.password)
    )
}

impl Db2Connection {
    pub async fn connect(form: &ConnectionForm) -> DriverResult<Self> {
        let mut effective_form = form.clone();
        let mut network_tunnel = None;

        if crate::network::tunnel_enabled(form) {
            let tunnel = crate::network::start_network_tunnel(form).await?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            network_tunnel = Some(tunnel);
        }

        let host = effective_form
            .host
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::validation("host cannot be empty"))?;
        let port = effective_form.port.unwrap_or(50000);
        if !(1..=65535).contains(&port) {
            return Err(AppError::validation("port out of range"));
        }
        let database = effective_form
            .database
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::validation("database cannot be empty"))?;
        let username = effective_form
            .username
            .clone()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::validation("username cannot be empty"))?;
        let password = effective_form.password.clone().unwrap_or_default();

        let config = Db2Config {
            host,
            port: port as u16,
            database,
            username,
            password,
        };
        let conn = Self {
            config,
            network_tunnel,
        };
        conn.test_connection().await?;
        Ok(conn)
    }

    pub async fn test_connection(&self) -> DriverResult<()> {
        self.run_blocking(|conn| {
            let cursor = conn
                .execute("SELECT 1 FROM SYSIBM.SYSDUMMY1", ())
                .map_err(|e| conn_failed_error(&e))?;
            if cursor.is_none() {
                return Err(conn_failed_error(
                    &"Empty response from SYSIBM.SYSDUMMY1".to_string(),
                ));
            }
            Ok(())
        })
        .await
    }

    pub async fn run_blocking<F, T>(&self, f: F) -> DriverResult<T>
    where
        F: FnOnce(odbc_api::Connection<'_>) -> DriverResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let cfg = self.config.clone();
        tokio::task::spawn_blocking(move || {
            let conn_string = build_connection_string(&cfg);
            let env = odbc_api::Environment::new().map_err(|e| conn_failed_error(&e))?;
            let conn = env
                .connect_with_connection_string(&conn_string, ConnectionOptions::default())
                .map_err(|e| conn_failed_error(&e))?;
            f(conn)
        })
        .await
        .map_err(|e| AppError::internal(format!("DB2 blocking task failed: {e}")))?
    }

    pub async fn close(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odbc_escape_value_plain() {
        assert_eq!(odbc_escape_value("myuser"), "myuser");
    }

    #[test]
    fn odbc_escape_value_with_semicolon() {
        assert_eq!(odbc_escape_value("p@ss;word"), "{p@ss;word}");
    }

    #[test]
    fn odbc_escape_value_with_braces() {
        assert_eq!(odbc_escape_value("a{b}c"), "{a{b}}c}");
    }
}
