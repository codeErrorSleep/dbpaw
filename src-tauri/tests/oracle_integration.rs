#[path = "common/oracle_context.rs"]
mod oracle_context;

use dbpaw_lib::db::drivers::oracle::OracleDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_table_name() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_millis();
    format!("DBPAW_ORA_IT_{}", ms)
}

#[tokio::test]
#[ignore]
async fn test_oracle_integration_flow() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();

    let driver = oracle_context::connect_with_retry(|| OracleDriver::connect(&form)).await;

    // test_connection
    driver
        .test_connection()
        .await
        .expect("test_connection should succeed");

    // list_databases returns schema names
    let schemas = driver
        .list_databases()
        .await
        .expect("list_databases should succeed");
    assert!(!schemas.is_empty(), "list_databases returned empty list");
    assert!(
        schemas.iter().any(|s| s == &schema),
        "list_databases should include {schema}"
    );

    let table = unique_table_name();

    // Clean up any leftovers from previous runs
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;

    // Create test table
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" ( \
                id     NUMBER(10)    PRIMARY KEY, \
                name   VARCHAR2(50), \
                amount NUMBER(10,2), \
                ts     DATE \
            )"
        ))
        .await
        .expect("CREATE TABLE should succeed");

    // Insert a row
    driver
        .execute_query(format!(
            "INSERT INTO \"{schema}\".\"{table}\" (id, name, amount, ts) \
             VALUES (1, 'hello', 12.34, SYSDATE)"
        ))
        .await
        .expect("INSERT should succeed");

    // list_tables
    let tables = driver
        .list_tables(Some(schema.clone()))
        .await
        .expect("list_tables should succeed");
    assert!(
        tables.iter().any(|t| t.name == table),
        "list_tables should contain {table}"
    );

    // get_table_structure
    let structure = driver
        .get_table_structure(schema.clone(), table.clone())
        .await
        .expect("get_table_structure should succeed");
    assert!(
        !structure.columns.is_empty(),
        "structure should have columns"
    );
    assert!(
        structure.columns.iter().any(|c| c.name == "ID" && c.primary_key),
        "ID column should be marked as primary key"
    );
    assert!(
        structure.columns.iter().any(|c| c.name == "NAME"),
        "NAME column should be present"
    );

    // get_table_metadata
    let metadata = driver
        .get_table_metadata(schema.clone(), table.clone())
        .await
        .expect("get_table_metadata should succeed");
    assert!(
        metadata.columns.iter().any(|c| c.primary_key),
        "metadata should have a primary key column"
    );

    // get_table_ddl
    let ddl = driver
        .get_table_ddl(schema.clone(), table.clone())
        .await
        .expect("get_table_ddl should succeed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    // get_table_data
    let result = driver
        .get_table_data(
            schema.clone(),
            table.clone(),
            1,
            10,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("get_table_data should succeed");
    assert_eq!(result.total, 1, "total should be 1");
    assert_eq!(result.data.len(), 1, "data should have 1 row");
    let row = result.data.first().unwrap();
    assert!(
        row.get("ID").is_some() || row.get("id").is_some(),
        "row should have ID column"
    );

    // execute_query SELECT
    let qr = driver
        .execute_query(format!(
            "SELECT id, name FROM \"{schema}\".\"{table}\""
        ))
        .await
        .expect("execute_query SELECT should succeed");
    assert!(qr.success);
    assert_eq!(qr.row_count, 1);
    assert!(
        qr.columns.iter().any(|c| c.name == "ID"),
        "columns should include ID"
    );

    // execute_query DML affected rows
    let upd = driver
        .execute_query(format!(
            "UPDATE \"{schema}\".\"{table}\" SET amount = 99.99 WHERE id = 1"
        ))
        .await
        .expect("execute_query UPDATE should succeed");
    assert!(upd.success);
    assert_eq!(upd.row_count, 1, "UPDATE should affect 1 row");

    // get_schema_overview
    let overview = driver
        .get_schema_overview(Some(schema.clone()))
        .await
        .expect("get_schema_overview should succeed");
    assert!(
        overview.tables.iter().any(|t| t.name == table),
        "schema_overview should include {table}"
    );

    // Cleanup
    let _ = driver
        .execute_query(format!("DROP TABLE \"{schema}\".\"{table}\""))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_integration_pagination() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let driver = oracle_context::connect_with_retry(|| OracleDriver::connect(&form)).await;
    let table = unique_table_name();

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
        .expect("CREATE TABLE should succeed");

    // Insert 5 rows
    for i in 1..=5 {
        driver
            .execute_query(format!(
                "INSERT INTO \"{schema}\".\"{table}\" (id) VALUES ({i})"
            ))
            .await
            .expect("INSERT should succeed");
    }

    let page1 = driver
        .get_table_data(schema.clone(), table.clone(), 1, 3, None, None, None, None)
        .await
        .expect("page 1 should succeed");
    assert_eq!(page1.total, 5);
    assert_eq!(page1.data.len(), 3);

    let page2 = driver
        .get_table_data(schema.clone(), table.clone(), 2, 3, None, None, None, None)
        .await
        .expect("page 2 should succeed");
    assert_eq!(page2.data.len(), 2);

    let _ = driver
        .execute_query(format!("DROP TABLE \"{schema}\".\"{table}\""))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_integration_connection_failure() {
    let mut form = oracle_context::oracle_form_from_test_context();
    form.password = Some("dbpaw_wrong_password_xyz".to_string());
    let result = OracleDriver::connect(&form).await;
    assert!(result.is_err(), "wrong password should fail");
    let err = result.err().expect("should have an error");
    assert!(err.contains("[CONN_FAILED]"), "error should be tagged CONN_FAILED");
}
