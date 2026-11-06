use crate::db::drivers::conn_failed_error;
use crate::error::AppError;
use crate::models::{NetworkProxy, ProxyKind};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::io;
use std::net::IpAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::{JoinHandle, JoinSet};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct ProxyTask(JoinHandle<()>);

impl Drop for ProxyTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(super) async fn start(
    config: NetworkProxy,
    host: String,
    port: u16,
) -> Result<(u16, ProxyTask), AppError> {
    // Establish the first upstream connection now so proxy/auth failures reach the caller.
    let first = connect(&config, &host, port).await?;
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| conn_failed_error(&e))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| conn_failed_error(&e))?
        .port();
    let task = tokio::spawn(async move {
        let mut first = Some(first);
        let mut forwards = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((mut local, _)) = accepted else { break };
                    let config = config.clone();
                    let host = host.clone();
                    let upstream = first.take();
                    forwards.spawn(async move {
                        let remote = match upstream {
                            Some(remote) => Ok(remote),
                            None => connect(&config, &host, port).await,
                        };
                        match remote {
                            Ok(mut remote) => { let _ = tokio::io::copy_bidirectional(&mut local, &mut remote).await; }
                            Err(error) => tracing::warn!(code = error.code(), "Network proxy connection failed"),
                        }
                    });
                }
                _ = forwards.join_next(), if !forwards.is_empty() => {}
            }
        }
        // Dropping JoinSet aborts all live forwarding tasks along with the listener.
    });
    Ok((local_port, ProxyTask(task)))
}

async fn connect(config: &NetworkProxy, host: &str, port: u16) -> Result<TcpStream, AppError> {
    tokio::time::timeout(CONNECT_TIMEOUT, async {
        let proxy_host = config.host.trim_matches(['[', ']']);
        let mut stream = TcpStream::connect((proxy_host, config.port as u16))
            .await
            .map_err(|e| conn_failed_error(&format!("Network proxy connection failed: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| conn_failed_error(&e))?;
        match config.kind {
            ProxyKind::Socks5 => socks5(&mut stream, config, host, port).await?,
            ProxyKind::Http => http_connect(&mut stream, config, host, port).await?,
        }
        Ok(stream)
    })
    .await
    .map_err(|_| conn_failed_error(&"Network proxy connection timed out"))?
}

fn protocol_error(message: &str) -> AppError {
    conn_failed_error(&format!("Network proxy: {message}"))
}

async fn socks5(
    stream: &mut TcpStream,
    config: &NetworkProxy,
    host: &str,
    port: u16,
) -> Result<(), AppError> {
    let authenticated = !config.username.is_empty();
    let method = if authenticated { 2 } else { 0 };
    stream.write_all(&[5, 1, method]).await.map_err(io_error)?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply).await.map_err(io_error)?;
    if reply != [5, method] {
        return Err(AppError::conn_auth_failed(
            "SOCKS5 proxy rejected the authentication method",
        ));
    }
    if authenticated {
        let mut auth = vec![1, config.username.len() as u8];
        auth.extend_from_slice(config.username.as_bytes());
        auth.push(config.password.len() as u8);
        auth.extend_from_slice(config.password.as_bytes());
        stream.write_all(&auth).await.map_err(io_error)?;
        stream.read_exact(&mut reply).await.map_err(io_error)?;
        if reply != [1, 0] {
            return Err(AppError::conn_auth_failed(
                "SOCKS5 proxy authentication failed",
            ));
        }
    }
    let host = host.trim_matches(['[', ']']);
    let mut request = vec![5, 1, 0];
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            request.push(1);
            request.extend_from_slice(&ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            request.push(4);
            request.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            if host.is_empty() || host.len() > 255 {
                return Err(AppError::validation("Invalid SOCKS5 target hostname"));
            }
            request.extend_from_slice(&[3, host.len() as u8]);
            request.extend_from_slice(host.as_bytes());
        }
    }
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await.map_err(io_error)?;
    let mut response = [0; 4];
    stream.read_exact(&mut response).await.map_err(io_error)?;
    if response[0] != 5 || response[2] != 0 {
        return Err(protocol_error("Invalid SOCKS5 reply"));
    }
    if response[1] != 0 {
        return Err(protocol_error(&format!(
            "SOCKS5 CONNECT rejected (code {})",
            response[1]
        )));
    }
    let address_len = match response[3] {
        1 => 4,
        4 => 16,
        3 => stream.read_u8().await.map_err(io_error)? as usize,
        _ => return Err(protocol_error("Invalid SOCKS5 address type")),
    };
    let mut bound_address = vec![0; address_len + 2];
    stream
        .read_exact(&mut bound_address)
        .await
        .map_err(io_error)?;
    Ok(())
}

async fn http_connect(
    stream: &mut TcpStream,
    config: &NetworkProxy,
    host: &str,
    port: u16,
) -> Result<(), AppError> {
    if host.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::validation("Invalid proxy target host"));
    }
    let host = host.trim_matches(['[', ']']);
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if !config.username.is_empty() {
        let credentials = STANDARD.encode(format!("{}:{}", config.username, config.password));
        request.push_str(&format!("Proxy-Authorization: Basic {credentials}\r\n"));
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(io_error)?;
    // Read exactly the header so server greetings immediately following it are preserved.
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= 16 * 1024 {
            return Err(protocol_error("HTTP CONNECT reply is too large"));
        }
        header.push(stream.read_u8().await.map_err(io_error)?);
    }
    let line = header
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let line = String::from_utf8_lossy(line);
    let mut parts = line.split_whitespace();
    let version = parts.next().unwrap_or_default();
    let status = parts.next().unwrap_or_default();
    if status == "407" {
        return Err(AppError::conn_auth_failed(
            "HTTP proxy authentication failed",
        ));
    }
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || status.len() != 3
        || !status.bytes().all(|b| b.is_ascii_digit())
        || !status.starts_with('2')
    {
        return Err(protocol_error(
            "HTTP proxy rejected CONNECT; check proxy access rules and target port",
        ));
    }
    Ok(())
}

fn io_error(error: io::Error) -> AppError {
    conn_failed_error(&format!("Network proxy handshake failed: {error}"))
}
