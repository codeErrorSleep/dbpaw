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
            "CREATE TABLE IF NOT EXISTS sqlite_type_probe (\
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
            "CREATE VIEW IF NOT EXISTS sqlite_type_probe_v AS \
             SELECT id, name FROM sqlite_type_probe"
                .to_string(),
        )
        .await
        .expect("create view failed");

    driver
        .execute_query(
            "INSERT INTO sqlite_type_probe (id, name, amount, payload, created_at) \
             VALUES (1, 'hello', 12.34, x'DEADBEEF', '2026-01-02 03:04:05')"
                .to_string(),
        )
        .await
        .expect("insert failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == "sqlite_type_probe"),
        "list_tables should include sqlite_type_probe"
    );
    assert!(
        tables.iter().any(|t| t.name == "sqlite_type_probe_v"),
        "list_tables should include sqlite_type_probe_v"
    );

    let metadata = driver
        .get_table_metadata("main".to_string(), "sqlite_type_probe".to_string())
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
        .get_table_ddl("main".to_string(), "sqlite_type_probe".to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(
            "SELECT id, name, amount, payload, created_at FROM sqlite_type_probe WHERE id = 1"
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
        .execute_query("DROP VIEW IF EXISTS sqlite_type_probe_v".to_string())
        .await;
    let _ = driver
        .execute_query("DROP TABLE IF EXISTS sqlite_type_probe".to_string())
        .await;
    driver.close().await;

    let _ = std::fs::remove_file(db_path);
}
