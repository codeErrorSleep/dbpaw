mod proxy;

use crate::error::AppError;
use crate::models::{ConnectionForm, NetworkProxy, ProxyKind};
use crate::ssh::SshTunnel;
use std::sync::Arc;

#[derive(Clone)]
pub struct NetworkTunnel {
    pub local_port: u16,
    _ssh: Option<SshTunnel>,
    _proxy: Option<Arc<proxy::ProxyTask>>,
}

pub fn proxy_enabled(form: &ConnectionForm) -> bool {
    form.proxy.as_ref().is_some_and(|proxy| proxy.enabled)
}

pub fn tunnel_enabled(form: &ConnectionForm) -> bool {
    form.ssh_enabled.unwrap_or(false) || proxy_enabled(form)
}

pub fn validate_proxy(form: &ConnectionForm) -> Result<(), AppError> {
    let Some(proxy) = form.proxy.as_ref().filter(|proxy| proxy.enabled) else {
        return Ok(());
    };
    if matches!(form.driver.as_str(), "sqlite" | "duckdb" | "cassandra") {
        return Err(AppError::unsupported(
            "Network proxy is not supported for this database type",
        ));
    }
    if form.driver == "redis" && form.mode.as_deref().unwrap_or("standalone") != "standalone" {
        return Err(AppError::unsupported(
            "Network proxy currently supports standalone Redis only",
        ));
    }
    if matches!(form.driver.as_str(), "mongodb" | "redis") && form.ssl.unwrap_or(false) {
        return Err(AppError::unsupported(
            "Network proxy with TLS is not supported for MongoDB or Redis yet",
        ));
    }
    if form.ssh_enabled.unwrap_or(false) {
        return Err(AppError::validation(
            "Use either an SSH tunnel or a network proxy for this connection",
        ));
    }
    if proxy.host.trim().is_empty()
        || proxy
            .host
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '@' | '\\' | '?' | '#'))
    {
        return Err(AppError::validation(
            "Proxy host must be a hostname or IP address without a URL scheme",
        ));
    }
    if !(1..=65535).contains(&proxy.port) {
        return Err(AppError::validation(
            "Proxy port must be between 1 and 65535",
        ));
    }
    if proxy.username.is_empty() && !proxy.password.is_empty() {
        return Err(AppError::validation(
            "Proxy username is required when a password is provided",
        ));
    }
    if proxy.kind == ProxyKind::Socks5 && (proxy.username.len() > 255 || proxy.password.len() > 255)
    {
        return Err(AppError::validation(
            "SOCKS5 credentials must not exceed 255 bytes",
        ));
    }
    if form.driver == "mssql" && form.host.as_deref().is_some_and(|host| host.contains('\\')) {
        return Err(AppError::validation(
            "Use the SQL Server host and explicit TCP port with a network proxy",
        ));
    }
    Ok(())
}

pub fn merge_proxy_password(form: &mut ConnectionForm, saved: &ConnectionForm) {
    if let (Some(proxy), Some(saved)) = (form.proxy.as_mut(), saved.proxy.as_ref()) {
        if proxy.password.is_empty()
            && proxy.kind == saved.kind
            && proxy.host == saved.host
            && proxy.port == saved.port
            && proxy.username == saved.username
        {
            proxy.password.clone_from(&saved.password);
        }
    }
}

pub async fn start_network_tunnel(form: &ConnectionForm) -> Result<NetworkTunnel, AppError> {
    validate_proxy(form)?;
    if let Some(config) = form.proxy.as_ref().filter(|proxy| proxy.enabled) {
        let host = form
            .host
            .as_deref()
            .ok_or_else(|| AppError::validation("Target host is required"))?;
        let default_port = match form.driver.as_str() {
            "mysql" | "mariadb" | "tidb" => 3306,
            "starrocks" | "doris" => 9030,
            "postgres" => 5432,
            "clickhouse" => 8123,
            "mssql" => 1433,
            "oracle" => 1521,
            "db2" => 50000,
            "mongodb" => 27017,
            "redis" => 6379,
            _ => {
                return Err(AppError::unsupported(
                    "Network tunnel is not supported for this driver",
                ))
            }
        };
        let port = form.port.unwrap_or(default_port);
        let port = u16::try_from(port)
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| AppError::validation("Target port must be between 1 and 65535"))?;
        let (local_port, task) = proxy::start(config.clone(), host.to_string(), port).await?;
        return Ok(NetworkTunnel {
            local_port,
            _ssh: None,
            _proxy: Some(Arc::new(task)),
        });
    }
    let ssh = crate::ssh::start_ssh_tunnel(form)?;
    Ok(NetworkTunnel {
        local_port: ssh.local_port,
        _ssh: Some(ssh),
        _proxy: None,
    })
}

pub fn reqwest_proxy(config: &NetworkProxy) -> Result<reqwest::Proxy, AppError> {
    let scheme = match config.kind {
        ProxyKind::Http => "http",
        ProxyKind::Socks5 => "socks5h",
    };
    let host = config.host.trim_matches(['[', ']']);
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let mut proxy = reqwest::Proxy::all(format!("{scheme}://{host}:{}", config.port))
        .map_err(|_| AppError::validation("Invalid network proxy address"))?;
    if !config.username.is_empty() {
        proxy = proxy.basic_auth(&config.username, &config.password);
    }
    Ok(proxy)
}

#[cfg(test)]
mod tests;
