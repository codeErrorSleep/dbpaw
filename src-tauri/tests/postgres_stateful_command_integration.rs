#[path = "common/postgres_context.rs"]
mod postgres_context;

use dbpaw_lib::ai::types::AiChatRequest;
use dbpaw_lib::commands::connection::{self, CreateDatabasePayload};
use dbpaw_lib::commands::metadata;
use dbpaw_lib::commands::{ai, query, storage, transfer};
use dbpaw_lib::db::drivers::postgres::PostgresDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::db::local::LocalDb;
use dbpaw_lib::models::{AiProviderForm, ConnectionForm};
use dbpaw_lib::state::AppState;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use testcontainers::clients::Cli;

fn unique_name(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_millis();
    format!("{}_{}", prefix, millis)
}

async fn wait_until_postgres_ready(form: &ConnectionForm) {
    let probe = form.clone();
    let driver = postgres_context::connect_with_retry(|| PostgresDriver::connect(&probe)).await;
    driver
        .test_connection()
        .await
        .expect("postgres should accept connections for stateful command tests");
    driver.close().await;
}

async fn init_state_with_local_db() -> AppState {
    let state = AppState::new();
    let local_db_dir = std::env::temp_dir().join(unique_name("dbpaw_localdb_stateful_it"));
    let db = LocalDb::init_with_app_dir(&local_db_dir)
        .await
        .expect("failed to initialize local db");
    let mut lock = state.local_db.lock().await;
    *lock = Some(Arc::new(db));
    drop(lock);
    state
}

async fn create_postgres_connection_for_state(
    state: &AppState,
    base_form: &ConnectionForm,
    suffix: &str,
) -> i64 {
    let mut form = base_form.clone();
    form.name = Some(format!("postgres-stateful-{suffix}"));
    let created = connection::create_connection_direct(state, form)
        .await
        .expect("create_connection should succeed");
    created.id
}

async fn drop_database_if_exists(form: &ConnectionForm, db_name: &str) {
    let driver = PostgresDriver::connect(form)
        .await
        .expect("failed to connect postgres driver for cleanup");
    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS \"{}\"", db_name))
        .await;
    driver.close().await;
}

async fn prepare_metadata_fixture(
    form: &ConnectionForm,
    schema: &str,
    parent_table: &str,
    child_table: &str,
) {
    let driver = PostgresDriver::connect(form)
        .await
        .expect("failed to connect postgres driver for metadata fixture");
    let parent_qualified = format!("\"{}\".\"{}\"", schema, parent_table);
    let child_qualified = format!("\"{}\".\"{}\"", schema, child_table);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", child_qualified))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", parent_qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, code TEXT)",
            parent_qualified
        ))
        .await
        .expect("create metadata parent table should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (\
                id INT PRIMARY KEY, \
                parent_id INT NOT NULL, \
                name TEXT, \
                CONSTRAINT fk_child_parent FOREIGN KEY (parent_id) REFERENCES {}(id)\
            )",
            child_qualified, parent_qualified
        ))
        .await
        .expect("create metadata child table should succeed");
    driver
        .execute_query(format!(
            "CREATE INDEX idx_child_name ON {} (name)",
            child_qualified
        ))
        .await
        .expect("create metadata child index should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, code) VALUES (1, 'p1')",
            parent_qualified
        ))
        .await
        .expect("insert parent row should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, parent_id, name) VALUES (10, 1, 'child-a')",
            child_qualified
        ))
        .await
        .expect("insert child row should succeed");
    driver.close().await;
}

async fn cleanup_metadata_fixture(
    form: &ConnectionForm,
    schema: &str,
    parent_table: &str,
    child_table: &str,
) {
    let driver = PostgresDriver::connect(form)
        .await
        .expect("failed to connect postgres driver for metadata cleanup");
    let parent_qualified = format!("\"{}\".\"{}\"", schema, parent_table);
    let child_qualified = format!("\"{}\".\"{}\"", schema, child_table);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", child_qualified))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", parent_qualified))
        .await;
    driver.close().await;
}

