use super::super::{DatabaseDriver, DriverResult};
use crate::error::AppError;
use crate::models::ConnectionForm;
use bb8::{Pool, RunError};
use futures_util::TryStreamExt;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel, QueryItem, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::MssqlDriver;

pub(crate) fn validation_error(message: impl Into<String>) -> AppError {
    AppError::validation(message)
}

pub(crate) fn query_error(message: impl Into<String>) -> AppError {
    AppError::query_failed(message)
}

pub(crate) fn conn_error(message: impl Into<String>) -> AppError {
    AppError::conn_failed(message, "Check connection settings")
}

pub(crate) fn map_pool_error(err: RunError<AppError>) -> AppError {
    match err {
        RunError::User(inner) => inner,
        RunError::TimedOut => conn_error("Timed out acquiring MSSQL connection"),
    }
}

#[derive(Clone)]
pub(crate) struct MssqlConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl: bool,
    pub auth_mode: Option<String>,
    pub instance_name: Option<String>,
}

pub struct MssqlConnectionManager {
    config: MssqlConfig,
    proxy_port: Option<u16>,
}

pub(crate) fn build_config(form: &ConnectionForm) -> DriverResult<MssqlConfig> {
    let raw_host = form
        .host
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| validation_error("host cannot be empty"))?;

    let (host, instance_name) = if let Some((h, inst)) = raw_host.rsplit_once('\\') {
        let h = h.trim().to_string();
        let inst = inst.trim().to_string();
        if h.is_empty() || inst.is_empty() {
            return Err(validation_error("invalid host\\instance format"));
        }
        (h, Some(inst))
    } else {
        (raw_host, None)
    };

    let port = form.port.unwrap_or(1433);
    if !(0..=65535).contains(&port) {
        return Err(validation_error("port out of range"));
    }
    let database = form
        .database
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "master".to_string());
    let auth_mode = form.auth_mode.clone().map(|v| v.trim().to_string());

    if let Some(ref mode) = auth_mode {
        if mode.eq_ignore_ascii_case("windows") && !cfg!(target_os = "windows") {
            return Err(validation_error(
                "Windows authentication is only available on Windows. Please use SQL Server authentication or Integrated authentication instead.",
            ));
        }
    }

    let username = form
        .username
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_default();
    let password = form.password.clone().unwrap_or_default();

    Ok(MssqlConfig {
        host,
        port: port as u16,
        database,
        username,
        password,
        ssl: form.ssl.unwrap_or(false),
        auth_mode,
        instance_name,
    })
}

async fn detect_for_json_support(client: &mut Client<Compat<TcpStream>>) -> bool {
    let Ok(mut stream) = client
        .simple_query("SELECT CAST(SERVERPROPERTY('ProductMajorVersion') AS VARCHAR(10))")
        .await
    else {
        return false;
    };
    while let Ok(Some(item)) = stream.try_next().await {
        if let QueryItem::Row(row) = item {
            if let Ok(Some(v)) = row.try_get::<&str, _>(0) {
                if let Ok(major) = v.trim().parse::<u32>() {
                    return major >= 13;
                }
            }
        }
    }
    false
}

impl MssqlConnectionManager {
    fn new(config: MssqlConfig) -> Self {
        Self {
            config,
            proxy_port: None,
        }
    }

    fn build_tiberius_config(&self, encryption: EncryptionLevel, trust_cert: bool) -> Config {
        let mut config = Config::new();
        config.host(&self.config.host);
        config.port(self.config.port);
        config.database(&self.config.database);
        if let Some(ref instance) = self.config.instance_name {
            config.instance_name(instance);
        }
        let auth = self.build_auth_method();
        config.authentication(auth);
        config.encryption(encryption);
        if trust_cert
            && !matches!(
                encryption,
                EncryptionLevel::Off | EncryptionLevel::NotSupported
            )
        {
            config.trust_cert();
        }
        config
    }

    fn build_auth_method(&self) -> AuthMethod {
        let mode = self.config.auth_mode.as_deref().unwrap_or("sql_server");
        match mode {
            "integrated" => AuthMethod::Integrated,
            "windows" => {
                #[cfg(target_os = "windows")]
                {
                    AuthMethod::windows(self.config.username.clone(), self.config.password.clone())
                }
                #[cfg(not(target_os = "windows"))]
                {
                    AuthMethod::sql_server(
                        self.config.username.clone(),
                        self.config.password.clone(),
                    )
                }
            }
            "aad_token" => AuthMethod::aad_token(self.config.password.clone()),
            _ => AuthMethod::sql_server(self.config.username.clone(), self.config.password.clone()),
        }
    }

