#[path = "common/doris_context.rs"]
mod doris_context;

use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;

#[tokio::test]
#[ignore]
async fn test_doris_integration_flow() {
    let form = doris_context::shared_doris_form();

    let driver: MysqlDriver =
        doris_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!dbs.is_empty(), "list_databases returned empty");

    let db_name = doris_context::unique_name("dbpaw_doris_it");
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
async fn test_doris_metadata_and_type_mapping_flow() {
    let form = doris_context::shared_doris_form();

    let driver: MysqlDriver =
        doris_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = doris_context::unique_name("dbpaw_doris_type_db");
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
                id INT, \
                amount DECIMAL(10,2), \
                created_at DATETIME, \
                name VARCHAR(50)\
            )",
            qualified
        ))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, amount, created_at, name) \
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
        .execute_query(format!(
            "SELECT amount, created_at, name FROM {} WHERE id = 1",
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
    assert_eq!(
        row["name"],
        serde_json::Value::String("hello".to_string()),
        "name should be 'hello'"
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
}

#[tokio::test]
#[ignore]
async fn test_doris_list_databases_and_tables() {
    let form = doris_context::shared_doris_form();

    let driver: MysqlDriver =
        doris_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = doris_context::unique_name("dbpaw_doris_probe");
    let table_name = "probe_tbl";

    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");

    driver
        .execute_query(format!(
            "CREATE TABLE IF NOT EXISTS `{}`.`{}` (id INT)",
            db_name, table_name
        ))
        .await
        .expect("create table failed");

    let dbs = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(
        dbs.iter().any(|db| *db == db_name),
        "list_databases should include {}",
        db_name
    );

    let tables = driver
        .list_tables(Some(db_name.clone()))
        .await
        .expect("list_tables failed");
    assert!(
        tables.iter().any(|t| t.name == table_name),
        "list_tables should include {}.{}",
        db_name,
        table_name
    );

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_doris_get_table_data_supports_pagination_sort_filter() {
    let form = doris_context::shared_doris_form();

    let driver: MysqlDriver =
        doris_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = doris_context::unique_name("dbpaw_doris_grid_db");
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
            "CREATE TABLE {} (id INT, name STRING, score INT)",
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

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_doris_table_structure_and_schema_overview() {
    let form = doris_context::shared_doris_form();

    let driver: MysqlDriver =
        doris_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = doris_context::unique_name("dbpaw_doris_overview_db");
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
            "CREATE TABLE {} (id INT, label VARCHAR(30))",
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
}

#[tokio::test]
#[ignore]
async fn test_doris_execute_query_reports_affected_rows() {
    let form = doris_context::shared_doris_form();

    let driver: MysqlDriver =
        doris_context::connect_with_retry(|| MysqlDriver::connect(&form)).await;

    let db_name = doris_context::unique_name("dbpaw_doris_affected_db");
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
            "CREATE TABLE {} (id INT, name STRING)",
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

    let deleted = driver
        .execute_query(format!("DELETE FROM {} WHERE id IN (1, 2)", qualified))
        .await
        .expect("delete affected_rows probe rows failed");
    assert_eq!(deleted.row_count, 2);

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
}
