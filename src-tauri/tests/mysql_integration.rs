use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::models::ConnectionForm;
use std::env;

#[tokio::test]
#[ignore]
async fn test_mysql_integration_flow() {
    // Retrieve connection info from environment variables
    // Defaults are set for a local MySQL instance often used in development
    let host = env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MYSQL_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse()
        .unwrap();
    let username = env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string());
    let password = env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    // Use a specific test database if provided, otherwise default to None (which might fail list_tables if implementation depends on it)
    // Looking at mysql.rs, list_tables uses self.form.database as default schema.
    let database = env::var("MYSQL_DB").ok();

    println!("Testing MySQL connection to {}:{}", host, port);

    let form = ConnectionForm {
        driver: "mysql".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: database.clone(),
        ..Default::default()
    };

    let driver: MysqlDriver = MysqlDriver::connect(&form)
        .await
        .expect("Failed to connect");

    // 1. Test Connection
    // This just runs "SELECT 1"
    let result = driver.test_connection().await;
    assert!(result.is_ok(), "Connection failed: {:?}", result.err());
    println!("Connection successful!");

    // 2. List Databases
    let dbs = driver.list_databases().await;
    assert!(dbs.is_ok(), "Failed to list databases: {:?}", dbs.err());
    let dbs = dbs.unwrap();
    println!("Databases found: {:?}", dbs);
    assert!(!dbs.is_empty());

    // 3. Operations requiring a specific database
    if let Some(db_name) = database {
        println!("Running operations on database: {}", db_name);

        // List Tables
        let tables = driver.list_tables(Some(db_name.clone())).await;
        assert!(tables.is_ok(), "Failed to list tables: {:?}", tables.err());
        let tables = tables.unwrap();
        println!("Tables: {:?}", tables);

        // Setup a test table
        let table_name = "test_driver_integration";
        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (id INT PRIMARY KEY, name VARCHAR(50))",
            table_name
        );
        let _ = driver
            .execute_query(create_sql)
            .await
            .expect("Failed to create table");

        // Insert
        let insert_sql = format!(
            "INSERT INTO {} (id, name) VALUES (1, 'Test Item')",
            table_name
        );
        // Clean up first just in case
        let _ = driver
            .execute_query(format!("DELETE FROM {} WHERE id = 1", table_name))
            .await;

        let insert_res = driver.execute_query(insert_sql).await;
        assert!(insert_res.is_ok(), "Insert failed: {:?}", insert_res.err());

        // Query
        let select_sql = format!("SELECT * FROM {} WHERE id = 1", table_name);
        let query_res = driver.execute_query(select_sql).await;
        assert!(query_res.is_ok(), "Select failed: {:?}", query_res.err());
        let query_data = query_res.unwrap();
        assert_eq!(query_data.row_count, 1);

        // Verify data content
        // data is Vec<serde_json::Value>
        if let Some(row) = query_data.data.first() {
            let name_val = row.get("name").and_then(|v| v.as_str());
            assert_eq!(name_val, Some("Test Item"));
        } else {
            panic!("No data returned");
        }

        // Clean up
        let drop_sql = format!("DROP TABLE {}", table_name);
        let _ = driver.execute_query(drop_sql).await;
        println!("Integration test finished successfully");
    } else {
        println!("Skipping table operations because MYSQL_DB env var is not set");
    }
}

