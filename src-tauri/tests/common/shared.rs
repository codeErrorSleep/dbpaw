use std::env;
use std::future::Future;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
const CONNECT_RETRY_ATTEMPTS: usize = 20;
#[allow(dead_code)]
const CONNECT_RETRY_DELAY_MS: u64 = 500;

pub fn should_reuse_local_db() -> bool {
    env::var("IT_REUSE_LOCAL_DB")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn wait_for_port(host: &str, port: u16, timeout: Duration) {
    let started_at = Instant::now();
    let addr = resolve_socket_addr(host, port);

    while started_at.elapsed() < timeout {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
            return;
        }
        sleep(Duration::from_millis(500));
    }

    panic!(
        "timed out waiting for {}:{} to accept connections",
        host, port
    );
}

pub fn ensure_docker_available() {
    let output = Command::new("docker").arg("info").output().unwrap_or_else(|error| {
        panic!(
            "failed to run `docker info`: {}. Install/start Docker, or run with IT_REUSE_LOCAL_DB=1",
            error
        )
    });

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "`docker info` failed: {}. Start Docker daemon, or run with IT_REUSE_LOCAL_DB=1",
            stderr.trim()
        );
    }
}

pub fn unique_container_name(kind: &str) -> String {
    let prefix = env::var("IT_CONTAINER_PREFIX").unwrap_or_else(|_| "dbpaw-it-".to_string());
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    format!("{prefix}{kind}-{pid}-{ts}")
}

#[allow(dead_code)]
pub fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

#[allow(dead_code)]
pub fn env_or_any(names: &[&str], default: &str) -> String {
    env_any(names).unwrap_or_else(|| default.to_string())
}

#[allow(dead_code)]
pub fn env_any(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| env::var(name).ok())
}

#[allow(dead_code)]
pub fn env_i64(name: &str, default: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

#[allow(dead_code)]
pub fn env_i64_any(names: &[&str], default: i64) -> i64 {
    env_any(names)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

#[allow(dead_code)]
pub async fn connect_with_retry<T, F, Fut>(mut connect: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let mut last_error = String::new();
    for _ in 0..CONNECT_RETRY_ATTEMPTS {
        match connect().await {
            Ok(value) => return value,
            Err(err) => {
                last_error = err;
                tokio::time::sleep(Duration::from_millis(CONNECT_RETRY_DELAY_MS)).await;
            }
        }
    }
    panic!("Failed to connect after retries: {last_error}");
}

fn resolve_socket_addr(host: &str, port: u16) -> SocketAddr {
    (host, port)
        .to_socket_addrs()
        .expect("failed to resolve socket address")
        .next()
        .expect("resolved zero socket addresses")
}
