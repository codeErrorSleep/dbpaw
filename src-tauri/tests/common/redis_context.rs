mod shared;

use dbpaw_lib::models::ConnectionForm;
use std::sync::OnceLock;
use std::time::Duration;
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::{Container, GenericImage, RunnableImage};

#[allow(unused_imports)]
pub use shared::{should_reuse_local_db, unique_name};

static SHARED_NOAUTH: OnceLock<(&'static Container<'static, GenericImage>, ConnectionForm)> =
    OnceLock::new();
static SHARED_AUTH: OnceLock<(&'static Container<'static, GenericImage>, ConnectionForm)> =
    OnceLock::new();

fn redis_noauth_image() -> RunnableImage<GenericImage> {
    let image = GenericImage::new("redis", "7-alpine")
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .with_exposed_port(6379);
    RunnableImage::from(image)
}

fn redis_auth_image() -> RunnableImage<GenericImage> {
    // GenericImage::Args = Vec<String>; pass args as CMD so docker-entrypoint.sh
    // receives: redis-server --requirepass testpass123
    let image = GenericImage::new("redis", "7-alpine")
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .with_exposed_port(6379);
    RunnableImage::from((
        image,
        vec![
            "redis-server".to_string(),
            "--requirepass".to_string(),
            "testpass123".to_string(),
        ],
    ))
}

fn redis_form_from_env_noauth(host: &str, port: u16) -> ConnectionForm {
    ConnectionForm {
        driver: "redis".to_string(),
        host: Some(shared::env_or("REDIS_HOST", host)),
        port: Some(shared::env_i64("REDIS_PORT", i64::from(port))),
        ..Default::default()
    }
}

fn redis_form_from_env_auth(host: &str, port: u16) -> ConnectionForm {
    ConnectionForm {
        driver: "redis".to_string(),
        host: Some(shared::env_or("REDIS_AUTH_HOST", host)),
        port: Some(shared::env_i64("REDIS_AUTH_PORT", i64::from(port))),
        password: Some(shared::env_or("REDIS_PASSWORD", "testpass123")),
        ..Default::default()
    }
}

/// Return a `ConnectionForm` for a Redis instance with no authentication.
pub fn shared_redis_noauth_form() -> ConnectionForm {
    if should_reuse_local_db() {
        return redis_form_from_env_noauth("127.0.0.1", 6379);
    }
    shared::ensure_docker_available();

    let (_container, form) = SHARED_NOAUTH.get_or_init(|| {
        let cli: &'static Cli = Box::leak(Box::new(Cli::default()));
        let runnable =
            redis_noauth_image().with_container_name(shared::unique_container_name("redis-noauth"));
        let container: &'static Container<'static, GenericImage> =
            Box::leak(Box::new(cli.run(runnable)));
        let port = container.get_host_port_ipv4(6379);
        shared::wait_for_port("127.0.0.1", port, Duration::from_secs(30));
        (container, redis_form_from_env_noauth("127.0.0.1", port))
    });
    form.clone()
}

/// Return a `ConnectionForm` for a password-protected Redis instance.
pub fn shared_redis_auth_form() -> ConnectionForm {
    if should_reuse_local_db() {
        return redis_form_from_env_auth("127.0.0.1", 6380);
    }
    shared::ensure_docker_available();

    let (_container, form) = SHARED_AUTH.get_or_init(|| {
        let cli: &'static Cli = Box::leak(Box::new(Cli::default()));
        let runnable =
            redis_auth_image().with_container_name(shared::unique_container_name("redis-auth"));
        let container: &'static Container<'static, GenericImage> =
            Box::leak(Box::new(cli.run(runnable)));
        let port = container.get_host_port_ipv4(6379);
        shared::wait_for_port("127.0.0.1", port, Duration::from_secs(30));
        (container, redis_form_from_env_auth("127.0.0.1", port))
    });
    form.clone()
}

/// Build a form for ACL-style authentication (username + password).
#[allow(dead_code)]
pub fn redis_acl_form(host: &str, port: u16) -> ConnectionForm {
    ConnectionForm {
        driver: "redis".to_string(),
        host: Some(shared::env_or("REDIS_ACL_HOST", host)),
        port: Some(shared::env_i64("REDIS_ACL_PORT", i64::from(port))),
        username: Some(shared::env_or("REDIS_ACL_USER", "default")),
        password: Some(shared::env_or("REDIS_ACL_PASSWORD", "testpass123")),
        ..Default::default()
    }
}

/// Return an optional `ConnectionForm` for a Redis Cluster.
///
/// When `IT_REUSE_LOCAL_DB=1`, reads `REDIS_CLUSTER_HOSTS` from the environment.
/// If the variable is not set, returns `None` so callers can skip cluster tests.
///
/// This never starts containers via testcontainers — cluster setup is assumed to
/// be provided externally (e.g. `docker-compose.redis.yml`).
pub fn shared_redis_cluster_form() -> Option<ConnectionForm> {
    let hosts = std::env::var("REDIS_CLUSTER_HOSTS").ok()?;
    if hosts.is_empty() {
        return None;
    }
    Some(ConnectionForm {
        driver: "redis".to_string(),
        host: Some(hosts),
        ..Default::default()
    })
}