#[tokio::test]
#[ignore]
async fn test_mysql_metadata_and_type_mapping_flow() {
    let host = env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MYSQL_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse()
        .unwrap();
    let username = env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string());
    let password = env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    let database = env::var("MYSQL_DB").unwrap_or_else(|_| "test_db".to_string());

    let form = ConnectionForm {
        driver: "mysql".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver: MysqlDriver = MysqlDriver::connect(&form)
        .await
        .expect("Failed to connect");

    let table_name = "dbpaw_type_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id INT PRIMARY KEY, \
                flag BOOLEAN, \
                amount DECIMAL(10,2), \
                created_at DATETIME, \
                payload VARBINARY(16), \
                note VARCHAR(50)\
            )",
            qualified
        ))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, flag, amount, created_at, payload, note) \
             VALUES (1, 1, 12.34, '2026-01-02 03:04:05', UNHEX('DEADBEEF'), 'hello')",
            qualified
        ))
        .await
        .expect("insert failed");

    let tables = driver
        .list_tables(Some(database.clone()))
        .await
        .expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == table_name),
        "list_tables should include {}",
        table_name
    );

    let tables_with_default_db = driver
        .list_tables(None)
        .await
        .expect("list_tables(None) should fallback to current database");
    assert!(
        tables_with_default_db.iter().any(|t| t.name == table_name),
        "list_tables(None) should include {} when MYSQL_DB is selected",
        table_name
    );

    let metadata = driver
        .get_table_metadata(database.clone(), table_name.to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata.columns.iter().any(|c| c.name == "payload"),
        "metadata should include payload column"
    );
    assert!(
        metadata
            .columns
            .iter()
            .any(|c| c.name == "id" && c.primary_key),
        "metadata should mark id as primary key"
    );

    let ddl = driver
        .get_table_ddl(database.clone(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(format!(
            "SELECT flag, amount, created_at, payload, note FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("select typed row failed");

    assert_eq!(result.row_count, 1);
    let row = result
        .data
        .first()
        .expect("typed result should include at least one row");
    assert!(row.get("flag").is_some(), "flag should exist");
    assert_eq!(
        row["amount"],
        serde_json::Value::String("12.34".to_string()),
        "amount should be rendered as string"
    );
    assert!(row.get("created_at").is_some(), "created_at should exist");
    assert!(
        row.get("payload").is_some(),
        "payload(VARBINARY) should be decodable without error"
    );

    let table_data = driver
        .get_table_data(
            database.clone(),
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

#[tokio::test]
#[ignore]
async fn test_mysql_list_databases_and_tables_with_binary_collation_database() {
    let host = env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MYSQL_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse()
        .unwrap();
    let username = env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string());
    let password = env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    let database = env::var("MYSQL_DB").unwrap_or_else(|_| "test_db".to_string());

    let form = ConnectionForm {
        driver: "mysql".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database),
        ..Default::default()
    };

    let driver: MysqlDriver = MysqlDriver::connect(&form)
        .await
        .expect("Failed to connect");

    let probe_db = "dbpaw_bin_probe";
    let probe_table = "probe_tbl";

    driver
        .execute_query(format!(
            "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET latin1 COLLATE latin1_bin",
            probe_db
        ))
        .await
        .expect("create binary-collation database failed");

    driver
        .execute_query(format!(
            "CREATE TABLE IF NOT EXISTS `{}`.`{}` (id INT PRIMARY KEY)",
            probe_db, probe_table
        ))
        .await
        .expect("create table in probe db failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed on binary-collation db");
    assert!(
        dbs.iter().any(|db| db == probe_db),
        "list_databases should include {}",
        probe_db
    );

    let tables = driver
        .list_tables(Some(probe_db.to_string()))
        .await
        .expect("list_tables failed on binary-collation db");
    assert!(
        tables.iter().any(|t| t.name == probe_table),
        "list_tables should include {}.{}",
        probe_db,
        probe_table
    );

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", probe_db))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_list_tables_with_unicode_table_name() {
    let host = env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MYSQL_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse()
        .unwrap();
    let username = env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string());
    let password = env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "123456".to_string());
    let database = env::var("MYSQL_DB").unwrap_or_else(|_| "test_db".to_string());

    let form = ConnectionForm {
        driver: "mysql".to_string(),
        host: Some(host),
        port: Some(port),
        username: Some(username),
        password: Some(password),
        database: Some(database.clone()),
        ..Default::default()
    };

    let driver: MysqlDriver = MysqlDriver::connect(&form)
        .await
        .expect("Failed to connect");

    let table_name = "dbpaw_中文_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(20))",
            qualified
        ))
        .await
        .expect("create unicode table failed");

    let tables = driver
        .list_tables(Some(database.clone()))
        .await
        .expect("list_tables failed for unicode table name");
    assert!(
        tables.iter().any(|t| t.name == table_name),
        "list_tables should include {}",
        table_name
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}
