#[path = "common/clickhouse_context.rs"]
mod clickhouse_context;

use dbpaw_lib::commands::{connection, metadata, query};
use dbpaw_lib::db::drivers::clickhouse::ClickHouseDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::time::{SystemTime, UNIX_EPOCH};
use testcontainers::clients::Cli;
use tokio::time::{sleep, Duration};

fn unique_table_name(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_millis();
    format!("{}_{}", prefix, millis)
}

async fn wait_until_clickhouse_ready(form: &ConnectionForm) {
    let mut last_error = String::new();
    for _ in 0..45 {
        let probe = form.clone();
        match connection::test_connection_ephemeral(probe).await {
            Ok(_) => return,
            Err(err) => {
                last_error = err;
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("clickhouse is not ready for command tests: {last_error}");
}

async fn prepare_query_test_table(form: &ConnectionForm, table: &str) {
    let driver = ClickHouseDriver::connect(form)
        .await
        .expect("failed to connect clickhouse driver");

    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let qualified = format!("`{}`.`{}`", database, table);

    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await
        .ok();
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id UInt32, name String) ENGINE = MergeTree ORDER BY id",
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

async fn cleanup_table(form: &ConnectionForm, table: &str) {
    let driver = ClickHouseDriver::connect(form)
        .await
        .expect("failed to connect clickhouse driver for cleanup");

    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let qualified = format!("`{}`.`{}`", database, table);

    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await
        .ok();
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
async fn test_clickhouse_command_test_connection_success() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;

    let result = connection::test_connection_ephemeral(form)
        .await
        .expect("test_connection_ephemeral should succeed");

    assert!(result.success);
    assert!(result.latency_ms.is_some());
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_test_connection_invalid_password_returns_error() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, mut form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    let ready_form = form.clone();
    wait_until_clickhouse_ready(&ready_form).await;
    form.password = Some("dbpaw_wrong_password".to_string());

    let result = connection::test_connection_ephemeral(form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_list_tables_by_conn_contains_created_table() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;
    let table = unique_table_name("dbpaw_cmd_tables");
    prepare_query_test_table(&form, &table).await;

    let tables = metadata::list_tables_by_conn(form.clone())
        .await
        .expect("list_tables_by_conn should succeed");

    assert!(tables.iter().any(|t| t.name == table));
    cleanup_table(&form, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_list_tables_by_conn_invalid_credentials_returns_error() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, mut form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    let ready_form = form.clone();
    wait_until_clickhouse_ready(&ready_form).await;
    form.password = Some("dbpaw_wrong_password".to_string());

    let result = metadata::list_tables_by_conn(form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_list_databases_contains_target_db() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;
    let target_db = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());

    let databases = connection::list_databases(form)
        .await
        .expect("list_databases should succeed");

    assert!(!databases.is_empty());
    assert!(databases.iter().any(|db| db == &target_db));
    assert!(databases.iter().all(|db| !db.trim().is_empty()));
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_list_databases_invalid_credentials_returns_error() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, mut form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    let ready_form = form.clone();
    wait_until_clickhouse_ready(&ready_form).await;
    form.password = Some("dbpaw_wrong_password".to_string());

    let result = connection::list_databases(form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_execute_by_conn_select_returns_rows() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;
    let table = unique_table_name("dbpaw_cmd_exec_select");
    prepare_query_test_table(&form, &table).await;

    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let qualified = format!("`{}`.`{}`", database, table);

    let sql = format!("SELECT id, name FROM {} ORDER BY id", qualified);
    let result = execute_by_conn_sql(form.clone(), sql)
        .await
        .expect("execute_by_conn should succeed");

    assert!(result.success);
    assert!(result.row_count >= 1);
    assert!(!result.data.is_empty());
    let row = result.data.first().expect("result row should exist");
    let name = row.get("name").and_then(|v| v.as_str());
    assert_eq!(name, Some("DbPaw"));
    cleanup_table(&form, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_execute_by_conn_invalid_sql_returns_error() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;

    let result = execute_by_conn_sql(
        form,
        "SELECT * FROM __dbpaw_missing_command_table".to_string(),
    )
    .await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_execute_by_conn_insert_affects_rows() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;
    let table = unique_table_name("dbpaw_cmd_exec_insert");

    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let qualified = format!("`{}`.`{}`", database, table);

    let driver = ClickHouseDriver::connect(&form)
        .await
        .expect("failed to connect clickhouse driver");
    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await
        .ok();
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id UInt32, name String) ENGINE = MergeTree ORDER BY id",
            qualified
        ))
        .await
        .expect("create table should succeed");
    driver.close().await;

    let sql = format!("INSERT INTO {} (id, name) VALUES (1, 'alpha')", qualified);
    let result = execute_by_conn_sql(form.clone(), sql)
        .await
        .expect("execute_by_conn insert should succeed");
    assert!(result.success);
    assert_eq!(result.row_count, 1);

    cleanup_table(&form, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_get_table_data_by_conn_pagination_works() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;

    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let table = unique_table_name("dbpaw_cmd_page");
    let qualified = format!("`{}`.`{}`", database, table);

    let driver = ClickHouseDriver::connect(&form)
        .await
        .expect("failed to connect clickhouse driver");
    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await
        .ok();
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id UInt32, name String) ENGINE = MergeTree ORDER BY id",
            qualified
        ))
        .await
        .expect("create table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')",
            qualified
        ))
        .await
        .expect("insert rows should succeed");
    driver.close().await;

    let page1 = query::get_table_data_by_conn(form.clone(), database.clone(), table.clone(), 1, 2)
        .await
        .expect("page 1 should succeed");
    let page2 = query::get_table_data_by_conn(form.clone(), database, table.clone(), 2, 2)
        .await
        .expect("page 2 should succeed");

    assert_eq!(page1.total, 3);
    assert_eq!(page1.limit, 2);
    assert_eq!(page1.page, 1);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page2.data.len(), 1);

    cleanup_table(&form, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_command_get_table_data_by_conn_invalid_pagination_returns_error() {
    let docker = (!clickhouse_context::should_reuse_local_db()).then(Cli::default);
    let (_clickhouse_container, form) =
        clickhouse_context::clickhouse_form_from_test_context(docker.as_ref());
    wait_until_clickhouse_ready(&form).await;

    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let table = unique_table_name("dbpaw_cmd_invalid_page");
    prepare_query_test_table(&form, &table).await;

    let result = query::get_table_data_by_conn(form.clone(), database, table.clone(), 0, 10).await;
    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(error.contains("[VALIDATION_ERROR]"));

    cleanup_table(&form, &table).await;
}
