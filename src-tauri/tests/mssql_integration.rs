use dbpaw_lib::db::drivers::mssql::MssqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;

#[tokio::test]
#[ignore]
async fn test_mssql_integration_flow() {
    let host = env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MSSQL_PORT")
        .unwrap_or_else(|_| "1433".to_string())
        .parse()
        .expect("MSSQL_PORT should be a number");
    let username = env::var("MSSQL_USER").unwrap_or_else(|_| "sa".to_string());
    let password = env::var("MSSQL_PASSWORD").unwrap_or_default();
    let database = env::var("MSSQL_DB").unwrap_or_else(|_| "master".to_string());

    let form = ConnectionForm {
        driver: "mssql".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver = MssqlDriver::connect(&form)
        .await
        .expect("Failed to connect to SQL Server");

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let databases = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!databases.is_empty(), "list_databases returned empty");

    let _tables = driver.list_tables(None).await.expect("list_tables failed");

    let result = driver
        .execute_query("SELECT TOP 1 name FROM sys.databases".to_string())
        .await
        .expect("execute_query failed");
    assert!(result.row_count >= 1);

    driver
        .execute_query(
            "IF OBJECT_ID('dbo.dbpaw_type_probe', 'U') IS NOT NULL DROP TABLE dbo.dbpaw_type_probe;"
                .to_string(),
        )
        .await
        .expect("drop table failed");

    driver
        .execute_query(
            "CREATE TABLE dbo.dbpaw_type_probe (id INT PRIMARY KEY, flag BIT, amount DECIMAL(10,2), created_at DATETIME2);"
                .to_string(),
        )
        .await
        .expect("create table failed");

    driver
        .execute_query(
            "INSERT INTO dbo.dbpaw_type_probe (id, flag, amount, created_at) VALUES (1, 1, 12.34, '2026-01-02T03:04:05');"
                .to_string(),
        )
        .await
        .expect("insert failed");

    let typed_result = driver
        .execute_query(
            "SELECT TOP 1 flag, amount, created_at FROM dbo.dbpaw_type_probe ORDER BY id DESC"
                .to_string(),
        )
        .await
        .expect("select typed row failed");

    let row = typed_result
        .data
        .first()
        .expect("typed_result should include at least one row");
    assert_eq!(row["flag"], serde_json::Value::Bool(true));
    assert_eq!(
        row["amount"],
        serde_json::Value::String("12.34".to_string())
    );
    assert!(
        row["created_at"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "created_at should be rendered as a non-empty string"
    );
}

#[tokio::test]
#[ignore]
async fn test_mssql_metadata_and_ddl_with_special_table_name() {
    let host = env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MSSQL_PORT")
        .unwrap_or_else(|_| "1433".to_string())
        .parse()
        .expect("MSSQL_PORT should be a number");
    let username = env::var("MSSQL_USER").unwrap_or_else(|_| "sa".to_string());
    let password = env::var("MSSQL_PASSWORD").unwrap_or_default();
    let database = env::var("MSSQL_DB").unwrap_or_else(|_| "master".to_string());

    let form = ConnectionForm {
        driver: "mssql".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database),
        ..Default::default()
    };

    let driver = MssqlDriver::connect(&form)
        .await
        .expect("Failed to connect to SQL Server");

    let schema = "dbo";
    let table_name = "dbpaw type-probe";
    let qualified = format!("[{}].[{}]", schema, table_name);

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'{}.{}', N'U') IS NOT NULL DROP TABLE {};",
            schema, table_name, qualified
        ))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, note NVARCHAR(50));",
            qualified
        ))
        .await
        .expect("create special-name table failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.schema == schema && t.name == table_name),
        "list_tables should include special-name table"
    );

    let metadata = driver
        .get_table_metadata(schema.to_string(), table_name.to_string())
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
        metadata.columns.iter().any(|c| c.name == "note"),
        "metadata should include note column"
    );

    let ddl = driver
        .get_table_ddl(schema.to_string(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'{}.{}', N'U') IS NOT NULL DROP TABLE {};",
            schema, table_name, qualified
        ))
        .await;
}
