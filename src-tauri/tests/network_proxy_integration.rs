#[path = "common/shared.rs"]
mod shared;

use dbpaw_lib::commands::connection;
use dbpaw_lib::db::drivers;
use dbpaw_lib::db::local::LocalDb;
use dbpaw_lib::models::{ConnectionForm, NetworkProxy, ProxyKind};
use dbpaw_lib::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::clients::Cli;
use testcontainers::core::WaitFor;
use testcontainers::{GenericImage, RunnableImage};

const PROXY_USER: &str = "proxy-user";
const PROXY_PASSWORD: &str = "proxy-test-password";

struct TempDirectory(PathBuf);

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn proxy_config(kind: ProxyKind, port: u16, authenticated: bool) -> NetworkProxy {
    NetworkProxy {
        enabled: true,
        kind,
        host: "127.0.0.1".into(),
        port: i64::from(port),
        username: if authenticated {
            PROXY_USER.into()
        } else {
            String::new()
        },
        password: if authenticated {
            PROXY_PASSWORD.into()
        } else {
            String::new()
        },
    }
}

async fn wait_until_ready(form: &ConnectionForm) {
    let mut last_error = String::new();
    for _ in 0..60 {
        match connection::test_connection_ephemeral(form.clone()).await {
            Ok(_) => return,
            Err(error) => last_error = error.to_string(),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!(
        "{} did not become ready through the proxy: {last_error}",
        form.driver
    );
}

async fn verify_database(form: &ConnectionForm) {
    connection::test_connection_ephemeral(form.clone())
        .await
        .expect("test connection through real proxy");
    if form.driver == "redis" {
        let mut conn = dbpaw_lib::datasources::redis::connect(form, None)
            .await
            .expect("connect Redis through real proxy");
        conn.query::<()>(
            redis::cmd("SET")
                .arg("dbpaw:proxy:probe")
                .arg("via-proxy")
                .clone(),
        )
        .await
        .unwrap();
        let value = conn
            .query::<String>(redis::cmd("GET").arg("dbpaw:proxy:probe").clone())
            .await
            .unwrap();
        assert_eq!(value, "via-proxy");
        conn.query::<usize>(redis::cmd("DEL").arg("dbpaw:proxy:probe").clone())
            .await
            .unwrap();
    } else {
        let driver = drivers::connect(form)
            .await
            .expect("create database driver through real proxy");
        let result = driver
            .execute_query("SELECT 'via-proxy' AS proxy_probe".into())
            .await
            .unwrap();
        assert_eq!(result.data[0]["proxy_probe"].as_str(), Some("via-proxy"));
        let databases = driver.list_databases().await.unwrap();
        assert!(databases.iter().any(|database| database == "test_db"));
        if form.driver == "mysql" {
            let status = driver
                .execute_query("SHOW SESSION STATUS LIKE 'Ssl_cipher'".into())
                .await
                .unwrap();
            assert!(
                !status.data[0]["Value"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty(),
                "MySQL proxy connection must retain TLS"
            );
        }
        driver.close().await;
    }
}

async fn verify_saved_connection(form: &ConnectionForm) {
    let temp = TempDirectory(std::env::temp_dir().join(shared::unique_name("dbpaw_proxy_it")));
    let db = Arc::new(LocalDb::init_with_app_dir(&temp.0).await.unwrap());
    let state = AppState::new();
    *state.local_db.lock().await = Some(db.clone());
    let mut form = form.clone();
    form.name = Some("saved-proxy-test".into());
    let created = connection::create_connection_direct(&state, form)
        .await
        .unwrap();
    assert!(created.proxy.as_ref().unwrap().password.is_empty());
    let mut edit = db.get_connection_form_by_id(created.id).await.unwrap();
    edit.proxy.as_mut().unwrap().password.clear();
    connection::update_connection_direct(&state, created.id, edit)
        .await
        .unwrap();
    let loaded = db.get_connection_form_by_id(created.id).await.unwrap();
    assert_eq!(loaded.proxy.as_ref().unwrap().password, PROXY_PASSWORD);
    let result = dbpaw_lib::services::query_service::execute_query(
        &state,
        created.id,
        "SELECT 'via-proxy' AS proxy_probe".into(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.data[0]["proxy_probe"].as_str(), Some("via-proxy"));
    state.pool_manager.close_all().await;
    drop(state);
    drop(db);
}

#[tokio::test]
#[ignore]
async fn real_databases_through_http_and_socks5_proxies() {
    assert_eq!(
        std::env::var("IT_DB").as_deref(),
        Ok("proxy"),
        "Run with IT_DB=proxy bun run test:integration"
    );
    if shared::should_reuse_local_db() {
        println!("Skipping proxy topology test: it requires isolated Docker containers; unset IT_REUSE_LOCAL_DB to run it");
        return;
    }
    shared::ensure_docker_available();
    let docker = Cli::default();
    let prefix = shared::unique_container_name("proxy");
    let network = format!("{prefix}-network");
    let mysql_host = format!("{prefix}-mysql");
    let redis_host = format!("{prefix}-redis");
    let clickhouse_host = format!("{prefix}-clickhouse");

    let _mysql = docker.run(
        RunnableImage::from(
            GenericImage::new("mysql", "8.4")
                .with_env_var("MYSQL_ROOT_PASSWORD", "db-test-password")
                .with_env_var("MYSQL_ROOT_HOST", "%")
                .with_env_var("MYSQL_DATABASE", "test_db"),
        )
        .with_network(&network)
        .with_container_name(&mysql_host),
    );
    let _redis = docker.run(
        RunnableImage::from((
            GenericImage::new("redis", "7-alpine"),
            vec![
                "redis-server".into(),
                "--requirepass".into(),
                "db-test-password".into(),
            ],
        ))
        .with_network(&network)
        .with_container_name(&redis_host),
    );
    let _clickhouse = docker.run(
        RunnableImage::from(
            GenericImage::new("clickhouse/clickhouse-server", "24.3")
                .with_env_var("CLICKHOUSE_USER", "dbpaw")
                .with_env_var("CLICKHOUSE_PASSWORD", "db-test-password")
                .with_env_var("CLICKHOUSE_DB", "test_db"),
        )
        .with_network(&network)
        .with_container_name(&clickhouse_host),
    );
    let proxy_image = GenericImage::new("ginuerzh/gost", "2.12.0")
        .with_exposed_port(1080)
        .with_exposed_port(1081)
        .with_exposed_port(8080)
        .with_exposed_port(8081)
        .with_wait_for(WaitFor::seconds(1));
    let proxy = docker.run(
        RunnableImage::from((
            proxy_image,
            vec![
                "-L=socks5://:1080".into(),
                format!("-L=socks5://{PROXY_USER}:{PROXY_PASSWORD}@:1081"),
                "-L=http://:8080".into(),
                format!("-L=http://{PROXY_USER}:{PROXY_PASSWORD}@:8081"),
            ],
        ))
        .with_network(&network)
        .with_container_name(format!("{prefix}-gost")),
    );
    let endpoints = [
        (ProxyKind::Socks5, proxy.get_host_port_ipv4(1080), false),
        (ProxyKind::Socks5, proxy.get_host_port_ipv4(1081), true),
        (ProxyKind::Http, proxy.get_host_port_ipv4(8080), false),
        (ProxyKind::Http, proxy.get_host_port_ipv4(8081), true),
    ];
    let forms = [
        ConnectionForm {
            driver: "mysql".into(),
            host: Some(mysql_host),
            port: Some(3306),
            username: Some("root".into()),
            password: Some("db-test-password".into()),
            database: Some("test_db".into()),
            ssl: Some(true),
            ..Default::default()
        },
        ConnectionForm {
            driver: "redis".into(),
            host: Some(redis_host.clone()),
            port: Some(6379),
            password: Some("db-test-password".into()),
            mode: Some("standalone".into()),
            seed_nodes: Some(vec![format!("{redis_host}:6379")]),
            ..Default::default()
        },
        ConnectionForm {
            driver: "clickhouse".into(),
            host: Some(clickhouse_host),
            port: Some(8123),
            username: Some("dbpaw".into()),
            password: Some("db-test-password".into()),
            database: Some("test_db".into()),
            ..Default::default()
        },
    ];

    for base in &forms {
        // These Docker-only names are deliberately unresolvable from the app host.
        assert!(tokio::net::lookup_host((
            base.host.as_deref().unwrap(),
            base.port.unwrap() as u16
        ))
        .await
        .is_err());
        let direct = tokio::time::timeout(
            Duration::from_secs(5),
            connection::test_connection_ephemeral(base.clone()),
        )
        .await;
        assert!(
            !matches!(direct, Ok(Ok(_))),
            "direct connection unexpectedly succeeded"
        );
        println!("PASS {}: direct connection fails", base.driver);
        for (kind, port, authenticated) in endpoints {
            let mut form = base.clone();
            form.proxy = Some(proxy_config(kind, port, authenticated));
            wait_until_ready(&form).await;
            verify_database(&form).await;
            println!(
                "PASS {}: {kind:?}, authenticated={authenticated}, real query and database listing",
                base.driver
            );
        }
    }
    let mut saved_form = forms[0].clone();
    saved_form.proxy = Some(proxy_config(endpoints[1].0, endpoints[1].1, true));
    verify_saved_connection(&saved_form).await;
    println!(
        "PASS saved MySQL: credentials retained after edit and saved connection executes query"
    );

    for (kind, port, authenticated) in endpoints {
        if !authenticated {
            continue;
        }
        let mut bad_auth = forms[0].clone();
        let mut config = proxy_config(kind, port, true);
        config.password = "wrong-proxy-password".into();
        bad_auth.proxy = Some(config);
        let error = connection::test_connection_ephemeral(bad_auth)
            .await
            .err()
            .expect("bad proxy credentials must fail");
        assert_eq!(error.code(), dbpaw_lib::error::codes::CONN_AUTH_FAILED);
        println!("PASS {kind:?}: incorrect proxy credentials rejected");
    }
    drop(proxy);
    for base in forms {
        let mut form = base;
        form.proxy = Some(proxy_config(endpoints[0].0, endpoints[0].1, false));
        assert!(
            connection::test_connection_ephemeral(form.clone())
                .await
                .is_err(),
            "must not fall back to direct connection"
        );
        println!(
            "PASS {}: stopped proxy fails without direct fallback",
            form.driver
        );
    }
}
