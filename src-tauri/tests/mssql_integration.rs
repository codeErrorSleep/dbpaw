#[path = "common/mssql_context.rs"]
mod mssql_context;

use dbpaw_lib::db::drivers::mssql::MssqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use testcontainers::clients::Cli;

#[tokio::test]
#[ignore]
async fn test_mssql_integration_flow() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let database = form
        .database
        .clone()
        .expect("MSSQL_DB or container default database should be present");
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    driver
        .test_connection()
        .await
        .expect("test_connection failed");

    let databases = driver
        .list_databases()
        .await
        .expect("list_databases failed");
    assert!(!databases.is_empty(), "list_databases returned empty");
    assert!(
        databases.iter().any(|db| db == &database),
        "list_databases should include {}",
        database
    );

    let table_name = "dbpaw_mssql_type_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id INT PRIMARY KEY, \
                name NVARCHAR(50), \
                amount DECIMAL(10,2), \
                payload VARBINARY(16), \
                created_at DATETIME2\
            )",
            qualified
        ))
        .await
        .expect("create table failed");

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name, amount, payload, created_at) \
             VALUES (1, N'hello', 12.34, 0xDEADBEEF, '2026-01-02T03:04:05')",
            qualified
        ))
        .await
        .expect("insert failed");

    let tables = driver.list_tables(None).await.expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.schema == "dbo" && t.name == table_name),
        "list_tables should include dbo.{}",
        table_name
    );

    let metadata = driver
        .get_table_metadata("dbo".to_string(), table_name.to_string())
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
        .get_table_ddl("dbo".to_string(), table_name.to_string())
        .await
        .expect("get_table_ddl failed");
    assert!(
        ddl.to_uppercase().contains("CREATE TABLE"),
        "DDL should contain CREATE TABLE"
    );

    let result = driver
        .execute_query(format!(
            "SELECT id, name, amount, created_at FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("select typed row failed");
    assert_eq!(result.row_count, 1);
    let row = result
        .data
        .first()
        .expect("typed result should include at least one row");
    let id_value = row.get("id").expect("id should exist");
    assert!(
        id_value == &serde_json::Value::String("1".to_string())
            || id_value == &serde_json::Value::Number(serde_json::Number::from(1)),
        "unexpected id value: {:?}",
        id_value
    );
    assert_eq!(row["name"], serde_json::Value::String("hello".to_string()));
    assert!(row.get("amount").is_some(), "amount should exist");
    assert!(row.get("created_at").is_some(), "created_at should exist");

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_get_table_data_supports_pagination_sort_filter_and_order_by() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_grid_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name NVARCHAR(30), score INT)",
            qualified
        ))
        .await
        .expect("create dbpaw_mssql_grid_probe failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name, score) VALUES \
             (1, N'alpha', 10), (2, N'beta', 20), (3, N'gamma', 30), (4, N'delta', 40)",
            qualified
        ))
        .await
        .expect("insert dbpaw_mssql_grid_probe failed");

    let page1 = driver
        .get_table_data(
            "dbo".to_string(),
            table_name.to_string(),
            1,
            2,
            Some("score".to_string()),
            Some("desc".to_string()),
            None,
            None,
        )
        .await
        .expect("get_table_data page1 failed");
    assert_eq!(page1.total, 4);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(
        page1.data[0]["name"],
        serde_json::Value::String("delta".to_string())
    );

    let filtered = driver
        .get_table_data(
            "dbo".to_string(),
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
            "dbo".to_string(),
            table_name.to_string(),
            1,
            1,
            Some("id".to_string()),
            Some("asc".to_string()),
            None,
            Some("name DESC".to_string()),
        )
        .await
        .expect("get_table_data with order_by failed");
    assert_eq!(ordered.total, 4);
    assert_eq!(ordered.data.len(), 1);
    assert_eq!(
        ordered.data[0]["name"],
        serde_json::Value::String("gamma".to_string())
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_get_table_data_rejects_invalid_sort_column() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_invalid_sort_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!("CREATE TABLE {} (id INT PRIMARY KEY)", qualified))
        .await
        .expect("create dbpaw_mssql_invalid_sort_probe failed");

    let result = driver
        .get_table_data(
            "dbo".to_string(),
            table_name.to_string(),
            1,
            10,
            Some("id desc".to_string()),
            Some("desc".to_string()),
            None,
            None,
        )
        .await;
    let err = result.expect_err("invalid sort column should return error");
    assert!(
        err.contains("[VALIDATION_ERROR] Invalid sort column name")
            || err.contains("Invalid column name"),
        "unexpected error: {}",
        err
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_table_structure_and_schema_overview() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_overview_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, label NVARCHAR(50) NOT NULL)",
            qualified
        ))
        .await
        .expect("create dbpaw_mssql_overview_probe failed");

    let structure = driver
        .get_table_structure("dbo".to_string(), table_name.to_string())
        .await
        .expect("get_table_structure failed");
    assert!(
        structure.columns.iter().any(|c| c.name == "id"),
        "table structure should include id"
    );
    assert!(
        structure.columns.iter().any(|c| c.name == "label"),
        "table structure should include label"
    );

    let overview = driver
        .get_schema_overview(Some("dbo".to_string()))
        .await
        .expect("get_schema_overview failed");
    assert!(
        overview
            .tables
            .iter()
            .any(|t| t.schema == "dbo" && t.name == table_name),
        "schema overview should include dbo.{}",
        table_name
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_metadata_includes_indexes_and_foreign_keys() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let parent = "dbpaw_mssql_parent_meta_probe";
    let child = "dbpaw_mssql_child_meta_probe";
    let parent_qualified = format!("[dbo].[{}]", parent);
    let child_qualified = format!("[dbo].[{}]", child);

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            child, child_qualified
        ))
        .await;
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            parent, parent_qualified
        ))
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
                name NVARCHAR(30), \
                CONSTRAINT fk_mssql_child_parent FOREIGN KEY (parent_id) REFERENCES {}(id)\
            )",
            child_qualified, parent_qualified
        ))
        .await
        .expect("create child table with fk failed");
    driver
        .execute_query(format!(
            "CREATE INDEX idx_mssql_child_name ON {} (name)",
            child_qualified
        ))
        .await
        .expect("create index failed");

    let metadata = driver
        .get_table_metadata("dbo".to_string(), child.to_string())
        .await
        .expect("get_table_metadata failed");
    assert!(
        metadata
            .indexes
            .iter()
            .any(|i| i.name == "idx_mssql_child_name" && i.columns.contains(&"name".to_string())),
        "metadata should include idx_mssql_child_name"
    );
    assert!(
        metadata
            .foreign_keys
            .iter()
            .any(|fk| fk.column == "parent_id" && fk.referenced_table == parent),
        "metadata should include FK parent_id -> {}(id)",
        parent
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            child, child_qualified
        ))
        .await;
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            parent, parent_qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_boolean_and_json_type_mapping_regression() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_bool_json_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, flag BIT, meta NVARCHAR(MAX))",
            qualified
        ))
        .await
        .expect("create bool/json probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, flag, meta) VALUES (1, 1, N'{{\"tier\":\"gold\"}}')",
            qualified
        ))
        .await
        .expect("insert bool/json probe row failed");

    let query_result = driver
        .execute_query(format!(
            "SELECT flag, JSON_VALUE(meta, '$.tier') AS tier FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("select bool/json row failed");
    assert_eq!(query_result.row_count, 1);
    let query_row = query_result.data.first().expect("query row should exist");
    let query_flag = query_row
        .get("flag")
        .expect("flag should exist in query result");
    assert!(
        query_flag == &serde_json::Value::Bool(true)
            || query_flag == &serde_json::Value::Number(serde_json::Number::from(1)),
        "unexpected query flag value: {:?}",
        query_flag
    );
    assert_eq!(
        query_row["tier"],
        serde_json::Value::String("gold".to_string())
    );

    let table_data = driver
        .get_table_data(
            "dbo".to_string(),
            table_name.to_string(),
            1,
            10,
            None,
            None,
            None,
            None,
        )
        .await
        .expect("get_table_data for bool/json probe failed");
    assert_eq!(table_data.total, 1);
    let grid_row = table_data.data.first().expect("table row should exist");
    let grid_flag = grid_row
        .get("flag")
        .expect("flag should exist in table_data result");
    assert!(
        grid_flag == &serde_json::Value::Bool(true)
            || grid_flag == &serde_json::Value::Number(serde_json::Number::from(1)),
        "unexpected grid flag value: {:?}",
        grid_flag
    );
    assert!(
        grid_row.get("meta").is_some(),
        "meta should exist in table_data"
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_execute_query_reports_affected_rows_for_update_delete() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_affected_rows_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name NVARCHAR(50))",
            qualified
        ))
        .await
        .expect("create affected_rows probe table failed");

    let inserted = driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, N'a'), (2, N'b')",
            qualified
        ))
        .await
        .expect("insert affected_rows probe rows failed");
    assert_eq!(inserted.row_count, 2);

    let updated = driver
        .execute_query(format!(
            "UPDATE {} SET name = N'bb' WHERE id = 2",
            qualified
        ))
        .await
        .expect("update affected_rows probe row failed");
    assert_eq!(updated.row_count, 1);

    let deleted = driver
        .execute_query(format!(
            "DELETE FROM {} WHERE id IN (1, 2)",
            qualified
        ))
        .await
        .expect("delete affected_rows probe rows failed");
    assert_eq!(deleted.row_count, 2);

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_transaction_commit_and_rollback() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_txn_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name NVARCHAR(50))",
            qualified
        ))
        .await
        .expect("create mssql txn probe table failed");

    driver
        .execute_query("BEGIN TRANSACTION".to_string())
        .await
        .expect("begin transaction failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, N'rolled_back')",
            qualified
        ))
        .await
        .expect("insert in rollback tx failed");
    driver
        .execute_query("ROLLBACK TRANSACTION".to_string())
        .await
        .expect("rollback failed");

    let rolled_back = driver
        .execute_query(format!(
            "SELECT COUNT(*) AS c FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("count after rollback failed");
    let rolled_back_count = rolled_back.data[0]["c"]
        .as_str()
        .expect("rollback count should be string")
        .parse::<i64>()
        .expect("rollback count should be numeric");
    assert_eq!(rolled_back_count, 0);

    driver
        .execute_query("BEGIN TRANSACTION".to_string())
        .await
        .expect("begin transaction failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (2, N'committed')",
            qualified
        ))
        .await
        .expect("insert in commit tx failed");
    driver
        .execute_query("COMMIT TRANSACTION".to_string())
        .await
        .expect("commit failed");

    let committed = driver
        .execute_query(format!(
            "SELECT COUNT(*) AS c FROM {} WHERE id = 2",
            qualified
        ))
        .await
        .expect("count after commit failed");
    let committed_count = committed.data[0]["c"]
        .as_str()
        .expect("commit count should be string")
        .parse::<i64>()
        .expect("commit count should be numeric");
    assert_eq!(committed_count, 1);

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_error_handling_for_sql_error() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let err = driver
        .execute_query("SELECT * FROM __dbpaw_table_not_exists".to_string())
        .await
        .expect_err("invalid SQL should return query error");
    assert!(
        err.contains("[QUERY_ERROR]") || err.contains("Invalid object name"),
        "unexpected error shape: {}",
        err
    );
}

