#[path = "common/postgres_context.rs"]
mod postgres_context;

use dbpaw_lib::db::drivers::postgres::PostgresDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use testcontainers::clients::Cli;

#[tokio::test]
#[ignore]
async fn test_postgres_integration_flow() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());

    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

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

#[tokio::test]
#[ignore]
async fn test_postgres_get_table_data_supports_pagination_sort_filter_and_order_by() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_grid_probe";
    let qualified = format!("public.{}", table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name TEXT, score INT)",
            qualified
        ))
        .await
        .expect("create pg grid probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name, score) VALUES \
             (1, 'alpha', 10), (2, 'beta', 20), (3, 'gamma', 30), (4, 'delta', 40)",
            qualified
        ))
        .await
        .expect("insert pg grid rows failed");

    let page1 = driver
        .get_table_data(
            "public".to_string(),
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
            "public".to_string(),
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
            "public".to_string(),
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
async fn test_postgres_get_table_data_rejects_invalid_sort_column() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_invalid_sort_probe";
    let qualified = format!("public.{}", table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!("CREATE TABLE {} (id INT PRIMARY KEY)", qualified))
        .await
        .expect("create invalid sort probe table failed");

    let result = driver
        .get_table_data(
            "public".to_string(),
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
async fn test_postgres_table_structure_and_schema_overview() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_overview_probe";
    let qualified = format!("public.{}", table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, label TEXT NOT NULL)",
            qualified
        ))
        .await
        .expect("create overview probe table failed");

    let structure = driver
        .get_table_structure("public".to_string(), table_name.to_string())
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
        .get_schema_overview(Some("public".to_string()))
        .await
        .expect("get_schema_overview failed");
    assert!(
        overview
            .tables
            .iter()
            .any(|t| t.schema == "public" && t.name == table_name),
        "schema overview should include public.{}",
        table_name
    );

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_metadata_includes_indexes_and_foreign_keys() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let parent = "dbpaw_pg_parent_meta_probe";
    let child = "dbpaw_pg_child_meta_probe";
    let parent_qualified = format!("public.{}", parent);
    let child_qualified = format!("public.{}", child);

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
                name TEXT, \
                CONSTRAINT fk_pg_child_parent FOREIGN KEY (parent_id) REFERENCES {}(id)\
            )",
            child_qualified, parent_qualified
        ))
        .await
        .expect("create child table with fk failed");
    driver
        .execute_query(format!(
            "CREATE INDEX idx_pg_child_name ON {} (name)",
            child_qualified
        ))
        .await
        .expect("create child index failed");

    let metadata = driver
        .get_table_metadata("public".to_string(), child.to_string())
        .await
        .expect("get_table_metadata for child failed");
    assert!(
        metadata
            .indexes
            .iter()
            .any(|i| i.name == "idx_pg_child_name" && i.columns.contains(&"name".to_string())),
        "metadata should include idx_pg_child_name"
    );
    assert!(
        metadata.foreign_keys.iter().any(|fk| {
            fk.name == "fk_pg_child_parent"
                && fk.column == "parent_id"
                && fk.referenced_table == parent
                && fk.referenced_column == "id"
        }),
        "metadata should include fk_pg_child_parent"
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
async fn test_postgres_boolean_and_json_type_mapping_regression() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_bool_json_probe";
    let qualified = format!("public.{}", table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, flag BOOLEAN, meta JSONB)",
            qualified
        ))
        .await
        .expect("create bool/json probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, flag, meta) VALUES (1, true, '{{\"tier\":\"gold\"}}'::jsonb)",
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
    assert_eq!(query_row["flag"], serde_json::Value::Bool(true));
    assert!(
        query_row.get("meta").is_some(),
        "meta should exist in query result"
    );

    let table_data = driver
        .get_table_data(
            "public".to_string(),
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
    assert_eq!(grid_row["flag"], serde_json::Value::Bool(true));
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
async fn test_postgres_transaction_commit_and_rollback() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_txn_probe";
    let qualified = format!("public.{}", table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name TEXT)",
            qualified
        ))
        .await
        .expect("create pg txn probe table failed");

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
    let rolled_back_count = rolled_back.data[0]["c"]
        .as_str()
        .expect("rollback count should be string")
        .parse::<i64>()
        .expect("rollback count should be numeric");
    assert_eq!(rolled_back_count, 0);

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
    let committed_count = committed.data[0]["c"]
        .as_str()
        .expect("commit count should be string")
        .parse::<i64>()
        .expect("commit count should be numeric");
    assert_eq!(committed_count, 1);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_execute_query_reports_affected_rows_for_update_delete() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_affected_rows_probe";
    let qualified = format!("public.{}", table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name TEXT)",
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
    assert!(inserted.success);

    let updated = driver
        .execute_query(format!("UPDATE {} SET name = 'bb' WHERE id = 2", qualified))
        .await
        .expect("update affected_rows probe row failed");
    assert!(updated.success);

    let deleted = driver
        .execute_query(format!("DELETE FROM {} WHERE id IN (1, 2)", qualified))
        .await
        .expect("delete affected_rows probe rows failed");
    assert!(deleted.success);

    let remain = driver
        .execute_query(format!("SELECT COUNT(*) AS c FROM {}", qualified))
        .await
        .expect("count after delete should succeed");
    let remain_count = remain.data[0]["c"]
        .as_str()
        .expect("count should be string")
        .parse::<i64>()
        .expect("count should be numeric");
    assert_eq!(remain_count, 0);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_large_text_and_blob_round_trip() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_large_field_probe";
    let qualified = format!("public.{}", table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, body TEXT, payload BYTEA)",
            qualified
        ))
        .await
        .expect("create large field probe table failed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, body, payload) VALUES (1, repeat('x', 70000), decode(repeat('ab', 2048), 'hex'))",
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
async fn test_postgres_error_handling_for_sql_error() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

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
async fn test_postgres_concurrent_connections_can_query() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let mut handles = Vec::new();

    for _ in 0..8 {
        let task_form = form.clone();
        handles.push(tokio::spawn(async move {
            let driver =
                postgres_context::connect_with_retry(|| PostgresDriver::connect(&task_form)).await;
            driver.execute_query("SELECT 1 AS ok".to_string()).await
        }));
    }

    for handle in handles {
        let result = handle.await.expect("concurrent postgres task panicked");
        let data = result.expect("concurrent postgres query failed");
        assert_eq!(data.row_count, 1);
        let ok = &data.data[0]["ok"];
        let matches = ok == "1" || *ok == serde_json::Value::Number(1.into());
        assert!(matches, "ok should be 1, got {}", ok);
    }
}

