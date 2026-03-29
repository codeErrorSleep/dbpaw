use dbpaw_lib::db::drivers::sqlite::SqliteDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;
use std::path::PathBuf;
use uuid::Uuid;

fn sqlite_test_path() -> PathBuf {
    if let Ok(v) = env::var("SQLITE_IT_DB_PATH") {
        return PathBuf::from(v);
    }
    let mut p = env::temp_dir();
    p.push(format!("dbpaw-sqlite-integration-{}.db", Uuid::new_v4()));
    p
}

fn json_to_i64(value: &serde_json::Value) -> i64 {
    if let Some(v) = value.as_i64() {
        return v;
    }
    if let Some(v) = value.as_str() {
        return v
            .parse::<i64>()
            .expect("string value should be numeric for integer assertion");
    }
    panic!("value should be i64 or numeric string, got {}", value);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_integration_flow() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert_eq!(dbs, vec!["main".to_string()]);

    driver
        .execute_query(
            "CREATE TABLE IF NOT EXISTS dbpaw_sqlite_type_probe (\
                id INTEGER PRIMARY KEY, \
                name TEXT, \
                amount NUMERIC, \
                payload BLOB, \
                created_at TEXT\
            )"
            .to_string(),
        )
        .await
        .expect("create table failed");

    driver
        .execute_query(
            "CREATE VIEW IF NOT EXISTS dbpaw_sqlite_type_probe_v AS \
             SELECT id, name FROM dbpaw_sqlite_type_probe"
                .to_string(),
        )
        .await
        .expect("create view failed");

    driver
        .execute_query(
            "INSERT INTO dbpaw_sqlite_type_probe (id, name, amount, payload, created_at) \
             VALUES (1, 'hello', 12.34, x'DEADBEEF', '2026-01-02 03:04:05')"
                .to_string(),
        )
        .await
        .expect("insert failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == "dbpaw_sqlite_type_probe"),
        "list_tables should include dbpaw_sqlite_type_probe"
    );
    assert!(
        tables.iter().any(|t| t.name == "dbpaw_sqlite_type_probe_v"),
        "list_tables should include dbpaw_sqlite_type_probe_v"
    );

    let metadata = driver
        .get_table_metadata("main".to_string(), "dbpaw_sqlite_type_probe".to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata
            .columns
            .iter()
            .any(|c| c.name == "id" && c.primary_key),
        "metadata should include primary key id"
    );
    assert!(
        metadata.columns.iter().any(|c| c.name == "payload"),
        "metadata should include payload column"
    );

    let ddl = driver
        .get_table_ddl("main".to_string(), "dbpaw_sqlite_type_probe".to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(
            "SELECT id, name, amount, payload, created_at FROM dbpaw_sqlite_type_probe WHERE id = 1"
                .to_string(),
        )
        .await
        .expect("select typed row failed");
    assert_eq!(result.row_count, 1);
    let row = result
        .data
        .first()
        .expect("typed result should include at least one row");
    assert_eq!(row["id"], serde_json::Value::String("1".to_string()));
    assert_eq!(row["name"], serde_json::Value::String("hello".to_string()));
    assert!(row.get("amount").is_some(), "amount should exist");
    assert!(row.get("payload").is_some(), "payload should exist");

    let _ = driver
        .execute_query("DROP VIEW IF EXISTS dbpaw_sqlite_type_probe_v".to_string())
        .await;
    let _ = driver
        .execute_query("DROP TABLE IF EXISTS dbpaw_sqlite_type_probe".to_string())
        .await;
    driver.close().await;

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_get_table_data_supports_pagination_sort_filter_and_order_by() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_grid_probe (id INTEGER PRIMARY KEY, name TEXT, score INTEGER)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_sqlite_grid_probe failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_sqlite_grid_probe (id, name, score) VALUES \
             (1, 'alpha', 10), (2, 'beta', 20), (3, 'gamma', 30), (4, 'delta', 40)"
                .to_string(),
        )
        .await
        .expect("insert dbpaw_sqlite_grid_probe failed");

    let page1 = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_sqlite_grid_probe".to_string(),
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
            "dbpaw_sqlite_grid_probe".to_string(),
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
            "dbpaw_sqlite_grid_probe".to_string(),
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

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_get_table_data_rejects_invalid_sort_column() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query("CREATE TABLE dbpaw_sqlite_invalid_sort_probe (id INTEGER PRIMARY KEY)".to_string())
        .await
        .expect("create dbpaw_sqlite_invalid_sort_probe failed");

    let result = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_sqlite_invalid_sort_probe".to_string(),
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

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_table_structure_and_schema_overview() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_overview_probe (id INTEGER PRIMARY KEY, label TEXT NOT NULL)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_sqlite_overview_probe failed");

    let structure = driver
        .get_table_structure("main".to_string(), "dbpaw_sqlite_overview_probe".to_string())
        .await
        .expect("get_table_structure failed");
    assert!(
        structure.columns.iter().any(|c| c.name == "id" && c.primary_key),
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
            .any(|t| t.schema == "main" && t.name == "dbpaw_sqlite_overview_probe"),
        "schema overview should include main.dbpaw_sqlite_overview_probe"
    );

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_metadata_includes_indexes_and_foreign_keys() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_parent_meta_probe (id INTEGER PRIMARY KEY); \
             CREATE TABLE dbpaw_sqlite_child_meta_probe (\
               id INTEGER PRIMARY KEY, \
               parent_id INTEGER NOT NULL, \
               name TEXT, \
               CONSTRAINT fk_dbpaw_sqlite_child_parent FOREIGN KEY(parent_id) REFERENCES dbpaw_sqlite_parent_meta_probe(id)\
             ); \
             CREATE INDEX idx_dbpaw_sqlite_child_name ON dbpaw_sqlite_child_meta_probe(name);"
                .to_string(),
        )
        .await
        .expect("create sqlite metadata probe tables failed");

    let metadata = driver
        .get_table_metadata("main".to_string(), "dbpaw_sqlite_child_meta_probe".to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata
            .indexes
            .iter()
            .any(|i| i.name == "idx_dbpaw_sqlite_child_name" && i.columns.contains(&"name".to_string())),
        "metadata should include idx_dbpaw_sqlite_child_name"
    );
    assert!(
        metadata
            .foreign_keys
            .iter()
            .any(|fk| fk.column == "parent_id" && fk.referenced_table == "dbpaw_sqlite_parent_meta_probe"),
        "metadata should include FK parent_id -> dbpaw_sqlite_parent_meta_probe(id)"
    );

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_boolean_and_json_type_mapping_regression() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_bool_json_probe (id INTEGER PRIMARY KEY, flag BOOLEAN, meta TEXT)"
                .to_string(),
        )
        .await
        .expect("create dbpaw_sqlite_bool_json_probe failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_sqlite_bool_json_probe (id, flag, meta) VALUES \
             (1, 1, '{\"tier\":\"gold\"}')"
                .to_string(),
        )
        .await
        .expect("insert dbpaw_sqlite_bool_json_probe failed");

    let query_result = driver
        .execute_query(
            "SELECT flag, json_extract(meta, '$.tier') AS tier \
             FROM dbpaw_sqlite_bool_json_probe WHERE id = 1"
                .to_string(),
        )
        .await
        .expect("select bool/json probe row failed");
    assert_eq!(query_result.row_count, 1);
    let query_row = query_result.data.first().expect("query row should exist");
    assert_eq!(query_row["flag"], serde_json::Value::Bool(true));
    assert_eq!(query_row["tier"], serde_json::Value::String("gold".to_string()));

    let table_data = driver
        .get_table_data(
            "main".to_string(),
            "dbpaw_sqlite_bool_json_probe".to_string(),
            1,
            10,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("get_table_data for dbpaw_sqlite_bool_json_probe failed");
    assert_eq!(table_data.total, 1);
    let grid_row = table_data.data.first().expect("table row should exist");
    assert_eq!(grid_row["flag"], serde_json::Value::Bool(true));
    assert!(
        grid_row.get("meta").is_some(),
        "meta should exist in table_data"
    );

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_transaction_commit_and_rollback() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_txn_probe (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
        )
        .await
        .expect("create sqlite txn probe table failed");

    let mut rollback_tx = driver.pool.begin().await.expect("begin rollback tx failed");
    sqlx::query("INSERT INTO dbpaw_sqlite_txn_probe (id, name) VALUES (?, ?)")
        .bind(1_i64)
        .bind("rolled_back")
        .execute(&mut *rollback_tx)
        .await
        .expect("insert in rollback tx failed");
    rollback_tx.rollback().await.expect("rollback tx failed");

    let rolled_back = driver
        .execute_query("SELECT COUNT(*) AS c FROM dbpaw_sqlite_txn_probe WHERE id = 1".to_string())
        .await
        .expect("count after rollback failed");
    let rolled_back_count = json_to_i64(&rolled_back.data[0]["c"]);
    assert_eq!(rolled_back_count, 0);

    let mut commit_tx = driver.pool.begin().await.expect("begin commit tx failed");
    sqlx::query("INSERT INTO dbpaw_sqlite_txn_probe (id, name) VALUES (?, ?)")
        .bind(2_i64)
        .bind("committed")
        .execute(&mut *commit_tx)
        .await
        .expect("insert in commit tx failed");
    commit_tx.commit().await.expect("commit tx failed");

    let committed = driver
        .execute_query("SELECT COUNT(*) AS c FROM dbpaw_sqlite_txn_probe WHERE id = 2".to_string())
        .await
        .expect("count after commit failed");
    let committed_count = json_to_i64(&committed.data[0]["c"]);
    assert_eq!(committed_count, 1);

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_execute_query_reports_affected_rows_for_update_delete() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_affected_rows_probe (id INTEGER PRIMARY KEY, name TEXT)"
                .to_string(),
        )
        .await
        .expect("create affected_rows probe table failed");

    let inserted = driver
        .execute_query(
            "INSERT INTO dbpaw_sqlite_affected_rows_probe (id, name) VALUES (1, 'a'), (2, 'b')"
                .to_string(),
        )
        .await
        .expect("insert affected_rows probe rows failed");
    assert_eq!(inserted.row_count, 2);

    let updated = driver
        .execute_query(
            "UPDATE dbpaw_sqlite_affected_rows_probe SET name = 'bb' WHERE id = 2".to_string(),
        )
        .await
        .expect("update affected_rows probe row failed");
    assert_eq!(updated.row_count, 1);

    let deleted = driver
        .execute_query("DELETE FROM dbpaw_sqlite_affected_rows_probe WHERE id IN (1, 2)".to_string())
        .await
        .expect("delete affected_rows probe rows failed");
    assert_eq!(deleted.row_count, 2);

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_large_text_and_blob_round_trip() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_large_field_probe (id INTEGER PRIMARY KEY, body TEXT, payload BLOB)"
                .to_string(),
        )
        .await
        .expect("create large field probe table failed");

    let large_body = "x".repeat(70000);
    let large_payload = vec![0xAB_u8; 2048];
    let mut conn = driver
        .pool
        .acquire()
        .await
        .expect("acquire sqlite pooled connection failed");
    sqlx::query("INSERT INTO dbpaw_sqlite_large_field_probe (id, body, payload) VALUES (?, ?, ?)")
        .bind(1_i64)
        .bind(&large_body)
        .bind(&large_payload)
        .execute(&mut *conn)
        .await
        .expect("insert large field probe row failed");
    drop(conn);

    let result = driver
        .execute_query(
            "SELECT body, payload FROM dbpaw_sqlite_large_field_probe WHERE id = 1".to_string(),
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

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_error_handling_for_sql_error() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

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
async fn test_sqlite_concurrent_connections_can_query() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut handles = Vec::new();

    for _ in 0..8 {
        let task_form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(db_path_str.clone()),
            ..Default::default()
        };
        handles.push(tokio::spawn(async move {
            let driver = SqliteDriver::connect(&task_form)
                .await
                .expect("connect sqlite in concurrent task failed");
            let result = driver.execute_query("SELECT 1 AS ok".to_string()).await;
            driver.close().await;
            result
        }));
    }

    for handle in handles {
        let result = handle.await.expect("concurrent sqlite task panicked");
        let data = result.expect("concurrent sqlite query failed");
        assert_eq!(data.row_count, 1);
        let ok = &data.data[0]["ok"];
        let matches = *ok == serde_json::Value::Number(1.into()) || ok == "1";
        assert!(matches, "ok should be 1, got {}", ok);
    }

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_view_can_be_listed_and_queried() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_view_base_probe (id INTEGER PRIMARY KEY, name TEXT, score INTEGER)"
                .to_string(),
        )
        .await
        .expect("create base table for view failed");
    driver
        .execute_query(
            "INSERT INTO dbpaw_sqlite_view_base_probe (id, name, score) VALUES (1, 'alice', 10), (2, 'bob', 20)"
                .to_string(),
        )
        .await
        .expect("insert base rows for view failed");
    driver
        .execute_query(
            "CREATE VIEW dbpaw_sqlite_view_probe_v AS SELECT id, name FROM dbpaw_sqlite_view_base_probe WHERE score >= 20"
                .to_string(),
        )
        .await
        .expect("create view failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.name == "dbpaw_sqlite_view_base_probe" && t.r#type == "table"),
        "list_tables should include base table"
    );
    assert!(
        tables
            .iter()
            .any(|t| t.name == "dbpaw_sqlite_view_probe_v" && t.r#type == "view"),
        "list_tables should include view with type=view"
    );

    let view_rows = driver
        .execute_query("SELECT id, name FROM dbpaw_sqlite_view_probe_v ORDER BY id".to_string())
        .await
        .expect("select from view failed");
    assert_eq!(view_rows.row_count, 1);
    let row = view_rows.data.first().expect("view row should exist");
    let id_matches = row["id"] == serde_json::Value::Number(2.into())
        || row["id"] == serde_json::Value::String("2".to_string());
    assert!(id_matches, "unexpected id payload: {}", row["id"]);
    assert_eq!(row["name"], serde_json::Value::String("bob".to_string()));

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_connection_failure_with_invalid_file_path() {
    let mut missing_dir = env::temp_dir();
    missing_dir.push(format!("dbpaw_sqlite_missing_dir_{}", Uuid::new_v4()));
    let missing_path = missing_dir.join("db.sqlite");
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(missing_path.to_string_lossy().to_string()),
        ..Default::default()
    };

    let err = match SqliteDriver::connect(&form).await {
        Ok(_) => panic!("invalid file path should fail"),
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
async fn test_sqlite_lock_conflict_or_busy_error() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver_a = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect sqlite driver A");
    let driver_b = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect sqlite driver B");

    driver_a
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_lock_probe (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
        )
        .await
        .expect("create lock probe table failed");
    driver_a
        .execute_query("PRAGMA busy_timeout = 100".to_string())
        .await
        .expect("set busy_timeout for driver A failed");
    driver_b
        .execute_query("PRAGMA busy_timeout = 100".to_string())
        .await
        .expect("set busy_timeout for driver B failed");

    let mut tx = driver_a
        .pool
        .begin()
        .await
        .expect("begin write lock tx failed");
    sqlx::query("INSERT INTO dbpaw_sqlite_lock_probe (id, name) VALUES (?, ?)")
        .bind(1_i64)
        .bind("a")
        .execute(&mut *tx)
        .await
        .expect("insert in lock tx failed");

    let err = driver_b
        .execute_query("INSERT INTO dbpaw_sqlite_lock_probe (id, name) VALUES (2, 'b')".to_string())
        .await
        .expect_err("concurrent write under lock should fail");
    assert!(
        err.contains("[QUERY_ERROR]"),
        "unexpected lock error shape: {}",
        err
    );
    let lower = err.to_ascii_lowercase();
    assert!(
        lower.contains("locked") || lower.contains("busy"),
        "unexpected lock/busy error: {}",
        err
    );

    tx.rollback().await.expect("rollback lock tx failed");
    driver_a.close().await;
    driver_b.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_batch_insert_and_batch_execute_flow() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_batch_probe (id INTEGER PRIMARY KEY, category TEXT, score INTEGER)"
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
        "INSERT INTO dbpaw_sqlite_batch_probe (id, category, score) VALUES {}",
        value_rows.join(", ")
    );
    let inserted = driver
        .execute_query(insert_sql)
        .await
        .expect("batch insert failed");
    assert_eq!(inserted.row_count, 50);

    let batch_sqls = vec![
        "UPDATE dbpaw_sqlite_batch_probe SET score = score + 100 WHERE id <= 10".to_string(),
        "UPDATE dbpaw_sqlite_batch_probe SET category = 'gamma' WHERE id BETWEEN 30 AND 40"
            .to_string(),
        "DELETE FROM dbpaw_sqlite_batch_probe WHERE id IN (3, 6, 9, 12, 15)".to_string(),
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
        .execute_query("SELECT COUNT(*) AS c FROM dbpaw_sqlite_batch_probe".to_string())
        .await
        .expect("count after batch execute failed");
    let total = json_to_i64(&check_total.data[0]["c"]);
    assert_eq!(total, 45);

    let check_gamma = driver
        .execute_query(
            "SELECT COUNT(*) AS c FROM dbpaw_sqlite_batch_probe WHERE category = 'gamma'"
                .to_string(),
        )
        .await
        .expect("count gamma rows failed");
    let gamma = json_to_i64(&check_gamma.data[0]["c"]);
    assert_eq!(gamma, 11);

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
#[ignore]
async fn test_sqlite_prepared_statements_prepare_execute_and_deallocate() {
    let db_path = sqlite_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();
    let form = ConnectionForm {
        driver: "sqlite".to_string(),
        file_path: Some(db_path_str),
        ..Default::default()
    };
    let driver = SqliteDriver::connect(&form)
        .await
        .expect("Failed to connect to sqlite db");

    driver
        .execute_query(
            "CREATE TABLE dbpaw_sqlite_prepared_stmt_probe (id INTEGER PRIMARY KEY, name TEXT)"
                .to_string(),
        )
        .await
        .expect("create prepared stmt probe table failed");

    let mut conn = driver
        .pool
        .acquire()
        .await
        .expect("acquire sqlite pooled connection failed");
    let prepared_insert_sql =
        "INSERT INTO dbpaw_sqlite_prepared_stmt_probe (id, name) VALUES (?, ?)".to_string();
    let insert_a = sqlx::query(&prepared_insert_sql)
        .bind(1_i64)
        .bind("alice")
        .execute(&mut *conn)
        .await
        .expect("prepared insert alice failed");
    assert_eq!(insert_a.rows_affected(), 1);
    let insert_b = sqlx::query(&prepared_insert_sql)
        .bind(2_i64)
        .bind("bob")
        .execute(&mut *conn)
        .await
        .expect("prepared insert bob failed");
    assert_eq!(insert_b.rows_affected(), 1);

    let prepared_update_sql =
        "UPDATE dbpaw_sqlite_prepared_stmt_probe SET name = ? WHERE id = ?".to_string();
    let updated = sqlx::query(&prepared_update_sql)
        .bind("alice-updated")
        .bind(1_i64)
        .execute(&mut *conn)
        .await
        .expect("prepared update failed");
    assert_eq!(updated.rows_affected(), 1);

    let prepared_select_sql =
        "SELECT name FROM dbpaw_sqlite_prepared_stmt_probe WHERE id = ?".to_string();
    let selected_name: String = sqlx::query_scalar(&prepared_select_sql)
        .bind(1_i64)
        .fetch_one(&mut *conn)
        .await
        .expect("prepared select failed");
    assert_eq!(selected_name, "alice-updated");
    drop(conn);

    let verify = driver
        .execute_query("SELECT COUNT(*) AS c FROM dbpaw_sqlite_prepared_stmt_probe".to_string())
        .await
        .expect("verify prepared writes failed");
    let total = json_to_i64(&verify.data[0]["c"]);
    assert_eq!(total, 2);

    driver.close().await;
    let _ = std::fs::remove_file(db_path);
}
