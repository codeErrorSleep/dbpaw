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