async fn get_local_db(state: &AppState) -> Arc<LocalDb> {
    let lock = state.local_db.lock().await;
    lock.as_ref()
        .cloned()
        .expect("local db should be initialized")
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_create_database_by_id_success() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "create-db-success").await;

    let db_name = unique_name("dbpaw_cmd_created_db");
    let payload = CreateDatabasePayload {
        name: db_name.clone(),
        if_not_exists: Some(true),
        charset: None,
        collation: None,
        encoding: None,
        lc_collate: None,
        lc_ctype: None,
    };

    connection::create_database_by_id_direct(&state, conn_id, payload)
        .await
        .expect("create_database_by_id should succeed");
    let dbs = connection::list_databases_by_id_direct(&state, conn_id)
        .await
        .expect("list_databases_by_id should succeed");
    assert!(dbs.iter().any(|d| d == &db_name));

    drop_database_if_exists(&form, &db_name).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_create_database_by_id_if_not_exists_idempotent() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "create-db-idempotent").await;

    let db_name = unique_name("dbpaw_cmd_idempotent_db");
    let payload = CreateDatabasePayload {
        name: db_name.clone(),
        if_not_exists: Some(true),
        charset: None,
        collation: None,
        encoding: None,
        lc_collate: None,
        lc_ctype: None,
    };

    connection::create_database_by_id_direct(&state, conn_id, payload.clone())
        .await
        .expect("first create_database_by_id should succeed");
    connection::create_database_by_id_direct(&state, conn_id, payload)
        .await
        .expect("second create_database_by_id should succeed");

    drop_database_if_exists(&form, &db_name).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_create_database_by_id_invalid_name_returns_validation_error() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "invalid-db-name").await;

    let payload = CreateDatabasePayload {
        name: "   ".to_string(),
        if_not_exists: Some(true),
        charset: None,
        collation: None,
        encoding: None,
        lc_collate: None,
        lc_ctype: None,
    };
    let result = connection::create_database_by_id_direct(&state, conn_id, payload).await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(err.contains("[VALIDATION_ERROR]"));

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_list_databases_by_id_success() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "list-db-success").await;

    let target_db = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let dbs = connection::list_databases_by_id_direct(&state, conn_id)
        .await
        .expect("list_databases_by_id should succeed");
    assert!(!dbs.is_empty());
    assert!(dbs.iter().any(|d| d == &target_db));

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_list_databases_by_id_invalid_id_returns_error() {
    let state = init_state_with_local_db().await;
    let result = connection::list_databases_by_id_direct(&state, -999_999).await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(!err.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_connection_crud_flow_create_get_update_delete() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;

    let unique = unique_name("dbpaw_cmd_conn");
    let mut create_form = form.clone();
    create_form.name = Some(format!("postgres-{unique}-created"));
    let created = connection::create_connection_direct(&state, create_form)
        .await
        .expect("create_connection should succeed");
    let conn_id = created.id;

    let listed = connection::get_connections_direct(&state)
        .await
        .expect("get_connections after create should succeed");
    assert!(listed.iter().any(|c| c.id == conn_id));

    let mut update_form = form.clone();
    update_form.name = Some(format!("postgres-{unique}-updated"));
    update_form.database = form.database.clone().or(Some("postgres".to_string()));
    let updated = connection::update_connection_direct(&state, conn_id, update_form)
        .await
        .expect("update_connection should succeed");
    assert_eq!(updated.id, conn_id);
    assert_eq!(updated.name, format!("postgres-{unique}-updated"));

    connection::delete_connection_direct(&state, conn_id)
        .await
        .expect("delete_connection should succeed");
    let listed_after_delete = connection::get_connections_direct(&state)
        .await
        .expect("get_connections after delete should succeed");
    assert!(!listed_after_delete.iter().any(|c| c.id == conn_id));
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_get_table_structure_success() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id =
        create_postgres_connection_for_state(&state, &form, "meta-structure-success").await;
    let schema = "public".to_string();
    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form, &schema, &parent, &child).await;

    let structure =
        metadata::get_table_structure_direct(&state, conn_id, schema.clone(), child.clone())
            .await
            .expect("get_table_structure should succeed");
    assert!(structure.columns.iter().any(|c| c.name == "id"));
    assert!(structure.columns.iter().any(|c| c.name == "parent_id"));

    cleanup_metadata_fixture(&form, &schema, &parent, &child).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_get_table_structure_missing_table_returns_error() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id =
        create_postgres_connection_for_state(&state, &form, "meta-structure-missing").await;
    let schema = "public".to_string();
    let missing_table = unique_name("dbpaw_meta_missing");

    let result = metadata::get_table_structure_direct(&state, conn_id, schema, missing_table).await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(!err.trim().is_empty());

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_get_table_ddl_success() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "meta-ddl-success").await;
    let schema = "public".to_string();
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form, &schema, &parent, &child).await;

    let ddl = metadata::get_table_ddl_direct(
        &state,
        conn_id,
        Some(database),
        schema.clone(),
        child.clone(),
    )
    .await
    .expect("get_table_ddl should succeed");
    assert!(ddl.to_uppercase().contains("CREATE TABLE"));
    assert!(ddl.contains(&child));

    cleanup_metadata_fixture(&form, &schema, &parent, &child).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_get_table_metadata_contains_indexes_and_foreign_keys() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id =
        create_postgres_connection_for_state(&state, &form, "meta-metadata-success").await;
    let schema = "public".to_string();
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form, &schema, &parent, &child).await;

    let meta = metadata::get_table_metadata_direct(
        &state,
        conn_id,
        Some(database),
        schema.clone(),
        child.clone(),
    )
    .await
    .expect("get_table_metadata should succeed");
    assert!(meta.indexes.iter().any(|idx| idx.name == "idx_child_name"));
    assert!(meta.foreign_keys.iter().any(|fk| fk.column == "parent_id"));

    cleanup_metadata_fixture(&form, &schema, &parent, &child).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_get_schema_overview_contains_target_schema() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "meta-schema-overview").await;
    let schema = "public".to_string();
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form, &schema, &parent, &child).await;

    let overview =
        metadata::get_schema_overview_direct(&state, conn_id, Some(database), Some(schema.clone()))
            .await
            .expect("get_schema_overview should succeed");
    assert!(overview
        .tables
        .iter()
        .any(|t| t.schema == schema && t.name == child));

    cleanup_metadata_fixture(&form, &schema, &parent, &child).await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_execute_query_by_id_success() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "query-by-id-success").await;
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());

    let result = query::execute_query_by_id_direct(
        &state,
        conn_id,
        "SELECT 1 AS v".to_string(),
        Some(database),
        Some("phase4_success".to_string()),
        Some("phase4-qid-success".to_string()),
    )
    .await
    .expect("execute_query_by_id should succeed");
    assert!(result.success);
    assert!(result.row_count >= 1);
    assert!(!result.data.is_empty());

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_execute_query_by_id_invalid_sql_returns_error() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "query-by-id-invalid").await;
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());

    let result = query::execute_query_by_id_direct(
        &state,
        conn_id,
        "SELECT * FROM __dbpaw_missing_phase4_table".to_string(),
        Some(database),
        Some("phase4_invalid".to_string()),
        Some("phase4-qid-invalid".to_string()),
    )
    .await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(!err.trim().is_empty());

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_list_sql_execution_logs_contains_recent_entries() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "query-log-list").await;
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());

    query::execute_query_by_id_direct(
        &state,
        conn_id,
        "SELECT 1 AS phase4_log_probe".to_string(),
        Some(database),
        Some("phase4_log_probe".to_string()),
        Some("phase4-qid-log".to_string()),
    )
    .await
    .expect("execute_query_by_id for log probe should succeed");

    let logs = query::list_sql_execution_logs_direct(&state, Some(20))
        .await
        .expect("list_sql_execution_logs should succeed");
    assert!(!logs.is_empty());
    assert!(logs.iter().any(|l| {
        l.source.as_deref() == Some("phase4_log_probe")
            && l.sql.contains("phase4_log_probe")
            && l.success
    }));

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_cancel_query_non_clickhouse_returns_false() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "query-cancel-non-ch").await;

    let canceled =
        query::cancel_query_direct(&state, conn_id.to_string(), "phase4-qid-cancel".to_string())
            .await
            .expect("cancel_query should return bool");
    assert!(!canceled);

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_storage_saved_query_crud_flow() {
    let state = init_state_with_local_db().await;
    let name = unique_name("saved_query");
    let created = storage::save_query_direct(
        &state,
        name.clone(),
        "SELECT 1".to_string(),
        Some("desc".to_string()),
        None,
        Some("postgres".to_string()),
    )
    .await
    .expect("save_query should succeed");
    assert_eq!(created.name, name);

    let all = storage::get_saved_queries_direct(&state)
        .await
        .expect("get_saved_queries should succeed");
    assert!(all.iter().any(|q| q.id == created.id));

    let updated = storage::update_saved_query_direct(
        &state,
        created.id,
        format!("{}_updated", created.name),
        "SELECT 2".to_string(),
        Some("desc2".to_string()),
        None,
        Some("postgres".to_string()),
    )
    .await
    .expect("update_saved_query should succeed");
    assert_eq!(updated.query, "SELECT 2");

    storage::delete_saved_query_direct(&state, created.id)
        .await
        .expect("delete_saved_query should succeed");
    let all_after = storage::get_saved_queries_direct(&state)
        .await
        .expect("get_saved_queries after delete should succeed");
    assert!(!all_after.iter().any(|q| q.id == created.id));
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_transfer_export_and_import_minimal_flow() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "transfer-minimal").await;
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let schema = "public".to_string();
    let table = unique_name("dbpaw_transfer_src");
    let qualified = format!("\"{}\".\"{}\"", schema, table);
    let driver = PostgresDriver::connect(&form)
        .await
        .expect("failed to connect postgres driver");
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, name TEXT)",
            qualified
        ))
        .await
        .expect("create transfer src table should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'a'), (2, 'b')",
            qualified
        ))
        .await
        .expect("insert transfer src rows should succeed");
    driver.close().await;

    let base = std::env::temp_dir().join(unique_name("dbpaw_transfer_it"));
    fs::create_dir_all(&base).expect("create temp transfer dir should succeed");
    let table_export_path = base.join("table_export.csv");
    let query_export_path = base.join("query_export.json");
    let import_sql_path = base.join("import.sql");

    let table_export = transfer::export_table_data_direct(
        &state,
        conn_id,
        Some(database.clone()),
        schema.clone(),
        table.clone(),
        "postgres".to_string(),
        transfer::ExportFormat::Csv,
        transfer::ExportScope::FullTable,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(table_export_path.to_string_lossy().to_string()),
        Some(100),
    )
    .await
    .expect("export_table_data should succeed");
    assert!(table_export.row_count >= 2);
    assert!(std::path::Path::new(&table_export.file_path).exists());

    let query_export = transfer::export_query_result_direct(
        &state,
        conn_id,
        Some(database.clone()),
        format!("SELECT * FROM {} ORDER BY id", qualified),
        "postgres".to_string(),
        transfer::ExportFormat::Json,
        Some(query_export_path.to_string_lossy().to_string()),
    )
    .await
    .expect("export_query_result should succeed");
    assert!(query_export.row_count >= 2);
    assert!(std::path::Path::new(&query_export.file_path).exists());

    let import_table = unique_name("dbpaw_import_dst");
    let import_sql = format!(
        "CREATE TABLE \"{}\".\"{}\" (id INT PRIMARY KEY, name TEXT);",
        schema, import_table
    );
    fs::write(&import_sql_path, import_sql).expect("write import sql file should succeed");
    let import_result = transfer::import_sql_file_direct(
        &state,
        conn_id,
        Some(database.clone()),
        import_sql_path.to_string_lossy().to_string(),
        "postgres".to_string(),
    )
    .await
    .expect("import_sql_file should succeed");
    assert!(import_result.success_statements >= 1);
    assert!(import_result.error.is_none());

    let cleanup_driver = PostgresDriver::connect(&form)
        .await
        .expect("failed to connect postgres driver for transfer cleanup");
    let _ = cleanup_driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    let _ = cleanup_driver
        .execute_query(format!(
            "DROP TABLE IF EXISTS \"{}\".\"{}\"",
            schema, import_table
        ))
        .await;
    cleanup_driver.close().await;
    let _ = fs::remove_file(table_export_path);
    let _ = fs::remove_file(query_export_path);
    let _ = fs::remove_file(import_sql_path);
    let _ = fs::remove_dir_all(base);
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_import_sql_file_supports_function_trigger_script() {
    let docker = (!postgres_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = postgres_context::postgres_form_from_test_context(docker.as_ref());
    wait_until_postgres_ready(&form).await;
    let state = init_state_with_local_db().await;
    let conn_id = create_postgres_connection_for_state(&state, &form, "import-trigger").await;
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let schema = "public".to_string();
    let table = unique_name("dbpaw_import_pg_tbl");
    let func_name = unique_name("dbpaw_import_pg_fn");
    let trigger_name = unique_name("dbpaw_import_pg_trg");
    let qualified = format!("\"{}\".\"{}\"", schema, table);
    let base = std::env::temp_dir().join(unique_name("dbpaw_postgres_import_it"));
    fs::create_dir_all(&base).expect("create temp transfer dir should succeed");
    let import_sql_path = base.join("import.sql");

    let import_sql = format!(
        r#"
CREATE TABLE {qualified} (
    id INT PRIMARY KEY,
    name TEXT,
    touch_count INT DEFAULT 0
);
CREATE OR REPLACE FUNCTION "{schema}"."{func_name}"()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.touch_count = COALESCE(NEW.touch_count, 0) + 1;
    RETURN NEW;
END;
$$;
CREATE TRIGGER "{trigger_name}"
BEFORE INSERT OR UPDATE ON {qualified}
FOR EACH ROW
EXECUTE FUNCTION "{schema}"."{func_name}"();
"#
    );
    fs::write(&import_sql_path, import_sql).expect("write import sql file should succeed");

    let import_result = transfer::import_sql_file_direct(
        &state,
        conn_id,
        Some(database.clone()),
        import_sql_path.to_string_lossy().to_string(),
        "postgres".to_string(),
    )
    .await
    .expect("import_sql_file should succeed");
    assert_eq!(
        import_result.success_statements,
        import_result.total_statements
    );
    assert!(import_result.error.is_none());

    let driver = PostgresDriver::connect(&form)
        .await
        .expect("failed to connect postgres driver for verification");
    driver
        .execute_query(format!(
            "INSERT INTO {} (id, name) VALUES (1, 'alpha')",
            qualified
        ))
        .await
        .expect("insert into imported postgres table should succeed");
    driver
        .execute_query(format!(
            "UPDATE {} SET name = 'beta' WHERE id = 1",
            qualified
        ))
        .await
        .expect("update imported postgres table should succeed");
    let verify = driver
        .execute_query(format!(
            "SELECT touch_count AS c FROM {} WHERE id = 1",
            qualified
        ))
        .await
        .expect("verify postgres trigger should succeed");
    let count = verify.data[0]["c"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("touch_count should parse");
    assert_eq!(count, 2);

    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", qualified))
        .await;
    let _ = driver
        .execute_query(format!(
            "DROP FUNCTION IF EXISTS \"{}\".\"{}\"()",
            schema, func_name
        ))
        .await;
    driver.close().await;

    let _ = fs::remove_file(import_sql_path);
    let _ = fs::remove_dir_all(base);
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_postgres_command_ai_minimal_provider_conversation_and_chat_flow() {
    let state = init_state_with_local_db().await;

    let start_without_provider = ai::ai_chat_start_direct(
        &state,
        AiChatRequest {
            request_id: unique_name("ai_start_no_provider"),
            provider_id: None,
            conversation_id: None,
            scenario: "sql_generate".to_string(),
            input: "select 1".to_string(),
            title: Some("phase5 start".to_string()),
            connection_id: None,
            database: None,
            schema_overview: None,
            selected_tables: None,
        },
    )
    .await;
    assert!(start_without_provider.is_err());

    let created_provider = ai::ai_create_provider_direct(
        &state,
        AiProviderForm {
            name: unique_name("ai_provider"),
            provider_type: Some("openai".to_string()),
            base_url: "https://example.invalid/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: Some("sk-test".to_string()),
            is_default: Some(true),
            enabled: Some(true),
            extra_json: None,
        },
    )
    .await
    .expect("ai_create_provider should succeed");
    let providers = ai::ai_list_providers_direct(&state)
        .await
        .expect("ai_list_providers should succeed");
    assert!(providers.iter().any(|p| p.id == created_provider.id));

    let updated_provider = ai::ai_update_provider_direct(
        &state,
        created_provider.id,
        AiProviderForm {
            name: format!("{}_updated", created_provider.name),
            provider_type: Some("openai".to_string()),
            base_url: "https://example.invalid/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: Some("sk-test-2".to_string()),
            is_default: Some(true),
            enabled: Some(true),
            extra_json: None,
        },
    )
    .await
    .expect("ai_update_provider should succeed");
    assert_eq!(updated_provider.id, created_provider.id);

    ai::ai_set_default_provider_direct(&state, created_provider.id)
        .await
        .expect("ai_set_default_provider should succeed");
    ai::ai_clear_provider_api_key_direct(&state, "openai".to_string())
        .await
        .expect("ai_clear_provider_api_key should succeed");

    let continue_without_conversation = ai::ai_chat_continue_direct(
        &state,
        AiChatRequest {
            request_id: unique_name("ai_continue_no_conv"),
            provider_id: Some(created_provider.id),
            conversation_id: None,
            scenario: "sql_generate".to_string(),
            input: "continue".to_string(),
            title: None,
            connection_id: None,
            database: None,
            schema_overview: None,
            selected_tables: None,
        },
    )
    .await;
    assert!(continue_without_conversation.is_err());

    let db = get_local_db(&state).await;
    let conv = db
        .create_ai_conversation(
            unique_name("ai_conv"),
            "sql_generate".to_string(),
            None,
            None,
        )
        .await
        .expect("create ai conversation in local db should succeed");
    let conversations = ai::ai_list_conversations_direct(&state, None, None)
        .await
        .expect("ai_list_conversations should succeed");
    assert!(conversations.iter().any(|c| c.id == conv.id));
    let detail = ai::ai_get_conversation_direct(&state, conv.id)
        .await
        .expect("ai_get_conversation should succeed");
    assert_eq!(detail.conversation.id, conv.id);
    ai::ai_delete_conversation_direct(&state, conv.id)
        .await
        .expect("ai_delete_conversation should succeed");
    let conversations_after = ai::ai_list_conversations_direct(&state, None, None)
        .await
        .expect("ai_list_conversations after delete should succeed");
    assert!(!conversations_after.iter().any(|c| c.id == conv.id));

    ai::ai_delete_provider_direct(&state, created_provider.id)
        .await
        .expect("ai_delete_provider should succeed");
}
