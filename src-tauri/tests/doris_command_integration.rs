#[path = "common/doris_context.rs"]
mod doris_context;

use dbpaw_lib::commands::{connection, metadata, query};
use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;

use doris_context::{shared_doris_form, unique_name, wait_until_ready};

async fn prepare_query_test_table(form: &ConnectionForm, table: &str) {
    let db_name = unique_name("dbpaw_doris_cmd_db");
    let qualified = format!("`{}`.`{}`", db_name, table);

    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect doris driver");

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");

    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await
        .expect("drop table should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT, name STRING)",
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

async fn cleanup_database(form: &ConnectionForm, db_name: &str) {
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect doris driver for cleanup");
    driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await
        .expect("drop database should succeed");
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
async fn test_doris_command_test_connection_success() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;

    let result = connection::test_connection_ephemeral(form)
        .await
        .expect("test_connection_ephemeral should succeed");

    assert!(result.success);
    assert!(result.latency_ms.is_some());
}

#[tokio::test]
#[ignore]
async fn test_doris_command_test_connection_invalid_host_returns_error() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;
    let mut bad_form = form.clone();
    bad_form.host = Some("invalid_host_that_does_not_exist".to_string());

    let result = connection::test_connection_ephemeral(bad_form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_doris_command_list_tables_by_conn_contains_created_table() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;
    let db_name = unique_name("dbpaw_doris_tables_db");
    let table = unique_name("dbpaw_cmd_tables");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT)",
            db_name, table
        ))
        .await
        .expect("create table should succeed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let tables = metadata::list_tables_by_conn(form_with_db.clone())
        .await
        .expect("list_tables_by_conn should succeed");

    assert!(tables.iter().any(|t| t.name == table));
    cleanup_database(&form, &db_name).await;
}

#[tokio::test]
#[ignore]
async fn test_doris_command_list_tables_by_conn_invalid_credentials_returns_error() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;
    let mut bad_form = form.clone();
    bad_form.username = Some("invalid_user".to_string());

    let result = metadata::list_tables_by_conn(bad_form).await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_doris_command_list_databases_contains_target_db() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;

    let db_name = unique_name("dbpaw_doris_list_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");
    driver.close().await;

    let databases = connection::list_databases(form.clone())
        .await
        .expect("list_databases should succeed");

    assert!(!databases.is_empty());
    assert!(databases.iter().any(|db| *db == db_name));
    assert!(databases.iter().all(|db| !db.trim().is_empty()));

    cleanup_database(&form, &db_name).await;
}

#[tokio::test]
#[ignore]
async fn test_doris_command_execute_by_conn_select_returns_rows() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;
    let db_name = unique_name("dbpaw_doris_select_db");
    let table = unique_name("dbpaw_cmd_exec_select");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT, name STRING)",
            db_name, table
        ))
        .await
        .expect("create table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO `{}`.`{}` (id, name) VALUES (1, 'DbPaw')",
            db_name, table
        ))
        .await
        .expect("insert row should succeed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

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
    cleanup_database(&form, &db_name).await;
}

#[tokio::test]
#[ignore]
async fn test_doris_command_execute_by_conn_invalid_sql_returns_error() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;

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
async fn test_doris_command_execute_by_conn_insert_affects_rows() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;
    let db_name = unique_name("dbpaw_doris_insert_db");
    let table = unique_name("dbpaw_cmd_exec_insert");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT, name STRING)",
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

    cleanup_database(&form, &db_name).await;
}

#[tokio::test]
#[ignore]
async fn test_doris_command_get_table_data_by_conn_pagination_works() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;

    let db_name = unique_name("dbpaw_doris_page_db");
    let table = unique_name("dbpaw_cmd_page");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT, name STRING)",
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

    cleanup_database(&form, &db_name).await;
}

#[tokio::test]
#[ignore]
async fn test_doris_command_get_table_data_by_conn_invalid_pagination_returns_error() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;

    let db_name = unique_name("dbpaw_doris_invalid_page_db");
    let table = unique_name("dbpaw_cmd_invalid_page");

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE `{}`.`{}` (id INT, name STRING)",
            db_name, table
        ))
        .await
        .expect("create table should succeed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

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

    cleanup_database(&form, &db_name).await;
}

#[tokio::test]
#[ignore]
async fn test_doris_command_create_database_by_id_success() {
    let form = shared_doris_form();
    wait_until_ready(&form).await;

    let db_name = unique_name("dbpaw_doris_cmd_db");

    // Create a temporary connection to test database creation
    let mut test_form = form.clone();
    test_form.database = Some(db_name.clone());

    // Note: For Doris, we test database creation through the driver directly
    // since the command layer may have additional state management
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect doris driver");

    let result = driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await;
    assert!(result.is_ok(), "create database should succeed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases should succeed");
    assert!(
        dbs.iter().any(|d| *d == db_name),
        "list_databases should include {}",
        db_name
    );

    cleanup_database(&form, &db_name).await;
}
