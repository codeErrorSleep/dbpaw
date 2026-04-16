#[path = "common/starrocks_context.rs"]
mod starrocks_context;

use dbpaw_lib::commands::connection::{self, CreateDatabasePayload};
use dbpaw_lib::commands::{metadata, query};
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

async fn prepare_query_test_table(form: &ConnectionForm, db_name: &str, table: &str) {
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect starrocks driver");
    let qualified = format!("`{}`.`{}`", db_name, table);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT, name VARCHAR(64))",
            qualified
        ))
        .await
        .expect("create table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'DbPaw')",
            qualified
        ))
        .await
        .expect("insert row should succeed");
    driver.close().await;
}

async fn cleanup_table(form: &ConnectionForm, db_name: &str, table: &str) {
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect starrocks driver for cleanup");
    let qualified = format!("`{}`.`{}`", db_name, table);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver.close().await;
}

async fn execute_by_conn_sql(
    form: ConnectionForm,
    sql: String,
) -> Result<dbpaw_lib::models::QueryResult, String> {
    query::execute_by_conn_direct(form, sql).await
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_test_connection_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let result = connection::test_connection_ephemeral(form)
        .await
        .expect("test_connection_ephemeral should succeed");

    assert!(result.success);
    assert!(result.latency_ms.is_some());
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_test_connection_invalid_password_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let mut bad_form = form.clone();
    bad_form.password = Some("dbpaw_wrong_password".to_string());

    let result = connection::test_connection_ephemeral(bad_form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_list_databases_contains_target_db() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let databases = connection::list_databases(form)
        .await
        .expect("list_databases should succeed");

    assert!(!databases.is_empty());
    assert!(databases.iter().all(|db| !db.trim().is_empty()));
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_list_databases_invalid_credentials_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let mut bad_form = form.clone();
    bad_form.password = Some("dbpaw_wrong_password".to_string());

    let result = connection::list_databases(bad_form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_list_tables_by_conn_contains_created_table() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let db_name = unique_name("dbpaw_starrocks_tables_db");
    let table = unique_name("dbpaw_cmd_tables");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());
    prepare_query_test_table(&form_with_db, &db_name, &table).await;

    let tables = metadata::list_tables_by_conn(form_with_db.clone())
        .await
        .expect("list_tables_by_conn should succeed");

    assert!(tables.iter().any(|t| t.name == table));
    cleanup_table(&form_with_db, &db_name, &table).await;

    let _ = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect for cleanup")
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_list_tables_by_conn_invalid_credentials_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let mut bad_form = form.clone();
    bad_form.password = Some("dbpaw_wrong_password".to_string());

    let result = metadata::list_tables_by_conn(bad_form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_execute_by_conn_select_returns_rows() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let db_name = unique_name("dbpaw_starrocks_select_db");
    let table = unique_name("dbpaw_cmd_exec_select");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());
    prepare_query_test_table(&form_with_db, &db_name, &table).await;

    let sql = format!("SELECT id, name FROM `{}`.`{}` ORDER BY id", db_name, table);
    let result = execute_by_conn_sql(form_with_db.clone(), sql)
        .await
        .expect("execute_by_conn should succeed");

    assert!(result.success);
    assert!(result.row_count >= 1);
    assert!(!result.data.is_empty());
    let row = result.data.first().expect("result row should exist");
    let name = row.get("name").and_then(|v| v.as_str());
    assert_eq!(name, Some("DbPaw"));

    cleanup_table(&form_with_db, &db_name, &table).await;
    let _ = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect for cleanup")
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_execute_by_conn_invalid_sql_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let db_name = unique_name("dbpaw_starrocks_invalid_db");
    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let result = execute_by_conn_sql(
        form_with_db,
        "SELECT * FROM __dbpaw_missing_command_table".to_string(),
    )
    .await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_execute_by_conn_insert_affects_rows() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let db_name = unique_name("dbpaw_starrocks_insert_db");
    let table = unique_name("dbpaw_cmd_exec_insert");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT, name VARCHAR(64))",
            db_name, table
        ))
        .await
        .expect("create table should succeed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let sql = format!(
        "INSERT INTO `{}`.`{}` (id, name) VALUES (1, 'alpha')",
        db_name, table
    );
    let result = execute_by_conn_sql(form_with_db.clone(), sql)
        .await
        .expect("execute_by_conn insert should succeed");
    assert!(result.success);
    assert_eq!(result.row_count, 1);

    cleanup_table(&form_with_db, &db_name, &table).await;
    let _ = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect for cleanup")
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_get_table_data_by_conn_pagination_works() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let db_name = unique_name("dbpaw_starrocks_page_db");
    let table = unique_name("dbpaw_cmd_page");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT, name VARCHAR(64))",
            db_name, table
        ))
        .await
        .expect("create table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO `{}`.`{}` (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')",
            db_name, table
        ))
        .await
        .expect("insert rows should succeed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let page1 = query::get_table_data_by_conn(
        form_with_db.clone(),
        db_name.clone(),
        table.clone(),
        1,
        2,
    )
    .await
    .expect("page 1 should succeed");
    let page2 = query::get_table_data_by_conn(
        form_with_db.clone(),
        db_name.clone(),
        table.clone(),
        2,
        2,
    )
    .await
    .expect("page 2 should succeed");

    assert_eq!(page1.total, 3);
    assert_eq!(page1.limit, 2);
    assert_eq!(page1.page, 1);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page2.data.len(), 1);

    cleanup_table(&form_with_db, &db_name, &table).await;
    let _ = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect for cleanup")
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_get_table_data_by_conn_invalid_pagination_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let db_name = unique_name("dbpaw_starrocks_invalid_page_db");
    let table = unique_name("dbpaw_cmd_invalid_page");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());
    prepare_query_test_table(&form_with_db, &db_name, &table).await;

    let result = query::get_table_data_by_conn(
        form_with_db.clone(),
        db_name.clone(),
        table.clone(),
        0,
        10,
    )
    .await;
    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(error.contains("[VALIDATION_ERROR]"));

    cleanup_table(&form_with_db, &db_name, &table).await;
    let _ = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect for cleanup")
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
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