#[tokio::test]
#[ignore]
async fn test_mssql_connection_failure_with_wrong_password() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, mut form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    form.password = Some("dbpaw_wrong_password".to_string());

    let err = match MssqlDriver::connect(&form).await {
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
async fn test_mssql_connection_timeout_or_unreachable_host_error() {
    let form = dbpaw_lib::models::ConnectionForm {
        driver: "mssql".to_string(),
        host: Some("203.0.113.1".to_string()),
        port: Some(1433),
        username: Some("sa".to_string()),
        password: Some("Password123".to_string()),
        database: Some("master".to_string()),
        ssl: Some(false),
        ..Default::default()
    };

    let err = match MssqlDriver::connect(&form).await {
        Ok(_) => panic!("unreachable host should fail"),
        Err(err) => err,
    };
    assert!(
        err.starts_with("[CONN_FAILED]"),
        "unexpected error: {}",
        err
    );
    assert!(
        err.to_ascii_lowercase().contains("timed out")
            || err.to_ascii_lowercase().contains("timeout")
            || err.to_ascii_lowercase().contains("network")
            || err.to_ascii_lowercase().contains("connection refused")
            || err.to_ascii_lowercase().contains("host")
            || err.to_ascii_lowercase().contains("unreachable"),
        "unexpected timeout/unreachable error: {}",
        err
    );
}

#[tokio::test]
#[ignore]
async fn test_mssql_batch_insert_and_batch_execute_flow() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_batch_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, category NVARCHAR(20), score INT)",
            qualified
        ))
        .await
        .expect("create batch probe table failed");

    let value_rows: Vec<String> = (1..=50)
        .map(|id| {
            let category = if id <= 25 { "alpha" } else { "beta" };
            format!("({}, N'{}, {})", id, category, id)
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
            "UPDATE {} SET category = N'gamma' WHERE id BETWEEN 30 AND 40",
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
            "SELECT COUNT(*) AS c FROM {} WHERE category = N'gamma'",
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
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_large_text_and_blob_round_trip() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_large_field_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, body NVARCHAR(MAX), payload VARBINARY(MAX))",
            qualified
        ))
        .await
        .expect("create large field probe table failed");

    let large_text = "x".repeat(70000);
    let blob_data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();

    driver
        .execute_query(format!(
            "INSERT INTO {} (id, body, payload) VALUES (1, N'{}', 0x{})",
            qualified,
            large_text,
            blob_data.iter().map(|b| format!("{:02x}", b)).collect::<String>()
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
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_concurrent_connections_can_query() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_concurrent_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT, value NVARCHAR(50))",
            qualified
        ))
        .await
        .expect("create concurrent probe table failed");
    driver
        .execute_query(format!("INSERT INTO {} VALUES (1, N'test')", qualified))
        .await
        .expect("insert concurrent probe row failed");
    driver.close().await;

    let mut handles = Vec::new();

    for _ in 0..8 {
        let task_form = form.clone();
        handles.push(tokio::spawn(async move {
            let task_driver =
                mssql_context::connect_with_retry(|| MssqlDriver::connect(&task_form)).await;
            let result = task_driver.execute_query("SELECT 1 AS ok".to_string()).await;
            task_driver.close().await;
            result
        }));
    }

    for handle in handles {
        let result = handle.await.expect("concurrent mssql task panicked");
        let data = result.expect("concurrent mssql query failed");
        assert_eq!(data.row_count, 1);
        let ok = &data.data[0]["ok"];
        let matches = ok == "1" || *ok == serde_json::Value::Number(1.into());
        assert!(matches, "ok should be 1, got {}", ok);
    }

    let cleanup_driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;
    let _ = cleanup_driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    cleanup_driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_view_can_be_listed_and_queried() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let base_table = "dbpaw_mssql_view_base_probe";
    let view_name = "dbpaw_mssql_view_probe_v";
    let qualified_table = format!("[dbo].[{}]", base_table);
    let qualified_view = format!("[dbo].[{}]", view_name);

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'V') IS NOT NULL DROP VIEW {};",
            view_name, qualified_view
        ))
        .await;
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            base_table, qualified_table
        ))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name NVARCHAR(50), score INT)",
            qualified_table
        ))
        .await
        .expect("create base table for view failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name, score) VALUES (1, N'alice', 10), (2, N'bob', 20)",
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
        .list_tables(Some("dbo".to_string()))
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
    assert_eq!(
        row["name"],
        serde_json::Value::String("bob".to_string())
    );

    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'V') IS NOT NULL DROP VIEW {};",
            view_name, qualified_view
        ))
        .await;
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            base_table, qualified_table
        ))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_mssql_prepared_statements_prepare_execute_and_deallocate() {
    let docker = (!mssql_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = mssql_context::mssql_form_from_test_context(docker.as_ref());
    let driver: MssqlDriver =
        mssql_context::connect_with_retry(|| MssqlDriver::connect(&form)).await;

    let table_name = "dbpaw_mssql_prepared_stmt_probe";
    let qualified = format!("[dbo].[{}]", table_name);
    let _ = driver
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name NVARCHAR(50))",
            qualified
        ))
        .await
        .expect("create prepared stmt probe table failed");

    let prepared_insert_sql = format!("INSERT INTO {} (id, name) VALUES (@P1, @P2)", qualified);
    let inserted_a = driver
        .execute_query(format!(
            "EXEC sp_executesql N'{}', N'@P1 INT, @P2 NVARCHAR(50)', @P1 = 1, @P2 = N'alice'",
            prepared_insert_sql.replace("'", "''")
        ))
        .await
        .expect("prepared insert alice failed");
    assert_eq!(inserted_a.row_count, 1);

    let inserted_b = driver
        .execute_query(format!(
            "EXEC sp_executesql N'{}', N'@P1 INT, @P2 NVARCHAR(50)', @P1 = 2, @P2 = N'bob'",
            prepared_insert_sql.replace("'", "''")
        ))
        .await
        .expect("prepared insert bob failed");
    assert_eq!(inserted_b.row_count, 1);

    let prepared_update_sql = format!("UPDATE {} SET name = @P1 WHERE id = @P2", qualified);
    let updated = driver
        .execute_query(format!(
            "EXEC sp_executesql N'{}', N'@P1 NVARCHAR(50), @P2 INT', @P1 = N'alice-updated', @P2 = 1",
            prepared_update_sql.replace("'", "''")
        ))
        .await
        .expect("prepared update failed");
    assert_eq!(updated.row_count, 1);

    let prepared_select_sql = format!("SELECT name FROM {} WHERE id = @P1", qualified);
    let selected = driver
        .execute_query(format!(
            "EXEC sp_executesql N'{}', N'@P1 INT', @P1 = 1",
            prepared_select_sql.replace("'", "''")
        ))
        .await
        .expect("prepared select failed");
    assert_eq!(selected.row_count, 1);
    assert_eq!(
        selected.data[0]["name"],
        serde_json::Value::String("alice-updated".to_string())
    );

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
        .execute_query(format!(
            "IF OBJECT_ID(N'dbo.{}', N'U') IS NOT NULL DROP TABLE {};",
            table_name, qualified
        ))
        .await;
    driver.close().await;
}
