use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;

#[tokio::test]
#[ignore]
async fn test_mariadb_integration_flow() {
    let host = env::var("MARIADB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MARIADB_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse()
        .unwrap();
    let username = env::var("MARIADB_USER").unwrap_or_else(|_| "root".to_string());
    let password = env::var("MARIADB_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    let database = env::var("MARIADB_DB").ok();

    let form = ConnectionForm {
        driver: "mariadb".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: database.clone(),
        ..Default::default()
    };

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("Failed to connect");

    driver.test_connection().await.expect("Connection failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("Failed to list databases");
    assert!(!dbs.is_empty());

    if let Some(db_name) = database {
        let table = "dbpaw_mariadb_integration";
        let qualified = format!("`{}`.`{}`", db_name, table);

        driver
            .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
            .await
            .expect("drop before test failed");

        driver
            .execute_query(format!(
                "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(50))",
                qualified
            ))
            .await
            .expect("create table failed");

        driver
            .execute_query(format!(
                "INSERT INTO {} (id, name) VALUES (1, 'MariaDB')",
                qualified
            ))
            .await
            .expect("insert failed");

        let result = driver
            .execute_query(format!("SELECT name FROM {} WHERE id = 1", qualified))
            .await
            .expect("select failed");
        assert_eq!(result.row_count, 1);
        assert_eq!(
            result.data[0].get("name").and_then(|v| v.as_str()),
            Some("MariaDB")
        );

        driver
            .execute_query(format!("DROP TABLE {}", qualified))
            .await
            .expect("drop after test failed");
    }
}

#[tokio::test]
#[ignore]
async fn test_mariadb_show_create_and_information_schema_compat() {
    let host = env::var("MARIADB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MARIADB_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse()
        .unwrap();
    let username = env::var("MARIADB_USER").unwrap_or_else(|_| "root".to_string());
    let password = env::var("MARIADB_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    let database = env::var("MARIADB_DB").unwrap_or_else(|_| "test".to_string());

    let form = ConnectionForm {
        driver: "mariadb".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver = MysqlDriver::connect(&form)
        .await
        .expect("Failed to connect");
    let table = "dbpaw_mariadb_meta";
    let qualified = format!("`{}`.`{}`", database, table);

    driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await
        .expect("drop before test failed");

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, payload VARCHAR(50))",
            qualified
        ))
        .await
        .expect("create table failed");

    let ddl = driver
        .get_table_ddl(database.clone(), table.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let overview = driver
        .get_schema_overview(Some(database.clone()))
        .await
        .expect("get_schema_overview failed");
    assert!(
        overview.tables.iter().any(|t| t.name == table),
        "schema overview should include {}",
        table
    );

    driver
        .execute_query(format!("DROP TABLE {}", qualified))
        .await
        .expect("drop after test failed");
}
