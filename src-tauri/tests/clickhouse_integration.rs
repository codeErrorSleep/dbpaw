use dbpaw_lib::db::drivers::clickhouse::ClickHouseDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;

#[tokio::test]
#[ignore]
async fn test_clickhouse_integration_flow() {
    let host = env::var("CLICKHOUSE_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("CLICKHOUSE_PORT")
        .unwrap_or_else(|_| "8123".to_string())
        .parse()
        .unwrap();
    let username = env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string());
    let password = env::var("CLICKHOUSE_PASSWORD").unwrap_or_default();
    let database = env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "default".to_string());

    let form = ConnectionForm {
        driver: "clickhouse".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver = ClickHouseDriver::connect(&form)
        .await
        .expect("Failed to connect to ClickHouse");

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let databases = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!databases.is_empty(), "list_databases returned empty");

    let tables = driver
        .list_tables(Some(database.clone()))
        .await
        .expect("list_tables failed");

    if let Some(first_table) = tables.first() {
        let _metadata = driver
            .get_table_metadata(first_table.schema.clone(), first_table.name.clone())
            .await
            .expect("get_table_metadata failed");

        let _ddl = driver
            .get_table_ddl(first_table.schema.clone(), first_table.name.clone())
            .await
            .expect("get_table_ddl failed");
    }

    let query_result = driver
        .execute_query("SELECT 1 AS ok".to_string())
        .await
        .expect("execute_query failed");
    assert_eq!(query_result.row_count, 1);

    let overview = driver
        .get_schema_overview(Some(database))
        .await
        .expect("get_schema_overview failed");
    assert!(
        !overview.tables.is_empty() || tables.is_empty(),
        "schema overview expected to have tables when list_tables has entries"
    );

    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_type_mapping_and_metadata_flow() {
    let host = env::var("CLICKHOUSE_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("CLICKHOUSE_PORT")
        .unwrap_or_else(|_| "8123".to_string())
        .parse()
        .unwrap();
    let username = env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string());
    let password = env::var("CLICKHOUSE_PASSWORD").unwrap_or_default();
    let database = env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "default".to_string());

    let form = ConnectionForm {
        driver: "clickhouse".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver = ClickHouseDriver::connect(&form)
        .await
        .expect("Failed to connect to ClickHouse");

    let table_name = "dbpaw_ch_type_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id UInt32, \
                amount Decimal(10,2), \
                created_at DateTime, \
                note Nullable(String)\
            ) ENGINE = MergeTree ORDER BY id",
            qualified
        ))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, amount, created_at, note) VALUES \
             (1, 12.34, toDateTime('2026-01-02 03:04:05'), NULL)",
            qualified
        ))
        .await
        .expect("insert probe row failed");

    // 1) list_databases/list_tables
    let databases = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(
        databases.iter().any(|d| d == &database),
        "list_databases should include active database {}",
        database
    );

    let tables = driver
        .list_tables(Some(database.clone()))
        .await
        .expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.schema == database && t.name == table_name),
        "list_tables should include {}.{}",
        database,
        table_name
    );

    // 2) execute_query type mapping (Decimal/DateTime/Nullable)
    let result = driver
        .execute_query(format!(
            "SELECT amount, created_at, note FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("select typed row failed");
    assert_eq!(result.row_count, 1);
    let row = result
        .data
        .first()
        .expect("typed result should include at least one row");

    assert!(row.get("amount").is_some(), "amount should exist");
    assert!(
        row["amount"].is_number() || row["amount"].is_string(),
        "Decimal should be represented as number or string in JSON"
    );
    assert!(
        row["created_at"].is_string(),
        "DateTime should be represented as string in JSON"
    );
    assert!(row["note"].is_null(), "Nullable(String) should decode NULL");

    // 3) schema overview + DDL
    let overview = driver
        .get_schema_overview(Some(database.clone()))
        .await
        .expect("get_schema_overview failed");
    assert!(
        overview
            .tables
            .iter()
            .any(|t| t.schema == database && t.name == table_name),
        "schema overview should include {}.{}",
        database,
        table_name
    );

    let ddl = driver
        .get_table_ddl(database.clone(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.contains(table_name) && ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE and table name"
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver.close().await;
}
