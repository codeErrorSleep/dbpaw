#[path = "common/starrocks_context.rs"]
mod starrocks_context;

use dbpaw_lib::commands::connection::{self, CreateDatabasePayload};
use dbpaw_lib::commands::{metadata, query, storage};
use dbpaw_lib::db::drivers::mysql::MysqlDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::db::local::LocalDb;
use dbpaw_lib::models::ConnectionForm;
use dbpaw_lib::state::AppState;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use testcontainers::clients::Cli;
use tokio::time::{sleep, Duration};

fn unique_name(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_millis();
    format!("{}_{}", prefix, millis)
}

async fn wait_until_starrocks_ready(form: &ConnectionForm) {
    let mut last_error = String::new();
    for _ in 0..90 {
        match connection::test_connection_ephemeral(form.clone()).await {
            Ok(_) => return,
            Err(err) => {
                last_error = err;
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("starrocks is not ready for stateful command tests: {last_error}");
}

async fn init_state_with_local_db() -> AppState {
    let state = AppState::new();
    let local_db_dir = std::env::temp_dir().join(unique_name("dbpaw_starrocks_stateful_it"));
    let db = LocalDb::init_with_app_dir(&local_db_dir)
        .await
        .expect("failed to initialize local db");
    let mut lock = state.local_db.lock().await;
    *lock = Some(Arc::new(db));
    drop(lock);
    state
}

async fn create_starrocks_connection_for_state(
    state: &AppState,
    base_form: &ConnectionForm,
    suffix: &str,
) -> i64 {
    let mut form = base_form.clone();
    form.name = Some(format!("starrocks-stateful-{suffix}"));
    let created = connection::create_connection_direct(state, form)
        .await
        .expect("create_connection should succeed");
    created.id
}

async fn drop_database_if_exists(form: &ConnectionForm, db_name: &str) {
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect starrocks driver for cleanup");
    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    driver.close().await;
}

async fn prepare_metadata_fixture(
    form: &ConnectionForm,
    schema: &str,
    parent_table: &str,
    child_table: &str,
) {
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect starrocks driver for metadata fixture");
    let parent_qualified = format!("`{}`.`{}`", schema, parent_table);
    let child_qualified = format!("`{}`.`{}`", schema, child_table);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", child_qualified))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", parent_qualified))
        .await;

    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, code VARCHAR(30))",
            parent_qualified
        ))
        .await
        .expect("create metadata parent table should succeed");
    driver
        .execute_query(format!(
            "CREATE TABLE {} (id INT PRIMARY KEY, parent_id INT, name VARCHAR(64))",
            child_qualified
        ))
        .await
        .expect("create metadata child table should succeed");
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
    let driver = MysqlDriver::connect(form)
        .await
        .expect("failed to connect starrocks driver for metadata cleanup");
    let parent_qualified = format!("`{}`.`{}`", schema, parent_table);
    let child_qualified = format!("`{}`.`{}`", schema, child_table);
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", child_qualified))
        .await;
    let _ = driver
        .execute_query(format!("DROP TABLE IF EXISTS {}", parent_qualified))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_create_database_by_id_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id = create_starrocks_connection_for_state(&state, &form, "create-db-success").await;

    let db_name = unique_name("dbpaw_starrocks_cmd_created_db");
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
async fn test_starrocks_command_create_database_by_id_if_not_exists_idempotent() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id =
        create_starrocks_connection_for_state(&state, &form, "create-db-idempotent").await;

    let db_name = unique_name("dbpaw_starrocks_cmd_idempotent_db");
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
async fn test_starrocks_command_create_database_by_id_invalid_name_returns_validation_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id = create_starrocks_connection_for_state(&state, &form, "invalid-db-name").await;

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
async fn test_starrocks_command_list_databases_by_id_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id = create_starrocks_connection_for_state(&state, &form, "list-db-success").await;

    let dbs = connection::list_databases_by_id_direct(&state, conn_id)
        .await
        .expect("list_databases_by_id should succeed");
    assert!(!dbs.is_empty());
    assert!(dbs.iter().all(|db| !db.trim().is_empty()));

    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_list_databases_by_id_invalid_id_returns_error() {
    let state = init_state_with_local_db().await;
    let result = connection::list_databases_by_id_direct(&state, -999_999).await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(!err.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_connection_crud_flow_create_get_update_delete() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;

    let unique = unique_name("dbpaw_starrocks_conn");
    let mut create_form = form.clone();
    create_form.name = Some(format!("starrocks-{unique}-created"));
    let created = connection::create_connection_direct(&state, create_form)
        .await
        .expect("create_connection should succeed");
    let conn_id = created.id;

    let listed = connection::get_connections_direct(&state)
        .await
        .expect("get_connections after create should succeed");
    assert!(listed.iter().any(|c| c.id == conn_id));

    let mut update_form = form.clone();
    update_form.name = Some(format!("starrocks-{unique}-updated"));
    let updated = connection::update_connection_direct(&state, conn_id, update_form)
        .await
        .expect("update_connection should succeed");
    assert_eq!(updated.id, conn_id);
    assert_eq!(updated.name, format!("starrocks-{unique}-updated"));

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
async fn test_starrocks_command_get_table_structure_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id =
        create_starrocks_connection_for_state(&state, &form, "meta-structure-success").await;

    let db_name = unique_name("dbpaw_starrocks_meta_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form_with_db, &db_name, &parent, &child).await;

    let structure =
        metadata::get_table_structure_direct(&state, conn_id, db_name.clone(), child.clone())
            .await
            .expect("get_table_structure should succeed");
    assert!(structure.columns.iter().any(|c| c.name == "id"));
    assert!(structure.columns.iter().any(|c| c.name == "parent_id"));

    cleanup_metadata_fixture(&form_with_db, &db_name, &parent, &child).await;
    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_get_table_structure_missing_table_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id =
        create_starrocks_connection_for_state(&state, &form, "meta-structure-missing").await;

    let db_name = unique_name("dbpaw_starrocks_missing_meta_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    let _ = driver.close().await;

    let missing_table = unique_name("dbpaw_meta_missing");
    let result =
        metadata::get_table_structure_direct(&state, conn_id, db_name.clone(), missing_table).await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(!err.trim().is_empty());

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_get_table_ddl_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id = create_starrocks_connection_for_state(&state, &form, "meta-ddl-success").await;

    let db_name = unique_name("dbpaw_starrocks_ddl_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form_with_db, &db_name, &parent, &child).await;

    let ddl = metadata::get_table_ddl_direct(
        &state,
        conn_id,
        Some(db_name.clone()),
        db_name.clone(),
        child.clone(),
    )
    .await
    .expect("get_table_ddl should succeed");
    assert!(ddl.to_uppercase().contains("CREATE TABLE"));
    assert!(ddl.contains(&child));

    cleanup_metadata_fixture(&form_with_db, &db_name, &parent, &child).await;
    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_get_schema_overview_contains_target_schema() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id =
        create_starrocks_connection_for_state(&state, &form, "meta-schema-overview").await;

    let db_name = unique_name("dbpaw_starrocks_overview_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let mut form_with_db = form.clone();
    form_with_db.database = Some(db_name.clone());

    let parent = unique_name("dbpaw_meta_parent");
    let child = unique_name("dbpaw_meta_child");
    prepare_metadata_fixture(&form_with_db, &db_name, &parent, &child).await;

    let overview = metadata::get_schema_overview_direct(
        &state,
        conn_id,
        Some(db_name.clone()),
        Some(db_name.clone()),
    )
    .await
    .expect("get_schema_overview should succeed");
    assert!(overview
        .tables
        .iter()
        .any(|t| t.schema == db_name && t.name == child));

    cleanup_metadata_fixture(&form_with_db, &db_name, &parent, &child).await;
    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_execute_query_by_id_success() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id = create_starrocks_connection_for_state(&state, &form, "query-by-id-success").await;

    let db_name = unique_name("dbpaw_starrocks_query_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let result = query::execute_query_by_id_direct(
        &state,
        conn_id,
        "SELECT 1 AS v".to_string(),
        Some(db_name.clone()),
        Some("phase4_success".to_string()),
        Some("phase4-qid-success".to_string()),
    )
    .await
    .expect("execute_query_by_id should succeed");
    assert!(result.success);
    assert!(result.row_count >= 1);
    assert!(!result.data.is_empty());

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_execute_query_by_id_invalid_sql_returns_error() {
    let docker = (!starrocks_context::should_reuse_local_db()).then(Cli::default);
    let (_container, form) = starrocks_context::starrocks_form_from_test_context(docker.as_ref());
    wait_until_starrocks_ready(&form).await;

    let state = init_state_with_local_db().await;
    let conn_id =
        create_starrocks_connection_for_state(&state, &form, "query-by-id-invalid").await;

    let db_name = unique_name("dbpaw_starrocks_invalid_query_db");
    let driver = MysqlDriver::connect(&form)
        .await
        .expect("failed to connect starrocks driver");
    driver
        .execute_query(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .await
        .expect("create database failed");
    driver.close().await;

    let result = query::execute_query_by_id_direct(
        &state,
        conn_id,
        "SELECT * FROM __dbpaw_missing_phase4_table".to_string(),
        Some(db_name.clone()),
        Some("phase4_invalid".to_string()),
        Some("phase4-qid-invalid".to_string()),
    )
    .await;
    assert!(result.is_err());
    let err = result.err().unwrap_or_default();
    assert!(!err.trim().is_empty());

    let _ = driver
        .execute_query(format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .await;
    let _ = connection::delete_connection_direct(&state, conn_id).await;
}

#[tokio::test]
#[ignore]
async fn test_starrocks_command_storage_saved_query_crud_flow() {
    let state = init_state_with_local_db().await;
    let name = unique_name("saved_query");
    let created = storage::save_query_direct(
        &state,
        name.clone(),
        "SELECT 1".to_string(),
        Some("desc".to_string()),
        None,
        Some("test_db".to_string()),
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
        Some("test_db".to_string()),
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
