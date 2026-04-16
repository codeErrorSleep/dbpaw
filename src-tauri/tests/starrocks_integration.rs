#[path = "common/starrocks_context.rs"]
mod starrocks_context;

use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use std::time::{SystemTime, UNIX_EPOCH};
use testcontainers::clients::Cli;

fn unique_name(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_millis();
    format!("{}_{}", prefix, millis)
}

#[tokio::test]
#[ignore]
async fn test_starrocks_integration_flow() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!dbs.is_empty(), "list_databases returned empty");

    let db_name = unique_name("dbpaw_starrocks_it");
    let table_name = "events";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!("CREATE TABLE {} (id INT, name STRING)", qualified))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'hello')",
            qualified
        ))
        .await
        .expect("insert failed");

    let tables = driver
        .list_tables(Some(db_name.clone()))
        .await
        .expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == table_name),
        "list_tables should include {}",
        table_name
    );

    let metadata = driver
        .get_table_metadata(db_name.clone(), table_name.to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata.columns.iter().any(|c| c.name == "name"),
        "metadata should include name column"
    );

    let ddl = driver
        .get_table_ddl(db_name.clone(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(format!("SELECT id, name FROM {} WHERE id = 1", qualified))
        .await
        .expect("select failed");
    assert_eq!(result.row_count, 1);
    assert_eq!(
        result.data[0]["name"],
        serde_json::Value::String("hello".to_string())
    );

    let table_data = driver
        .get_table_data(
            db_name.clone(),
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

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_metadata_and_type_mapping_flow() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = unique_name("dbpaw_starrocks_type_db");
    let table_name = "dbpaw_type_probe";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id INT PRIMARY KEY, \
                amount DECIMAL(10,2), \
                created_at DATETIME, \
                note VARCHAR(50)\
            )",
            qualified
        ))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, amount, created_at, note) \
             VALUES (1, 12.34, '2026-01-02 03:04:05', 'hello')",
            qualified
        ))
        .await
        .expect("insert failed");

    let tables = driver
        .list_tables(Some(db_name.clone()))
        .await
        .expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == table_name),
        "list_tables should include {}",
        table_name
    );

    let metadata = driver
        .get_table_metadata(db_name.clone(), table_name.to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata.columns.iter().any(|c| c.name == "amount"),
        "metadata should include amount column"
    );

    let ddl = driver
        .get_table_ddl(db_name.clone(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

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
    assert!(row.get("created_at").is_some(), "created_at should exist");
    assert!(row.get("note").is_some(), "note should exist");

    let table_data = driver
        .get_table_data(
            db_name.clone(),
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

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_list_tables_with_unicode_table_name() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = unique_name("dbpaw_starrocks_unicode_db");
    let table_name = "dbpaw_中文_probe";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

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
        .list_tables(Some(db_name.clone()))
        .await
        .expect("list_tables failed for unicode table name");
    assert!(
        tables.iter().any(|t| t.name == table_name),
        "list_tables should include {}",
        table_name
    );

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_get_table_data_supports_pagination_sort_filter_and_order_by() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = unique_name("dbpaw_starrocks_grid_db");
    let table_name = "dbpaw_grid_probe";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

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

    // Test pagination
    let page1 = driver
        .get_table_data(
            db_name.clone(),
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

    // Test filter
    let filtered = driver
        .get_table_data(
            db_name.clone(),
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

    // Test order_by priority
    let ordered = driver
        .get_table_data(
            db_name.clone(),
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
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_get_table_data_rejects_invalid_sort_column() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = unique_name("dbpaw_starrocks_invalid_sort_db");
    let table_name = "dbpaw_invalid_sort_probe";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

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
            db_name.clone(),
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
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_table_structure_and_schema_overview() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = unique_name("dbpaw_starrocks_overview_db");
    let table_name = "dbpaw_overview_probe";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

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
        .get_table_structure(db_name.clone(), table_name.to_string())
        .await
        .expect("get_table_structure failed");
    assert!(
        structure.columns.iter().any(|c| c.name == "id"),
        "table structure should include id column"
    );
    assert!(
        structure.columns.iter().any(|c| c.name == "label"),
        "table structure should include label column"
    );

    let overview = driver
        .get_schema_overview(Some(db_name.clone()))
        .await
        .expect("get_schema_overview failed");
    assert!(
        overview
            .tables
            .iter()
            .any(|t| t.schema == db_name && t.name == table_name),
        "schema overview should include {}.{}",
        db_name,
        table_name
    );

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_execute_query_reports_affected_rows_for_update_delete() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    let driver: MysqlDriver =
        starrocks_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = unique_name("dbpaw_starrocks_affected_db");
    let table_name = "dbpaw_affected_rows_probe";
    let qualified = format!("`{}`.`{}`", db_name, table_name);

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

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
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}