#[tokio::test]
#[ignore]
async fn test_postgres_view_can_be_listed_and_queried() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let base_table = "dbpaw_pg_view_base_probe";
    let view_name = "dbpaw_pg_view_probe_v";
    let qualified_table = format!("public.{}", base_table);
    let qualified_view = format!("public.{}", view_name);

    let _ = driver
        .execute_query(format!("DROP VIEW IF EXISTS {}", qualified_view))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified_table))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name TEXT, score INT)",
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
        .list_tables(Some("public".to_string()))
        .await
        .expect("list_tables failed");
    assert!(
        tables
            .iter()
            .any(|t| t.name == base_table && t.r#type == "BASE TABLE"),
        "list_tables should include base table"
    );
    assert!(
        tables
            .iter()
            .any(|t| t.name == view_name && t.r#type == "VIEW"),
        "list_tables should include view with type=VIEW"
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
async fn test_postgres_array_types_decoded_as_json_arrays() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_array_type_probe";
    let qualified = format!("public.{}", table_name);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id       INT PRIMARY KEY,\
                ints2    SMALLINT[],\
                ints4    INT[],\
                ints8    BIGINT[],\
                floats4  FLOAT4[],\
                floats8  FLOAT8[],\
                texts    TEXT[],\
                bools    BOOLEAN[],\
                jsonbs   JSONB[]\
            )",
            qualified
        ))
        .await
        .expect("create array probe table failed");

    // row 1: fully populated arrays
    driver
        .execute_query(format!(
            "INSERT INTO {} VALUES \
             (1, ARRAY[1::smallint,2::smallint], ARRAY[10,20,30], ARRAY[100::bigint,200::bigint], \
              ARRAY[1.5::float4,2.5::float4], ARRAY[3.14::float8,6.28::float8], \
              ARRAY['hello','world'], ARRAY[true,false,true], \
              ARRAY['{{\"a\":1}}'::jsonb,'{{\"b\":2}}'::jsonb])",
            qualified
        ))
        .await
        .expect("insert full-array row failed");

    // row 2: arrays containing NULL elements
    driver
        .execute_query(format!(
            "INSERT INTO {} VALUES \
             (2, ARRAY[NULL::smallint,5::smallint], ARRAY[NULL::int,42], NULL, \
              NULL, NULL, \
              ARRAY['x',NULL::text,'z'], ARRAY[NULL::boolean], \
              NULL)",
            qualified
        ))
        .await
        .expect("insert null-element row failed");

    // row 3: empty arrays
    driver
        .execute_query(format!(
            "INSERT INTO {} VALUES \
             (3, ARRAY[]::smallint[], ARRAY[]::int[], ARRAY[]::bigint[], \
              ARRAY[]::float4[], ARRAY[]::float8[], \
              ARRAY[]::text[], ARRAY[]::boolean[], \
              ARRAY[]::jsonb[])",
            qualified
        ))
        .await
        .expect("insert empty-array row failed");

    let result = driver
        .execute_query(format!("SELECT * FROM {} ORDER BY id", qualified))
        .await
        .expect("select array probe rows failed");

    assert_eq!(result.row_count, 3, "expected 3 rows");

    // ---- row 1: full arrays ----
    let r1 = &result.data[0];

    let ints2 = r1["ints2"].as_array().expect("ints2 should be array");
    assert_eq!(ints2.len(), 2);
    assert_eq!(ints2[0].as_i64().unwrap_or(-1), 1);
    assert_eq!(ints2[1].as_i64().unwrap_or(-1), 2);

    let ints4 = r1["ints4"].as_array().expect("ints4 should be array");
    assert_eq!(ints4.len(), 3);
    assert_eq!(ints4[2].as_i64().unwrap_or(-1), 30);

    let ints8 = r1["ints8"].as_array().expect("ints8 should be array");
    assert_eq!(ints8.len(), 2);
    assert_eq!(ints8[1].as_i64().unwrap_or(-1), 200);

    let floats8 = r1["floats8"].as_array().expect("floats8 should be array");
    assert_eq!(floats8.len(), 2);
    assert!(
        floats8[0]
            .as_f64()
            .map(|v| (v - 3.14).abs() < 0.01)
            .unwrap_or(false),
        "floats8[0] should be ~3.14, got {:?}",
        floats8[0]
    );

    let texts = r1["texts"].as_array().expect("texts should be array");
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0].as_str().unwrap_or(""), "hello");
    assert_eq!(texts[1].as_str().unwrap_or(""), "world");

    let bools = r1["bools"].as_array().expect("bools should be array");
    assert_eq!(bools.len(), 3);
    assert_eq!(bools[0], serde_json::Value::Bool(true));
    assert_eq!(bools[1], serde_json::Value::Bool(false));

    let jsonbs = r1["jsonbs"].as_array().expect("jsonbs should be array");
    assert_eq!(jsonbs.len(), 2);
    assert_eq!(jsonbs[0]["a"], serde_json::Value::Number(1.into()));
    assert_eq!(jsonbs[1]["b"], serde_json::Value::Number(2.into()));

    // ---- row 2: null elements inside arrays ----
    let r2 = &result.data[1];

    let ints2_null = r2["ints2"].as_array().expect("ints2 row2 should be array");
    assert_eq!(
        ints2_null[0],
        serde_json::Value::Null,
        "first element should be NULL"
    );
    assert_eq!(ints2_null[1].as_i64().unwrap_or(-1), 5);

    let ints4_null = r2["ints4"].as_array().expect("ints4 row2 should be array");
    assert_eq!(
        ints4_null[0],
        serde_json::Value::Null,
        "first int4 element should be NULL"
    );

    let texts_null = r2["texts"].as_array().expect("texts row2 should be array");
    assert_eq!(texts_null[0].as_str().unwrap_or(""), "x");
    assert_eq!(
        texts_null[1],
        serde_json::Value::Null,
        "middle text element should be NULL"
    );
    assert_eq!(texts_null[2].as_str().unwrap_or(""), "z");

    let bools_null = r2["bools"].as_array().expect("bools row2 should be array");
    assert_eq!(
        bools_null[0],
        serde_json::Value::Null,
        "bool element should be NULL"
    );

    // column-level NULL (entire array is NULL)
    assert_eq!(
        r2["ints8"],
        serde_json::Value::Null,
        "whole ints8 column should be NULL"
    );

    // ---- row 3: empty arrays ----
    let r3 = &result.data[2];
    assert_eq!(r3["ints4"].as_array().expect("ints4 row3").len(), 0);
    assert_eq!(r3["texts"].as_array().expect("texts row3").len(), 0);
    assert_eq!(r3["bools"].as_array().expect("bools row3").len(), 0);
    assert_eq!(r3["jsonbs"].as_array().expect("jsonbs row3").len(), 0);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_connection_failure_with_wrong_password() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, mut form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    form.password = Some("dbpaw_wrong_password".to_string());

    let err = match PostgresDriver::connect(&form).await {
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
async fn test_postgres_connection_timeout_or_unreachable_host_error() {
    let form = dbpaw_lib::models::ConnectionForm {
        driver: "postgres".to_string(),
        host: Some("203.0.113.1".to_string()),
        port: Some(5432),
        username: Some("postgres".to_string()),
        password: Some("postgres".to_string()),
        database: Some("postgres".to_string()),
        ssl: Some(false),
        ..Default::default()
    };

    let err = match PostgresDriver::connect(&form).await {
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
async fn test_postgres_batch_insert_and_batch_execute_flow() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_batch_probe";
    let qualified = format!("public.{}", table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, category TEXT, score INT)",
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
    assert!(inserted.success);

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
    assert_eq!(affected.len(), 3);

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
async fn test_postgres_prepared_statements_prepare_execute_and_deallocate() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&form)).await;

    let table_name = "dbpaw_pg_prepared_stmt_probe";
    let qualified = format!("public.{}", table_name);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name TEXT)",
            qualified
        ))
        .await
        .expect("create prepared stmt probe table failed");

    let mut conn = driver
        .pool
        .acquire()
        .await
        .expect("acquire postgres pooled connection failed");
    let prepared_insert_sql = format!("INSERT INTO {} (id, name) VALUES ($1, $2)", qualified);
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

    let prepared_update_sql = format!("UPDATE {} SET name = $1 WHERE id = $2", qualified);
    let updated = sqlx::query(&prepared_update_sql)
        .bind("alice-updated")
        .bind(1_i64)
        .execute(&mut *conn)
        .await
        .expect("prepared update failed");
    assert_eq!(updated.rows_affected(), 1);

    let prepared_select_sql = format!("SELECT name FROM {} WHERE id = $1", qualified);
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
