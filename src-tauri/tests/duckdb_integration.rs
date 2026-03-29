#[path = "common/duckdb_context.rs"]
mod duckdb_context;

use dbpaw_lib::db::drivers::duckdb::DuckdbDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;

#[tokio::test]
#[ignore]
async fn test_duckdb_integration_flow() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();

    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP VIEW IF EXISTS dbpaw_duckdb_type_probe_v".to_string())
        .await;
    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_type_probe".to_string())
        .await;

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!dbs.is_empty(), "list_databases returned empty");

    driver
        .execute_query(
            "CREATE TABLE IF NOT EXISTS dbpaw_duckdb_type_probe (\
                id INTEGER PRIMARY KEY, \
                name VARCHAR, \
                amount DOUBLE, \
                created_at TIMESTAMP\
            )"
            .to_string(),
        )
        .await
        .expect("create table failed");

    driver
        .execute_query(
            "CREATE VIEW IF NOT EXISTS dbpaw_duckdb_type_probe_v AS \
             SELECT id, name FROM dbpaw_duckdb_type_probe"
                .to_string(),
        )
        .await
        .expect("create view failed");

    driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_type_probe (id, name, amount, created_at) \
             VALUES (1, 'hello', 12.34, '2026-01-02 03:04:05')"
                .to_string(),
        )
        .await
        .expect("insert failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == "dbpaw_duckdb_type_probe"),
        "list_tables should include dbpaw_duckdb_type_probe"
    );
    assert!(
        tables.iter().any(|t| t.name == "dbpaw_duckdb_type_probe_v"),
        "list_tables should include dbpaw_duckdb_type_probe_v"
    );

    let metadata = driver
        .get_table_metadata("main".to_string(), "dbpaw_duckdb_type_probe".to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata.columns.iter().any(|c| c.name == "id"),
        "metadata should include id column"
    );

    let ddl = driver
        .get_table_ddl("main".to_string(), "dbpaw_duckdb_type_probe".to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(
            "SELECT id, name, amount, created_at \
             FROM dbpaw_duckdb_type_probe WHERE id = 1"
                .to_string(),
        )
        .await
        .expect("select typed row failed");
    assert_eq!(result.row_count, 1);
    let row = result
        .data
        .first()
        .expect("typed result should include at least one row");
    let id_value = row.get("id").expect("id should exist");
    assert!(
        id_value == &serde_json::Value::String("1".to_string())
            || id_value == &serde_json::Value::Number(serde_json::Number::from(1)),
        "unexpected id value: {:?}",
        id_value
    );
    assert_eq!(row["name"], serde_json::Value::String("hello".to_string()));
    assert!(row.get("amount").is_some(), "amount should exist");
    assert!(row.get("created_at").is_some(), "created_at should exist");

    let _ = driver
        .execute_query("DROP VIEW IF EXISTS dbpaw_duckdb_type_probe_v".to_string())
        .await;
    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_type_probe".to_string())
        .await;
    driver.close().await;

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_get_table_data_supports_pagination_sort_filter_and_order_by() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_grid_probe".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_grid_probe (id INTEGER PRIMARY KEY, name VARCHAR, score INTEGER)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_duckdb_grid_probe failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_grid_probe (id, name, score) VALUES \
             (1, 'alpha', 10), (2, 'beta', 20), (3, 'gamma', 30), (4, 'delta', 40)"
                .to_string(),
        )
        .await
        .expect("insert dbpaw_duckdb_grid_probe failed");

    let page1 = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_duckdb_grid_probe".to_string(),
            1,
            2,
            Some("score".to_string()),
            Some("desc".to_string()),
            None,
            None,
        )
        .await
        .expect("get_table_data page1 failed");
    assert_eq!(page1.total, 4);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(
        page1.data[0]["name"],
        serde_json::Value::String("delta".to_string())
    );

    let filtered = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_duckdb_grid_probe".to_string(),
            1,
            10,
            None,
            None,
            Some("score >= 20".to_string()),
            None,
        )
        .await
        .expect("get_table_data with filter failed");
    assert_eq!(filtered.total, 3);

    let ordered = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_duckdb_grid_probe".to_string(),
            1,
            1,
            Some("id".to_string()),
            Some("asc".to_string()),
            None,
            Some("name DESC".to_string()),
        )
        .await
        .expect("get_table_data with order_by failed");
    assert_eq!(ordered.total, 4);
    assert_eq!(ordered.data.len(), 1);
    assert_eq!(
        ordered.data[0]["name"],
        serde_json::Value::String("gamma".to_string())
    );

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_grid_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_get_table_data_rejects_invalid_sort_column() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_invalid_sort_probe".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_invalid_sort_probe (id INTEGER PRIMARY KEY)".to_string(),
        )
        .await
        .expect("create dbpaw_duckdb_invalid_sort_probe failed");

    let result = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_duckdb_invalid_sort_probe".to_string(),
            1,
            10,
            Some("id desc".to_string()),
            Some("desc".to_string()),
            None,
            None,
        )
        .await;
    let err = result.expect_err("invalid sort column should return error");
    assert!(
        err.contains("[VALIDATION_ERROR] Invalid sort column name"),
        "unexpected error: {}",
        err
    );

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_invalid_sort_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_table_structure_and_schema_overview() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_overview_probe".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_overview_probe (id INTEGER PRIMARY KEY, label VARCHAR NOT NULL)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_duckdb_overview_probe failed");

    let structure = driver
        .get_table_structure(
            "main".to_string(),
            "dbpaw_duckdb_overview_probe".to_string(),
        )
        .await
        .expect("get_table_structure failed");
    assert!(
        structure
            .columns
            .iter()
            .any(|c| c.name == "id" && c.primary_key),
        "table structure should include primary key id"
    );
    assert!(
        structure.columns.iter().any(|c| c.name == "label"),
        "table structure should include label"
    );

    let overview = driver
        .get_schema_overview(Some("main".to_string()))
        .await
        .expect("get_schema_overview failed");
    assert!(
        overview
            .tables
            .iter()
            .any(|t| t.schema == "main" && t.name == "dbpaw_duckdb_overview_probe"),
        "schema overview should include main.dbpaw_duckdb_overview_probe"
    );

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_overview_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_metadata_includes_empty_indexes_and_foreign_keys() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_meta_probe".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_meta_probe (id INTEGER PRIMARY KEY, name VARCHAR)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_duckdb_meta_probe failed");

    let metadata = driver
        .get_table_metadata("main".to_string(), "dbpaw_duckdb_meta_probe".to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata.columns.iter().any(|c| c.name == "id"),
        "metadata should include id column"
    );
    assert!(
        metadata.indexes.is_empty(),
        "duckdb metadata indexes should be empty for now"
    );
    assert!(
        metadata.foreign_keys.is_empty(),
        "duckdb metadata foreign_keys should be empty for now"
    );

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_meta_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_boolean_and_json_type_mapping_regression() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_bool_json_probe".to_string())
        .await;

    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_bool_json_probe (id INTEGER PRIMARY KEY, flag BOOLEAN, meta VARCHAR)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_duckdb_bool_json_probe failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_bool_json_probe (id, flag, meta) VALUES \
             (1, true, '{\"tier\":\"gold\"}')"
                .to_string(),
        )
        .await
        .expect("insert dbpaw_duckdb_bool_json_probe failed");

    let query_result = driver
        .execute_query(
            "SELECT flag, meta FROM dbpaw_duckdb_bool_json_probe WHERE id = 1".to_string(),
        )
        .await
        .expect("select bool/json probe row failed");
    assert_eq!(query_result.row_count, 1);
    let query_row = query_result.data.first().expect("query row should exist");
    let query_flag = query_row
        .get("flag")
        .expect("flag should exist in query result");
    assert!(
        query_flag == &serde_json::Value::Bool(true)
            || query_flag == &serde_json::Value::Number(serde_json::Number::from(1))
            || query_flag == &serde_json::Value::String("true".to_string()),
        "unexpected query flag value: {:?}",
        query_flag
    );
    assert!(query_row.get("meta").is_some(), "meta should exist");

    let table_data = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_duckdb_bool_json_probe".to_string(),
            1,
            10,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("get_table_data for dbpaw_duckdb_bool_json_probe failed");
    assert_eq!(table_data.total, 1);
    let grid_row = table_data.data.first().expect("table row should exist");
    let grid_flag = grid_row
        .get("flag")
        .expect("flag should exist in table_data result");
    assert!(
        grid_flag == &serde_json::Value::Bool(true)
            || grid_flag == &serde_json::Value::Number(serde_json::Number::from(1))
            || grid_flag == &serde_json::Value::String("true".to_string()),
        "unexpected grid flag value: {:?}",
        grid_flag
    );
    assert!(
        grid_row.get("meta").is_some(),
        "meta should exist in table_data"
    );

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_bool_json_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_execute_query_reports_affected_rows_for_update_delete() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_affected_rows_probe".to_string())
        .await;
    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_affected_rows_probe (id INTEGER PRIMARY KEY, name VARCHAR)"
                .to_string(),
        )
        .await
        .expect("create affected_rows probe table failed");

    let inserted = driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_affected_rows_probe (id, name) VALUES (1, 'a'), (2, 'b')"
                .to_string(),
        )
        .await
        .expect("insert affected_rows probe rows failed");
    assert_eq!(inserted.row_count, 2);

    let updated = driver
        .execute_query(
            "UPDATE dbpaw_duckdb_affected_rows_probe SET name = 'bb' WHERE id = 2".to_string(),
        )
        .await
        .expect("update affected_rows probe row failed");
    assert_eq!(updated.row_count, 1);

    let deleted = driver
        .execute_query(
            "DELETE FROM dbpaw_duckdb_affected_rows_probe WHERE id IN (1, 2)".to_string(),
        )
        .await
        .expect("delete affected_rows probe rows failed");
    assert_eq!(deleted.row_count, 2);

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_affected_rows_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_transaction_commit_and_rollback() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_txn_probe".to_string())
        .await;
    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_txn_probe (id INTEGER PRIMARY KEY, name VARCHAR)"
                .to_string(),
        )
        .await
        .expect("create duckdb txn probe table failed");

    driver
        .execute_query("BEGIN TRANSACTION".to_string())
        .await
        .expect("begin transaction failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_txn_probe (id, name) VALUES (1, 'rolled_back')".to_string(),
        )
        .await
        .expect("insert in rollback tx failed");
    driver
        .execute_query("ROLLBACK".to_string())
        .await
        .expect("rollback failed");

    let rolled_back = driver
        .execute_query(
            "SELECT COUNT(*) AS c FROM dbpaw_duckdb_txn_probe WHERE id = 1".to_string(),
        )
        .await
        .expect("count after rollback failed");
    let rolled_back_count = rolled_back.data[0]["c"]
        .as_str()
        .expect("rollback count should be string")
        .parse::<i64>()
        .expect("rollback count should be numeric");
    assert_eq!(rolled_back_count, 0);

    driver
        .execute_query("BEGIN TRANSACTION".to_string())
        .await
        .expect("begin transaction failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_txn_probe (id, name) VALUES (2, 'committed')".to_string(),
        )
        .await
        .expect("insert in commit tx failed");
    driver
        .execute_query("COMMIT".to_string())
        .await
        .expect("commit failed");

    let committed = driver
        .execute_query(
            "SELECT COUNT(*) AS c FROM dbpaw_duckdb_txn_probe WHERE id = 2".to_string(),
        )
        .await
        .expect("count after commit failed");
    let committed_count = committed.data[0]["c"]
        .as_str()
        .expect("commit count should be string")
        .parse::<i64>()
        .expect("commit count should be numeric");
    assert_eq!(committed_count, 1);

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_txn_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_error_handling_for_sql_error() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let err = driver
        .execute_query("SELECT * FROM __dbpaw_table_not_exists".to_string())
        .await
        .expect_err("invalid SQL should return query error");
    assert!(
        err.contains("[QUERY_ERROR]"),
        "unexpected error shape: {}",
        err
    );

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_connection_failure_with_invalid_path() {
    let form = dbpaw_lib::models::ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some("/nonexistent/path/that/cannot/be/created/dbpaw_test.duckdb".to_string()),
        ..Default::default()
    };

    let err = match DuckdbDriver::connect(&form).await {
        Ok(_) => panic("invalid path should fail"),
        Err(err) => err,
    };
    assert!(
        err.starts_with("[CONN_FAILED]"),
        "unexpected error: {}",
        err
    );
    assert!(!err.trim().is_empty(), "error message should not be empty");
}

