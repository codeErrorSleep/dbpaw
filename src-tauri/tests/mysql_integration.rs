#[path = "common/mysql_context.rs"]
mod mysql_context;

use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use std::env;
use testcontainers::clients::Cli;

#[tokio::test]
#[ignore]
async fn test_mysql_integration_flow() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form.database.clone();

    println!(
        "Testing MySQL connection to {}:{}",
        form.host.as_deref().unwrap_or("localhost"),
        form.port.unwrap_or(3306)
    );

    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

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
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");

    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

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
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());

    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

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
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");

    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

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

#[tokio::test]
#[ignore]
async fn test_mysql_get_table_data_supports_pagination_sort_filter_and_order_by() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_grid_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(20), score INT)",
            qualified
        ))
        .await
        .expect("create grid probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name, score) VALUES \
             (1, 'alpha', 10), (2, 'beta', 20), (3, 'gamma', 30), (4, 'delta', 40)",
            qualified
        ))
        .await
        .expect("insert grid probe rows failed");

    let page1 = driver
        .get_table_data(
            database.clone(),
            table_name.to_string(),
            1,
            2,
            Some("score".to_string()),
            Some("desc".to_string()),
            None,
            None,
        )
        .await
        .expect("get_table_data for page1 failed");
    assert_eq!(page1.total, 4);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(
        page1.data[0]["name"],
        serde_json::Value::String("delta".to_string())
    );

    let filtered = driver
        .get_table_data(
            database.clone(),
            table_name.to_string(),
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
            database.clone(),
            table_name.to_string(),
            1,
            1,
            Some("id".to_string()),
            Some("asc".to_string()),
            None,
            Some("name DESC".to_string()),
        )
        .await
        .expect("get_table_data with order_by priority failed");
    assert_eq!(ordered.total, 4);
    assert_eq!(ordered.data.len(), 1);
    assert_eq!(
        ordered.data[0]["name"],
        serde_json::Value::String("gamma".to_string())
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_get_table_data_rejects_invalid_sort_column() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_invalid_sort_probe";
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
        .expect("create invalid sort probe table failed");

    let result = driver
        .get_table_data(
            database.clone(),
            table_name.to_string(),
            1,
            10,
            Some("id desc".to_string()),
            Some("desc".to_string()),
            None,
            None,
        )
        .await;
    let err = result.expect_err("invalid sort column should return an error");
    assert!(
        err.contains("[VALIDATION_ERROR] Invalid sort column name"),
        "unexpected error: {}",
        err
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_table_structure_and_schema_overview() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_overview_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, label VARCHAR(30) NOT NULL)",
            qualified
        ))
        .await
        .expect("create overview probe table failed");

    let structure = driver
        .get_table_structure(database.clone(), table_name.to_string())
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
        "table structure should include label column"
    );

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

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_metadata_includes_indexes_and_foreign_keys() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let parent = "dbpaw_parent_meta_probe";
    let child = "dbpaw_child_meta_probe";
    let parent_qualified = format!("`{}`.`{}`", database, parent);
    let child_qualified = format!("`{}`.`{}`", database, child);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", child_qualified))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", parent_qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY)",
            parent_qualified
        ))
        .await
        .expect("create parent table failed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id INT PRIMARY KEY, \
                parent_id INT NOT NULL, \
                name VARCHAR(30), \
                INDEX idx_child_name (name), \
                CONSTRAINT fk_child_parent FOREIGN KEY (parent_id) REFERENCES {}(id)\
            )",
            child_qualified, parent_qualified
        ))
        .await
        .expect("create child table with fk/index failed");

    let metadata = driver
        .get_table_metadata(database.clone(), child.to_string())
        .await
        .expect("get_table_metadata for child failed");
    assert!(
        metadata
            .indexes
            .iter()
            .any(|i| i.name == "idx_child_name" && i.columns.contains(&"name".to_string())),
        "metadata should include idx_child_name index"
    );
    assert!(
        metadata.foreign_keys.iter().any(|fk| {
            fk.name == "fk_child_parent"
                && fk.column == "parent_id"
                && fk.referenced_table == parent
                && fk.referenced_column == "id"
        }),
        "metadata should include fk_child_parent"
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", child_qualified))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", parent_qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_boolean_and_json_type_mapping_regression() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_bool_json_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, flag BOOLEAN, meta JSON)",
            qualified
        ))
        .await
        .expect("create bool/json probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, flag, meta) VALUES (1, 1, '{{\"tier\": \"gold\"}}')",
            qualified
        ))
        .await
        .expect("insert bool/json probe row failed");

    let query_result = driver
        .execute_query(format!("SELECT flag, meta FROM {} WHERE id = 1", qualified))
        .await
        .expect("select bool/json row failed");
    assert_eq!(query_result.row_count, 1);
    let query_row = query_result.data.first().expect("query row should exist");
    assert!(query_row.get("flag").is_some(), "flag should exist");
    assert!(query_row.get("meta").is_some(), "meta should exist");

    let table_data = driver
        .get_table_data(
            database.clone(),
            table_name.to_string(),
            1,
            10,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("get_table_data for bool/json table failed");
    assert_eq!(table_data.total, 1);
    let grid_row = table_data
        .data
        .first()
        .expect("table data row should exist");
    assert!(
        grid_row.get("flag").is_some(),
        "flag should exist in table_data"
    );
    assert!(
        grid_row.get("meta").is_some(),
        "meta should exist in table_data"
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_transaction_commit_and_rollback() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_txn_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(30))",
            qualified
        ))
        .await
        .expect("create txn probe table failed");

    let mut rollback_tx = driver.pool.begin().await.expect("begin rollback tx failed");
    sqlx::query(&format!(
        "INSERT INTO {} (id, name) VALUES (1, 'rolled_back')",
        qualified
    ))
    .execute(&mut *rollback_tx)
    .await
    .expect("insert in rollback tx failed");
    rollback_tx.rollback().await.expect("rollback tx failed");

    let rolled_back = driver
        .execute_query(format!(
            "SELECT COUNT(*) AS c FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("count after rollback failed");
    assert_eq!(rolled_back.row_count, 1);
    let rolled_back_count = rolled_back.data[0]["c"]
        .as_str()
        .expect("rollback count should be string");
    assert_eq!(rolled_back_count, "0");

    let mut commit_tx = driver.pool.begin().await.expect("begin commit tx failed");
    sqlx::query(&format!(
        "INSERT INTO {} (id, name) VALUES (2, 'committed')",
        qualified
    ))
    .execute(&mut *commit_tx)
    .await
    .expect("insert in commit tx failed");
    commit_tx.commit().await.expect("commit tx failed");

    let committed = driver
        .execute_query(format!(
            "SELECT COUNT(*) AS c FROM {} WHERE id = 2",
            qualified
        ))
        .await
        .expect("count after commit failed");
    assert_eq!(committed.row_count, 1);
    let committed_count = committed.data[0]["c"]
        .as_str()
        .expect("commit count should be string");
    assert_eq!(committed_count, "1");

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_execute_query_reports_affected_rows_for_update_delete() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_affected_rows_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(30))",
            qualified
        ))
        .await
        .expect("create affected_rows probe table failed");

    let inserted = driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'a'), (2, 'b')",
            qualified
        ))
        .await
        .expect("insert affected_rows probe rows failed");
    assert_eq!(inserted.row_count, 2);

    let updated = driver
        .execute_query(format!("UPDATE {} SET name = 'bb' WHERE id = 2", qualified))
        .await
        .expect("update affected_rows probe row failed");
    assert_eq!(updated.row_count, 1);

    let deleted = driver
        .execute_query(format!("DELETE FROM {} WHERE id IN (1, 2)", qualified))
        .await
        .expect("delete affected_rows probe rows failed");
    assert_eq!(deleted.row_count, 2);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_large_text_and_blob_round_trip() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_large_field_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, body LONGTEXT, payload LONGBLOB)",
            qualified
        ))
        .await
        .expect("create large field probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, body, payload) VALUES (1, REPEAT('x', 70000), UNHEX(REPEAT('AB', 2048)))",
            qualified
        ))
        .await
        .expect("insert large field probe row failed");

    let result = driver
        .execute_query(format!(
            "SELECT body, payload FROM {} WHERE id = 1",
            qualified
        ))
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
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_error_handling_for_sql_error() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let err = driver
        .execute_query("SELECT * FROM __dbpaw_table_not_exists".to_string())
        .await
        .expect_err("invalid SQL should return query error");
    assert!(
        err.contains("[QUERY_ERROR]"),
        "unexpected error shape: {}",
        err
    );
}

