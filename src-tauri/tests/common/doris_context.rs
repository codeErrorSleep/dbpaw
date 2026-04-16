mod shared;

use dbpaw_lib::commands::connection;
use dbpaw_lib::models::ConnectionForm;
use std::sync::OnceLock;
use std::time::Duration;
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::{Container, GenericImage, RunnableImage};
use tokio::time::sleep;

#[allow(unused_imports)]
pub use shared::{connect_with_retry, should_reuse_local_db, unique_name};

// Shared container for the current test binary.  Started once on first use and
// kept alive until the process exits, so all tests in the binary pay the
// container startup cost only once (~25 s per binary instead of per test).
//
// Tests that need their own isolated container lifetime (e.g. server-shutdown
// scenarios) should call `doris_form_from_test_context` with a local `Cli`.
static SHARED_CONTAINER: OnceLock<(&'static Container<'static, GenericImage>, ConnectionForm)> =
    OnceLock::new();

/// Build the standard Apache Doris test image.
fn doris_image() -> RunnableImage<GenericImage> {
    let image = GenericImage::new("apache/doris", "doris-all-in-one-2.1.7")
        .with_wait_for(WaitFor::seconds(25))
        .with_exposed_port(9030);
    RunnableImage::from(image)
}

/// Build a `ConnectionForm` from environment variables, falling back to
/// sensible defaults for local development.
fn doris_form_from_env(host: &str, port: u16) -> ConnectionForm {
    ConnectionForm {
        driver: "doris".to_string(),
        host: Some(shared::env_or("DORIS_HOST", host)),
        port: Some(shared::env_i64("DORIS_PORT", i64::from(port))),
        username: Some(shared::env_or("DORIS_USER", "root")),
        password: Some(shared::env_or("DORIS_PASSWORD", "")),
        database: std::env::var("DORIS_DB").ok(),
        ..Default::default()
    }
}

/// Return a `ConnectionForm` backed by the shared Doris container for this
/// binary.  The container is started on first call; subsequent calls reuse it.
/// Use this in the vast majority of tests.
pub fn shared_doris_form() -> ConnectionForm {
    if should_reuse_local_db() {
        return doris_form_from_env("localhost", 9030);
    }
    shared::ensure_docker_available();

    let (_container, form) = SHARED_CONTAINER.get_or_init(|| {
        let cli: &'static Cli = Box::leak(Box::new(Cli::default()));
        let runnable = doris_image()
            .with_container_name(shared::unique_container_name("doris-shared"));
        let container: &'static Container<'static, GenericImage> =
            Box::leak(Box::new(cli.run(runnable)));
        let port = container.get_host_port_ipv4(9030);
        shared::wait_for_port("127.0.0.1", port, Duration::from_secs(180));
        (container, doris_form_from_env("127.0.0.1", port))
    });
    form.clone()
}

/// Start an isolated Doris container and return it together with a matching
/// `ConnectionForm`.  Use only when the test itself needs to control the
/// container lifetime (e.g. to test behaviour after server shutdown).
#[allow(dead_code)]
pub fn doris_form_from_test_context<'a>(
    docker: Option<&'a Cli>,
) -> (Option<Container<'a, GenericImage>>, ConnectionForm) {
    if should_reuse_local_db() {
        return (None, doris_form_from_env("localhost", 9030));
    }
    shared::ensure_docker_available();

    let docker = docker.expect("docker client is required when IT_REUSE_LOCAL_DB is not set");
    let runnable = doris_image().with_container_name(shared::unique_container_name("doris"));
    let container = docker.run(runnable);
    let port = container.get_host_port_ipv4(9030);

    shared::wait_for_port("127.0.0.1", port, Duration::from_secs(180));

    let mut form = doris_form_from_env("127.0.0.1", port);
    apply_doris_env_overrides(&mut form);
    (Some(container), form)
}

fn apply_doris_env_overrides(form: &mut ConnectionForm) {
    if let Ok(host) = std::env::var("DORIS_HOST") {
        form.host = Some(host);
    }
    if let Ok(port) = std::env::var("DORIS_PORT") {
        form.port = Some(
            port.parse::<i64>()
                .expect("DORIS_PORT should be a valid number"),
        );
    }
    if let Ok(user) = std::env::var("DORIS_USER") {
        form.username = Some(user);
    }
    if let Ok(password) = std::env::var("DORIS_PASSWORD") {
        form.password = Some(password);
    }
    if let Ok(database) = std::env::var("DORIS_DB") {
        form.database = Some(database);
    }
}

/// Poll the Doris server until it accepts connections or the timeout expires.
/// Uses the application-level `test_connection_ephemeral` command so it
/// validates the full stack, not just TCP reachability.
#[allow(dead_code)]
pub async fn wait_until_ready(form: &ConnectionForm) {
    let mut last_error = String::new();
    for _ in 0..180 {
        match connection::test_connection_ephemeral(form.clone()).await {
            Ok(_) => return,
            Err(err) => {
                last_error = err;
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("Doris at {}:{} did not become ready in time: {last_error}",
        form.host.as_deref().unwrap_or("?"),
        form.port.unwrap_or(9030));
}