#[tokio::test]
#[ignore]
async fn test_duckdb_database_locked_error() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();

    let driver1 = DuckdbDriver::connect(&form)
        .await
        .expect("First connection should succeed");

    let driver2_result = DuckdbDriver::connect(&form).await;

    match driver2_result {
        Ok(driver2) => {
            driver2.close().await;
        }
        Err(err) => {
            assert!(
                err.starts_with("[CONN_FAILED]") || err.contains("busy") || err.contains("locked"),
                "expected lock/busy error, got: {}",
                err
            );
        }
    }

    driver1.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_batch_insert_and_batch_execute_flow() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_batch_probe".to_string())
        .await;
    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_batch_probe (id INTEGER PRIMARY KEY, category VARCHAR, score INTEGER)"
                .to_string(),
        )
        .await
        .expect("create batch probe table failed");

    let value_rows: Vec<String> = (1..=50)
        .map(|id| {
            let category = if id <= 25 { "alpha" } else { "beta" };
            format!("({}, '{}', {})", id, category, id)
        })
        .collect();
    let insert_sql = format!(
        "INSERT INTO dbpaw_duckdb_batch_probe (id, category, score) VALUES {}",
        value_rows.join(", ")
    );
    let inserted = driver
        .execute_query(insert_sql)
        .await
        .expect("batch insert failed");
    assert_eq!(inserted.row_count, 50);

    let batch_sqls = vec![
        "UPDATE dbpaw_duckdb_batch_probe SET score = score + 100 WHERE id <= 10".to_string(),
        "UPDATE dbpaw_duckdb_batch_probe SET category = 'gamma' WHERE id BETWEEN 30 AND 40"
            .to_string(),
        "DELETE FROM dbpaw_duckdb_batch_probe WHERE id IN (3, 6, 9, 12, 15)".to_string(),
    ];
    let mut affected = Vec::new();
    for sql in batch_sqls {
        let result = driver
            .execute_query(sql)
            .await
            .expect("batch execute statement failed");
        affected.push(result.row_count);
    }
    assert_eq!(affected, vec![10, 11, 5]);

    let check_total = driver
        .execute_query(
            "SELECT COUNT(*) AS c FROM dbpaw_duckdb_batch_probe".to_string(),
        )
        .await
        .expect("count after batch execute failed");
    let total = check_total.data[0]["c"]
        .as_str()
        .expect("count should be string")
        .parse::<i64>()
        .expect("count should be numeric");
    assert_eq!(total, 45);

    let check_gamma = driver
        .execute_query(
            "SELECT COUNT(*) AS c FROM dbpaw_duckdb_batch_probe WHERE category = 'gamma'"
                .to_string(),
        )
        .await
        .expect("count gamma rows failed");
    let gamma = check_gamma.data[0]["c"]
        .as_str()
        .expect("gamma count should be string")
        .parse::<i64>()
        .expect("gamma count should be numeric");
    assert_eq!(gamma, 11);

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_batch_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_large_text_and_blob_round_trip() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_large_field_probe".to_string())
        .await;
    driver
        .execute_query(
            "CREATE TABLE dbpaw_duckdb_large_field_probe (id INTEGER PRIMARY KEY, body TEXT, payload BLOB)"
                .to_string(),
        )
        .await
        .expect("create large field probe table failed");

    let large_text = "x".repeat(70000);
    let blob_data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
    let blob_hex: String = blob_data.iter().map(|b| format!("{:02x}", b)).collect();

    driver
        .execute_query(
            format!(
                "INSERT INTO dbpaw_duckdb_large_field_probe (id, body, payload) VALUES (1, '{}', '{}'::BLOB)",
                large_text, blob_hex
            )
            .to_string(),
        )
        .await
        .expect("insert large field probe row failed");

    let result = driver
        .execute_query(
            "SELECT body, payload FROM dbpaw_duckdb_large_field_probe WHERE id = 1".to_string(),
        )
        .await
        .expect("select large field probe row failed");
    assert_eq!(result.row_count, 1);
    let row = result.data.first().expect("large field row should exist");
    let body = row
        .get("body")
        .and_then(|v| v.as_str())
        .expect("body should be string");
    assert_eq!(body.len(), 70000);
    assert!(row.get("payload").is_some(), "payload should exist");

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_large_field_probe".to_string())
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_concurrent_connections_can_query() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();

    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");
    driver
        .execute_query(
            "CREATE TABLE IF NOT EXISTS dbpaw_duckdb_concurrent_probe (id INTEGER, value VARCHAR)"
                .to_string(),
        )
        .await
        .expect("create concurrent probe table failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_duckdb_concurrent_probe VALUES (1, 'test')".to_string(),
        )
        .await
        .expect("insert concurrent probe row failed");
    driver.close().await;

    let mut handles = Vec::new();

    for i in 0..4 {
        let task_db_path = db_path.clone();
        handles.push(tokio::spawn(async move {
            let task_form = dbpaw_lib::models::ConnectionForm {
                driver: "duckdb".to_string(),
                file_path: Some(task_db_path.to_string_lossy().to_string()),
                ..Default::default()
            };
            let task_driver = DuckdbDriver::connect(&task_form)
                .await
                .expect("Failed to connect to duckdb in concurrent task");
            tokio::time::sleep(tokio::time::Duration::from_millis(10 * i as u64)).await;
            let result = task_driver
                .execute_query(
                    "SELECT * FROM dbpaw_duckdb_concurrent_probe WHERE id = 1".to_string(),
                )
                .await;
            task_driver.close().await;
            result
        }));
    }

    for handle in handles {
        let result = handle.await.expect("concurrent duckdb task panicked");
        let data = result.expect("concurrent duckdb query failed");
        assert_eq!(data.row_count, 1);
        assert_eq!(
            data.data[0]["value"],
            serde_json::Value::String("test".to_string())
        );
    }

    let cleanup_driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect for cleanup");
    let _ = cleanup_driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_duckdb_concurrent_probe".to_string())
        .await;
    cleanup_driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_duckdb_view_can_be_listed_and_queried() {
    let (db_path, form) = duckdb_context::duckdb_form_from_test_context();
    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    let base_table = "dbpaw_duckdb_view_base_probe";
    let view_name = "dbpaw_duckdb_view_probe_v";

    let _ = driver
        .execute_query(format!("DROP VIEW IF EXISTS {}", view_name))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", base_table))
        .await;

    driver
        .execute_query(
            format!(
                "CREATE TABLE {} (id INTEGER PRIMARY KEY, name VARCHAR, score INTEGER)",
                base_table
            )
            .to_string(),
        )
        .await
        .expect("create base table for view failed");
    driver
        .execute_query(
            format!(
                "INSERT INTO {} (id, name, score) VALUES (1, 'alice', 10), (2, 'bob', 20)",
                base_table
            )
            .to_string(),
        )
        .await
        .expect("insert base rows for view failed");
    driver
        .execute_query(
            format!(
                "CREATE VIEW {} AS SELECT id, name FROM {} WHERE score >= 20",
                view_name, base_table
            )
            .to_string(),
        )
        .await
        .expect("create view failed");

    let tables = driver
        .list_tables(Some("main".to_string()))
        .await
        .expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.name == base_table && t.r#type == "table"),
        "list_tables should include base table"
    );
    assert!(
        tables.iter().any(|t| t.name == view_name && t.r#type == "view"),
        "list_tables should include view with type=view"
    );

    let view_rows = driver
        .execute_query(
            format!("SELECT id, name FROM {} ORDER BY id", view_name)
                .to_string(),
        )
        .await
        .expect("select from view failed");
    assert_eq!(view_rows.row_count, 1);
    let row = view_rows.data.first().expect("view row should exist");
    let id_matches = row["id"] == serde_json::Value::Number(2.into())
        || row["id"] == serde_json::Value::String("2".to_string());
    assert!(id_matches, "unexpected id payload: {}", row["id"]);
    assert_eq!(
        row["name"],
        serde_json::Value::String("bob".to_string())
    );

    let _ = driver
        .execute_query(format!("DROP VIEW IF EXISTS {}", view_name))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", base_table))
        .await;
    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}
