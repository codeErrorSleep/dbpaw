use dbpaw_lib::db::drivers::duckdb::DuckdbDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;
use std::path::PathBuf;
use uuid::Uuid;

fn duckdb_test_path() -> PathBuf {
    if let Ok(v) = env::var("DUCKDB_IT_DB_PATH") {
        return PathBuf::from(v);
    }
    let mut p = env::temp_dir();
    p.push(format!(
        "dbpaw-duckdb-integration-{}.duckdb",
        Uuid::new_v4()
    ));
    p
}

#[tokio::test]
#[ignore]
async fn test_duckdb_integration_flow() {
    let db_path = duckdb_test_path();
    let db_path_str = db_path.to_string_lossy().to_string();

    let form = ConnectionForm {
        driver: "duckdb".to_string(),
        file_path: Some(db_path_str.clone()),
        ..Default::default()
    };

    let driver = DuckdbDriver::connect(&form)
        .await
        .expect("Failed to connect to duckdb");

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(dbs.iter().any(|db| db == "main"));

    driver
        .execute_query(
            "CREATE TABLE IF NOT EXISTS duck_type_probe (\
                id INTEGER PRIMARY KEY, \
                name VARCHAR, \
                amount DOUBLE\
            )"
            .to_string(),
        )
        .await
        .expect("create table failed");

    driver
        .execute_query(
            "INSERT INTO duck_type_probe (id, name, amount) \
             VALUES (1, 'hello', 12.34)"
                .to_string(),
        )
        .await
        .expect("insert failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == "duck_type_probe"),
        "list_tables should include duck_type_probe"
    );

    let metadata = driver
        .get_table_metadata("main".to_string(), "duck_type_probe".to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata.columns.iter().any(|c| c.name == "id"),
        "metadata should include id column"
    );

    let ddl = driver
        .get_table_ddl("main".to_string(), "duck_type_probe".to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query("SELECT id, name, amount FROM duck_type_probe WHERE id = 1".to_string())
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

    let _ = driver
        .execute_query("DROP TABLE IF EXISTS duck_type_probe".to_string())
        .await;
    driver.close().await;

    let _ = std::fs::remove_file(db_path);
}
