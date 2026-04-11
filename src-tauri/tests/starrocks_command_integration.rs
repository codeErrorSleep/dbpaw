#[path = "common/starrocks_context.rs"]
mod starrocks_context;

use dbpaw_lib::commands::connection::{self, CreateDatabasePayload};
use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::db::local::LocalDb;
use dbpaw_lib::models::ConnectionForm;
use dbpaw_lib::state::AppState;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use testcontainers::clients::Cli;
use tokio::time::{sleep, Duration};

fn unique_name(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_millis();
    format!("{}_{}", prefix, millis)
}

async fn wait_until_starrocks_ready(form: &ConnectionForm) {
    let mut last_error = String::new();
    for _ in 0..90 {
        match connection::test_connection_ephemeral(form.clone()).await {
            Ok(_) => return,
            Err(err) => {
                last_error = err;
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("starrocks is not ready for command tests: {last_error}");
}

async fn init_state_with_local_db() -> AppState {
    let state = AppState::new();
    let local_db_dir = std::env::temp_dir().join(unique_name("dbpaw_starrocks_stateful_it"));
    let db = LocalDb::init_with_app_dir(&local_db_dir)
        .await
        .expect("failed to initialize local db");
    let mut lock = state.local_db.lock().await;
    *lock = Some(Arc::new(db));
    drop(lock);
    state
}

async fn create_starrocks_connection_for_state(
    state: &AppState,
    base_form: &ConnectionForm,
) -> i64 {
    let mut form = base_form.clone();
    form.name = Some(unique_name("starrocks-command"));
    let created = connection::create_connection_direct(state, form)
        .await
        .expect("create_connection should succeed");
    created.id
}

async fn drop_database_if_exists(form: &ConnectionForm, db_name: &str) {
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect starrocks driver for cleanup");
    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_create_database_by_id_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id = create_starrocks_connection_for_state(&state, &form).await;

    let db_name = unique_name("dbpaw_starrocks_cmd_db");
    let payload = CreateDatabasePayload {
        name: db_name.clone(),
        if_not_exists: Some(true),
        charset: None,
        collation: None,
        encoding: None,
        lc_collate: None,
        lc_ctype: None,
    };

    connection::create_database_by_id_direct(&state, conn_id, payload)
        .await
        .expect("create_database_by_id should succeed");

    let dbs = connection::list_databases_by_id_direct(&state, conn_id)
        .await
        .expect("list_databases_by_id should succeed");
    assert!(dbs.iter().any(|d| d == &db_name));

    drop_database_if_exists(&form, &db_name).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}
