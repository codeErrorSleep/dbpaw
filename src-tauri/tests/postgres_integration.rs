use dbpaw_lib::db::drivers::postgres::PostgresDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;

#[tokio::test]
#[ignore]
async fn test_postgres_integration_flow() {
    let host = env::var("POSTGRES_HOST")
        .or_else(|_| env::var("PG_HOST"))
        .unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("POSTGRES_PORT")
        .or_else(|_| env::var("PG_PORT"))
        .unwrap_or_else(|_| "5432".to_string())
        .parse()
        .expect("POSTGRES_PORT should be a number");
    let username = env::var("POSTGRES_USER")
        .or_else(|_| env::var("PGUSER"))
        .unwrap_or_else(|_| "postgres".to_string());
    let password = env::var("POSTGRES_PASSWORD")
        .or_else(|_| env::var("PGPASSWORD"))
        .unwrap_or_else(|_| "postgres".to_string());
    let database = env::var("POSTGRES_DB")
        .or_else(|_| env::var("PGDATABASE"))
        .unwrap_or_else(|_| "postgres".to_string());

    let form = ConnectionForm {
        driver: "postgres".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver = PostgresDriver::connect(&form)
        .await
        .expect("Failed to connect to Postgres");

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let databases = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!databases.is_empty(), "list_databases returned empty");

    let table_name = "dbpaw_pg_type_probe";
    let qualified = format!("public.{}", table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id INT PRIMARY KEY, \
                amount NUMERIC(10,2), \
                created_at TIMESTAMPTZ, \
                payload BYTEA, \
                meta JSONB\
            )",
            qualified
        ))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, amount, created_at, payload, meta) VALUES \
             (1, 12.34, '2026-01-02T03:04:05Z', E'\\\\xDEADBEEF', '{{\"k\":\"v\"}}'::jsonb)",
            qualified
        ))
        .await
        .expect("insert failed");

    let tables = driver
        .list_tables(Some("public".to_string()))
        .await
        .expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.schema == "public" && t.name == table_name),
        "list_tables should include public.{}",
        table_name
    );

    let metadata = driver
        .get_table_metadata("public".to_string(), table_name.to_string())
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
        .get_table_ddl("public".to_string(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(format!(
            "SELECT amount, created_at, payload, meta FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("select typed row failed");
    assert_eq!(result.row_count, 1);

    let row = result
        .data
        .first()
        .expect("typed result should include at least one row");
    assert_eq!(
        row["amount"],
        serde_json::Value::String("12.34".to_string()),
        "amount should be rendered as string"
    );
    assert!(row.get("created_at").is_some(), "created_at should exist");
    assert!(row.get("payload").is_some(), "payload should exist");
    assert!(row.get("meta").is_some(), "meta should exist");

    let table_data = driver
        .get_table_data(
            "public".to_string(),
            table_name.to_string(),
            1,
            100,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("get_table_data failed");
    assert_eq!(table_data.total, 1);
    assert_eq!(table_data.data.len(), 1);
    let grid_row = table_data
        .data
        .first()
        .expect("table_data should include at least one row");
    assert_eq!(
        grid_row["amount"],
        serde_json::Value::String("12.34".to_string()),
        "amount should be rendered as string in table data"
    );
    assert!(
        grid_row.get("created_at").is_some(),
        "created_at should exist"
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}