    async fn connect_single(&self) -> DriverResult<Client<Compat<TcpStream>>> {
        let attempts = if self.config.ssl {
            vec![
                (
                    EncryptionLevel::Required,
                    false,
                    "encrypt=required,trust_cert=false",
                ),
                (EncryptionLevel::On, false, "encrypt=on,trust_cert=false"),
            ]
        } else {
            vec![
                (EncryptionLevel::Off, false, "encrypt=off"),
                (
                    EncryptionLevel::NotSupported,
                    false,
                    "encrypt=not_supported",
                ),
                (EncryptionLevel::On, true, "encrypt=on,trust_cert=true"),
                (
                    EncryptionLevel::Required,
                    true,
                    "encrypt=required,trust_cert=true",
                ),
            ]
        };

        let mut errors = Vec::new();
        for (encryption, trust_cert, label) in attempts {
            let config = self.build_tiberius_config(encryption, trust_cert);
            match self.connect_with_config(config).await {
                Ok(client) => return Ok(client),
                Err(err) => errors.push(format!("{label}: {err}")),
            }
        }

        Err(conn_error(format!(
            "SQL Server handshake failed after retries: {}",
            errors.join(" | ")
        )))
    }

    async fn connect_with_config(&self, config: Config) -> DriverResult<Client<Compat<TcpStream>>> {
        let connect_future = async {
            let tcp = if let Some(port) = self.proxy_port {
                TcpStream::connect(("127.0.0.1", port))
                    .await
                    .map_err(|e| conn_error(e.to_string()))?
            } else {
                TcpStream::connect_named(&config)
                    .await
                    .map_err(|e| conn_error(e.to_string()))?
            };
            tcp.set_nodelay(true)
                .map_err(|e| conn_error(e.to_string()))?;
            Ok::<TcpStream, AppError>(tcp)
        };

        let tcp = tokio::time::timeout(std::time::Duration::from_secs(10), connect_future)
            .await
            .map_err(|_| conn_error("Connection timed out"))??;

        Client::connect(config, tcp.compat_write())
            .await
            .map_err(|e| conn_error(e.to_string()))
    }
}

#[async_trait::async_trait]
impl bb8::ManageConnection for MssqlConnectionManager {
    type Connection = Client<Compat<TcpStream>>;
    type Error = AppError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.connect_single().await
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.simple_query("SELECT 1")
            .await
            .map_err(|e| AppError::internal(format!("{}", e)))?;
        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

pub(crate) async fn connect(form: &ConnectionForm) -> DriverResult<MssqlDriver> {
    let mut cfg_form = form.clone();
    let mut network_tunnel = None;

    if crate::network::tunnel_enabled(form) {
        let tunnel = crate::network::start_network_tunnel(form).await?;
        cfg_form.host = Some("127.0.0.1".to_string());
        cfg_form.port = Some(tunnel.local_port as i64);
        network_tunnel = Some(tunnel);
    }

    let config = build_config(&cfg_form)?;
    let mut manager = MssqlConnectionManager::new(config);
    if crate::network::proxy_enabled(form) {
        manager.proxy_port = network_tunnel.as_ref().map(|tunnel| tunnel.local_port);
        manager.config.host = form.host.clone().unwrap_or_default();
    }
    let pool = Pool::builder()
        .max_size(10)
        .build(manager)
        .await
        .map_err(|e| conn_error(format!("Failed to create connection pool: {}", e)))?;

    let supports_for_json = {
        let mut client = pool.get().await.map_err(map_pool_error)?;
        detect_for_json_support(&mut client).await
    };
    let driver = MssqlDriver {
        pool,
        network_tunnel,
        supports_for_json,
    };
    driver.test_connection().await?;
    Ok(driver)
}

#[cfg(test)]
mod tests {
    use super::build_config;
    use crate::models::ConnectionForm;

    #[test]
    fn test_build_config_parses_named_instance() {
        let form = ConnectionForm {
            host: Some("myhost\\SQLEXPRESS".to_string()),
            port: Some(1433),
            database: Some("testdb".to_string()),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            ssl: Some(false),
            ..Default::default()
        };
        let config = build_config(&form).unwrap();
        assert_eq!(config.host, "myhost");
        assert_eq!(config.instance_name, Some("SQLEXPRESS".to_string()));
    }

    #[test]
    fn test_build_config_parses_plain_host() {
        let form = ConnectionForm {
            host: Some("localhost".to_string()),
            port: None,
            database: None,
            username: Some("sa".to_string()),
            password: Some("pw".to_string()),
            ..Default::default()
        };
        let config = build_config(&form).unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 1433);
        assert_eq!(config.database, "master");
        assert!(config.instance_name.is_none());
    }

    #[test]
    fn test_build_config_rejects_invalid_instance_format() {
        let form = ConnectionForm {
            host: Some("\\ ".to_string()),
            ..Default::default()
        };
        assert!(build_config(&form).is_err());
    }
}