#[tokio::test]
#[ignore]
async fn test_mysql_concurrent_connections_can_query() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let mut handles = Vec::new();

    for _ in 0..8 {
        let task_form = form.clone();
        handles.push(tokio::spawn(async move {
            let driver =
                mysql_context::connect_with_retry(|| MysqlDriver::connect(&task_form)).await;
            driver.execute_query("SELECT 1 AS ok".to_string()).await
        }));
    }

    for handle in handles {
        let result = handle.await.expect("concurrent mysql task panicked");
        let data = result.expect("concurrent mysql query failed");
        assert_eq!(data.row_count, 1);
        let ok = data.data[0]["ok"]
            .as_str()
            .expect("ok should be a stringified scalar");
        assert_eq!(ok, "1");
    }
}

#[tokio::test]
#[ignore]
async fn test_mysql_view_can_be_listed_and_queried() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let base_table = "dbpaw_view_base_probe";
    let view_name = "dbpaw_view_probe_v";
    let qualified_table = format!("`{}`.`{}`", database, base_table);
    let qualified_view = format!("`{}`.`{}`", database, view_name);

    let _ = driver
        .execute_query(format!("DROP VIEW IF EXISTS {}", qualified_view))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified_table))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(30), score INT)",
            qualified_table
        ))
        .await
        .expect("create base table for view failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name, score) VALUES (1, 'alice', 10), (2, 'bob', 20)",
            qualified_table
        ))
        .await
        .expect("insert base rows for view failed");
    driver
        .execute_query(format!(
            "CREATE VIEW {} AS SELECT id, name FROM {} WHERE score >= 20",
            qualified_view, qualified_table
        ))
        .await
        .expect("create view failed");

    let tables = driver
        .list_tables(Some(database.clone()))
        .await
        .expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.name == base_table && t.r#type == "table"),
        "list_tables should include base table"
    );
    assert!(
        tables
            .iter()
            .any(|t| t.name == view_name && t.r#type == "view"),
        "list_tables should include view with type=view"
    );

    let view_rows = driver
        .execute_query(format!(
            "SELECT id, name FROM {} ORDER BY id",
            qualified_view
        ))
        .await
        .expect("select from view failed");
    assert_eq!(view_rows.row_count, 1);
    let row = view_rows.data.first().expect("view row should exist");
    let id_matches = row["id"] == serde_json::Value::Number(2.into())
        || row["id"] == serde_json::Value::String("2".to_string());
    assert!(id_matches, "unexpected id payload: {}", row["id"]);
    assert_eq!(row["name"], serde_json::Value::String("bob".to_string()));

    let _ = driver
        .execute_query(format!("DROP VIEW IF EXISTS {}", qualified_view))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified_table))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_connection_failure_with_wrong_password() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, mut form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    form.password = Some("dbpaw_wrong_password".to_string());

    let err = match MysqlDriver::connect(&form).await {
        Ok(_) => panic!("wrong password should fail"),
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
async fn test_mysql_connection_timeout_or_unreachable_host_error() {
    let form = dbpaw_lib::models::ConnectionForm {
        driver: "mysql".to_string(),
        host: Some("203.0.113.1".to_string()),
        port: Some(3306),
        username: Some("root".to_string()),
        password: Some("123456".to_string()),
        database: Some("test_db".to_string()),
        ssl: Some(false),
        ..Default::default()
    };

    let err = match MysqlDriver::connect(&form).await {
        Ok(_) => panic!("unreachable host should fail"),
        Err(err) => err,
    };
    assert!(
        err.starts_with("[CONN_FAILED]"),
        "unexpected error: {}",
        err
    );
    assert!(
        err.contains("could not reach the server")
            || err.to_ascii_lowercase().contains("timed out")
            || err.to_ascii_lowercase().contains("timeout")
            || err.to_ascii_lowercase().contains("network unreachable"),
        "unexpected timeout/unreachable error: {}",
        err
    );
}

#[tokio::test]
#[ignore]
async fn test_mysql_test_connection_fallback_when_prepare_unsupported() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let expect_prepare_unsupported =
        env::var("MYSQL_EXPECT_PREPARED_UNSUPPORTED").unwrap_or_default() == "1";

    let mut conn = driver
        .pool
        .acquire()
        .await
        .expect("acquire mysql pooled connection failed");
    let prepare_probe = sqlx::query("SELECT 1").execute(&mut *conn).await;

    match prepare_probe {
        Ok(_) => {
            assert!(
                !expect_prepare_unsupported,
                "expected prepared protocol unsupported but probe succeeded"
            );
        }
        Err(err) => {
            let err_text = err.to_string().to_ascii_lowercase();
            assert!(
                err_text.contains("1295")
                    || err_text.contains("prepare does not support sql")
                    || err_text.contains("prepared statement protocol"),
                "unexpected prepare probe error: {}",
                err
            );
            driver
                .test_connection()
                .await
                .expect("test_connection should fallback to raw_sql when prepare is unsupported");
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_mysql_batch_insert_and_batch_execute_flow() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_batch_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, category VARCHAR(20), score INT)",
            qualified
        ))
        .await
        .expect("create batch probe table failed");

    let value_rows: Vec<String> = (1..=50)
        .map(|id| {
            let category = if id <= 25 { "alpha" } else { "beta" };
            format!("({}, '{}', {})", id, category, id)
        })
        .collect();
    let insert_sql = format!(
        "INSERT INTO {} (id, category, score) VALUES {}",
        qualified,
        value_rows.join(", ")
    );
    let inserted = driver
        .execute_query(insert_sql)
        .await
        .expect("batch insert failed");
    assert_eq!(inserted.row_count, 50);

    let batch_sqls = vec![
        format!(
            "UPDATE {} SET score = score + 100 WHERE id <= 10",
            qualified
        ),
        format!(
            "UPDATE {} SET category = 'gamma' WHERE id BETWEEN 30 AND 40",
            qualified
        ),
        format!("DELETE FROM {} WHERE id IN (3, 6, 9, 12, 15)", qualified),
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
        .execute_query(format!("SELECT COUNT(*) AS c FROM {}", qualified))
        .await
        .expect("count after batch execute failed");
    let total = check_total.data[0]["c"]
        .as_str()
        .expect("count should be string")
        .parse::<i64>()
        .expect("count should be numeric");
    assert_eq!(total, 45);

    let check_gamma = driver
        .execute_query(format!(
            "SELECT COUNT(*) AS c FROM {} WHERE category = 'gamma'",
            qualified
        ))
        .await
        .expect("count gamma rows failed");
    let gamma = check_gamma.data[0]["c"]
        .as_str()
        .expect("gamma count should be string")
        .parse::<i64>()
        .expect("gamma count should be numeric");
    assert_eq!(gamma, 11);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_mysql_prepared_statements_prepare_execute_and_deallocate() {
    let docker = (!mysql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mysql_context::mysql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MYSQL_DB or container default database should be present");
    let driver: MysqlDriver =
        mysql_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let table_name = "dbpaw_prepared_stmt_probe";
    let qualified = format!("`{}`.`{}`", database, table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name VARCHAR(30))",
            qualified
        ))
        .await
        .expect("create prepared stmt probe table failed");

    let mut conn = driver
        .pool
        .acquire()
        .await
        .expect("acquire mysql pooled connection failed");
    let prepared_insert_sql = format!("INSERT INTO {} (id, name) VALUES (?, ?)", qualified);
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

    let prepared_update_sql = format!("UPDATE {} SET name = ? WHERE id = ?", qualified);
    let updated = sqlx::query(&prepared_update_sql)
        .bind("alice-updated")
        .bind(1_i64)
        .execute(&mut *conn)
        .await
        .expect("prepared update failed");
    assert_eq!(updated.rows_affected(), 1);

    let prepared_select_sql = format!("SELECT name FROM {} WHERE id = ?", qualified);
    let selected_name: String = sqlx::query_scalar(&prepared_select_sql)
        .bind(1_i64)
        .fetch_one(&mut *conn)
        .await
        .expect("prepared select failed");
    assert_eq!(selected_name, "alice-updated");
    drop(conn);

    let verify = driver
        .execute_query(format!("SELECT COUNT(*) AS c FROM {}", qualified))
        .await
        .expect("verify prepared writes failed");
    let total = verify.data[0]["c"]
        .as_str()
        .expect("verify count should be string")
        .parse::<i64>()
        .expect("verify count should parse");
    assert_eq!(total, 2);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}
