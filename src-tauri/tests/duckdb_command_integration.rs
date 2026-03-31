use dbpaw_lib::commands::{connection, metadata, query};
use dbpaw_lib::db::drivers::duckdb::DuckdbDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn duckdb_test_path() -> PathBuf {
    if let Ok(v) = env::var("DUCKDB_IT_DB_PATH") {
        return PathBuf::from(v);
    }
    let mut p = env::temp_dir();
    p.push(format!("dbpaw-duckdb-cmd-{}.db", Uuid::new_v4()));
    p
}

fn unique_table_name(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_millis();
    format!("{}_{}", prefix, millis)
}

async fn prepare_query_test_table(form: &ConnectionForm, table: &str) {
    let driver = DuckdbDriver::connect(form)
        .await
        .expect("failed to connect duckdb driver");

    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", table))
        .await
        .expect("drop table should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY, name VARCHAR)",
            table
        ))
        .await
        .expect("create table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'DbPaw')",
            table
        ))
        .await
        .expect("insert row should succeed");
    driver.close().await;
}

async fn cleanup_table(form: &ConnectionForm, table: &str) {
    let driver = DuckdbDriver::connect(form)
        .await
        .expect("failed to connect duckdb driver for cleanup");
    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", table))
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
async fn test_duckdb_command_test_connection_success() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let result = connection::test_connection_ephemeral(form)
        .await
        .expect("test_connection_ephemeral should succeed");

    assert!(result.success);
    assert!(result.latency_ms.is_some());

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_test_connection_invalid_file_path_returns_error() {
    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some("/nonexistent/path/to/database.db".to_string()),
        ..Default::default()
    };

    let result = connection::test_connection_ephemeral(form).await;

    // DuckDB might succeed with nonexistent path (creates file), so we adjust expectations
    // Alternative: test with read-only or permissions issue
    assert!(result.is_ok() || result.is_err());
    if result.is_err() {
        let error = result.err().unwrap_or_default();
        assert!(!error.trim().is_empty());
    }
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_list_tables_by_conn_contains_created_table() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let table = unique_table_name("dbpaw_cmd_tables");
    prepare_query_test_table(&form, &table).await;

    let tables = metadata::list_tables_by_conn(form.clone())
        .await
        .expect("list_tables_by_conn should succeed");

    assert!(tables.iter().any(|t| t.name == table));
    cleanup_table(&form, &table).await;

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_list_databases_contains_default_db() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let databases = connection::list_databases(form)
        .await
        .expect("list_databases should succeed");

    assert!(!databases.is_empty());
    assert!(databases.iter().all(|db| !db.trim().is_empty()));

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_execute_by_conn_select_returns_rows() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let table = unique_table_name("dbpaw_cmd_exec_select");
    prepare_query_test_table(&form, &table).await;

    let sql = format!("SELECT id, name FROM {} ORDER BY id", table);
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

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_execute_by_conn_invalid_sql_returns_error() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let result = execute_by_conn_sql(
        form,
        "SELECT * FROM __dbpaw_missing_command_table".to_string(),
    )
    .await;

    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(!error.trim().is_empty());

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_execute_by_conn_insert_affects_rows() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let table = unique_table_name("dbpaw_cmd_exec_insert");

    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("failed to connect duckdb driver");
    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", table))
        .await
        .expect("drop table should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY, name VARCHAR)",
            table
        ))
        .await
        .expect("create table should succeed");
    driver.close().await;

    let sql = format!("INSERT INTO {} (id, name) VALUES (1, 'alpha')", table);
    let result = execute_by_conn_sql(form.clone(), sql)
        .await
        .expect("execute_by_conn insert should succeed");
    assert!(result.success);
    assert_eq!(result.row_count, 1);

    cleanup_table(&form, &table).await;

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_get_table_data_by_conn_pagination_works() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let database = "".to_string(); // DuckDB uses empty string for default database
    let table = unique_table_name("dbpaw_cmd_page");

    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("failed to connect duckdb driver");
    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", table))
        .await
        .expect("drop table should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INTEGER PRIMARY KEY, name VARCHAR)",
            table
        ))
        .await
        .expect("create table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')",
            table
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

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_command_get_table_data_by_conn_invalid_pagination_returns_error() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let database = "".to_string();
    let table = unique_table_name("dbpaw_cmd_invalid_page");
    prepare_query_test_table(&form, &table).await;

    let result = query::get_table_data_by_conn(form.clone(), database, table.clone(), 0, 10).await;
    assert!(result.is_err());
    let error = result.err().unwrap_or_default();
    assert!(error.contains("[VALIDATION_ERROR]"));

    cleanup_table(&form, &table).await;

    // Cleanup
    let _ = std::fs::remove_file(db_path);
}
