#[path = "common/oracle_context.rs"]
mod oracle_context;

use dbpaw_lib::commands::{connection, metadata, query};
use dbpaw_lib::db::drivers::oracle::OracleDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_table_name(prefix: &str) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_millis();
    format!("{prefix}_{ms}")
}

async fn prepare_test_table(schema: &str, table: &str, form: &dbpaw_lib::models::ConnectionForm) {
    let driver = OracleDriver::connect(form)
        .await
        .expect("connect for setup should succeed");
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" \
             (id NUMBER(10) PRIMARY KEY, name VARCHAR2(64))"
        ))
        .await
        .expect("CREATE TABLE should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO \"{schema}\".\"{table}\" (id, name) VALUES (1, 'DbPaw')"
        ))
        .await
        .expect("INSERT should succeed");
    driver.close().await;
}

async fn cleanup_table(schema: &str, table: &str, form: &dbpaw_lib::models::ConnectionForm) {
    let driver = OracleDriver::connect(form)
        .await
        .expect("connect for cleanup should succeed");
    let _ = driver
        .execute_query(format!("DROP TABLE \"{schema}\".\"{table}\""))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_test_connection_success() {
    let form = oracle_context::oracle_form_from_test_context();
    let result = connection::test_connection_ephemeral(form)
        .await
        .expect("test_connection_ephemeral should succeed");
    assert!(result.success);
    assert!(result.latency_ms.is_some());
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_test_connection_invalid_password_returns_error() {
    let mut form = oracle_context::oracle_form_from_test_context();
    form.password = Some("dbpaw_wrong_password_xyz".to_string());
    let result = connection::test_connection_ephemeral(form).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_list_databases_returns_schemas() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();

    let databases = connection::list_databases(form)
        .await
        .expect("list_databases should succeed");
    assert!(!databases.is_empty());
    assert!(
        databases.iter().any(|d| d == &schema),
        "schemas should include {schema}"
    );
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_list_tables_by_conn_contains_created_table() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_TABLES").to_uppercase();
    prepare_test_table(&schema, &table, &form).await;

    let tables = metadata::list_tables_by_conn(form.clone())
        .await
        .expect("list_tables_by_conn should succeed");
    assert!(
        tables.iter().any(|t| t.name == table),
        "tables should contain {table}"
    );

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_execute_select_returns_rows() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_SEL").to_uppercase();
    prepare_test_table(&schema, &table, &form).await;

    let sql = format!("SELECT id, name FROM \"{schema}\".\"{table}\" ORDER BY id");
    let result = query::execute_by_conn_direct(form.clone(), sql)
        .await
        .expect("execute SELECT should succeed");

    assert!(result.success);
    assert_eq!(result.row_count, 1);
    assert!(!result.data.is_empty());
    let row = result.data.first().unwrap();
    let name = row.get("NAME").and_then(|v| v.as_str());
    assert_eq!(name, Some("DbPaw"));

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_execute_invalid_sql_returns_error() {
    let form = oracle_context::oracle_form_from_test_context();
    let result =
        query::execute_by_conn_direct(form, "SELECT * FROM __dbpaw_no_such_table".to_string())
            .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_execute_insert_affects_rows() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_INS").to_uppercase();

    let driver = OracleDriver::connect(&form)
        .await
        .expect("connect for setup");
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" \
             (id NUMBER(10) PRIMARY KEY, name VARCHAR2(64))"
        ))
        .await
        .expect("CREATE TABLE");
    driver.close().await;

    let sql = format!(
        "INSERT INTO \"{schema}\".\"{table}\" (id, name) VALUES (1, 'alpha')"
    );
    let result = query::execute_by_conn_direct(form.clone(), sql)
        .await
        .expect("INSERT should succeed");
    assert!(result.success);
    assert_eq!(result.row_count, 1);

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_get_table_data_pagination_works() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_PAGE").to_uppercase();

    let driver = OracleDriver::connect(&form)
        .await
        .expect("connect for setup");
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" (id NUMBER(10) PRIMARY KEY)"
        ))
        .await
        .expect("CREATE TABLE");
    for i in 1..=3 {
        driver
            .execute_query(format!(
                "INSERT INTO \"{schema}\".\"{table}\" (id) VALUES ({i})"
            ))
            .await
            .expect("INSERT");
    }
    driver.close().await;

    let page1 =
        query::get_table_data_by_conn(form.clone(), schema.clone(), table.clone(), 1, 2)
            .await
            .expect("page 1 should succeed");
    let page2 =
        query::get_table_data_by_conn(form.clone(), schema.clone(), table.clone(), 2, 2)
            .await
            .expect("page 2 should succeed");

    assert_eq!(page1.total, 3);
    assert_eq!(page1.limit, 2);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page2.data.len(), 1);

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_get_table_data_invalid_pagination_returns_error() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_INVP").to_uppercase();
    prepare_test_table(&schema, &table, &form).await;

    let result =
        query::get_table_data_by_conn(form.clone(), schema.clone(), table.clone(), 0, 10).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("[VALIDATION_ERROR]"));

    cleanup_table(&schema, &table, &form).await;
}
