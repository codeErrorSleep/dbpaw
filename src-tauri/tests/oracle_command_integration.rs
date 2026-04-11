#[path = "common/oracle_context.rs"]
mod oracle_context;

use dbpaw_lib::commands::{connection, metadata, query, transfer};
use dbpaw_lib::db::drivers::oracle::OracleDriver;
use dbpaw_lib::db::drivers::DatabaseDriver;
use dbpaw_lib::db::local::LocalDb;
use dbpaw_lib::state::AppState;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_table_name(prefix: &str) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_millis();
    format!("{prefix}_{ms}")
}

async fn init_state_with_local_db() -> AppState {
    let state = AppState::new();
    let local_db_dir = std::env::temp_dir().join(unique_table_name("dbpaw_oracle_localdb_it"));
    let db = LocalDb::init_with_app_dir(&local_db_dir)
        .await
        .expect("failed to initialize local db");
    let mut lock = state.local_db.lock().await;
    *lock = Some(Arc::new(db));
    drop(lock);
    state
}

async fn create_oracle_connection_for_state(
    state: &AppState,
    base_form: &dbpaw_lib::models::ConnectionForm,
    suffix: &str,
) -> i64 {
    let mut form = base_form.clone();
    form.name = Some(format!("oracle-stateful-{suffix}"));
    let created = connection::create_connection_direct(state, form)
        .await
        .expect("create_connection should succeed");
    created.id
}

async fn prepare_test_table(schema: &str, table: &str, form: &dbpaw_lib::models::ConnectionForm) {
    let driver = OracleDriver::connect(form)
        .await
        .expect("connect for setup should succeed");
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" \
             (id NUMBER(10) PRIMARY KEY, name VARCHAR2(64))"
        ))
        .await
        .expect("CREATE TABLE should succeed");
    driver
        .execute_query(format!(
            "INSERT INTO \"{schema}\".\"{table}\" (id, name) VALUES (1, 'DbPaw')"
        ))
        .await
        .expect("INSERT should succeed");
    driver.close().await;
}

async fn cleanup_table(schema: &str, table: &str, form: &dbpaw_lib::models::ConnectionForm) {
    let driver = OracleDriver::connect(form)
        .await
        .expect("connect for cleanup should succeed");
    let _ = driver
        .execute_query(format!("DROP TABLE \"{schema}\".\"{table}\""))
        .await;
    driver.close().await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_test_connection_success() {
    let form = oracle_context::oracle_form_from_test_context();
    let result = connection::test_connection_ephemeral(form)
        .await
        .expect("test_connection_ephemeral should succeed");
    assert!(result.success);
    assert!(result.latency_ms.is_some());
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_test_connection_invalid_password_returns_error() {
    let mut form = oracle_context::oracle_form_from_test_context();
    form.password = Some("dbpaw_wrong_password_xyz".to_string());
    let result = connection::test_connection_ephemeral(form).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_list_databases_returns_schemas() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();

    let databases = connection::list_databases(form)
        .await
        .expect("list_databases should succeed");
    assert!(!databases.is_empty());
    assert!(
        databases.iter().any(|d| d == &schema),
        "schemas should include {schema}"
    );
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_list_tables_by_conn_contains_created_table() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_TABLES").to_uppercase();
    prepare_test_table(&schema, &table, &form).await;

    let tables = metadata::list_tables_by_conn(form.clone())
        .await
        .expect("list_tables_by_conn should succeed");
    assert!(
        tables.iter().any(|t| t.name == table),
        "tables should contain {table}"
    );

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_execute_select_returns_rows() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_SEL").to_uppercase();
    prepare_test_table(&schema, &table, &form).await;

    let sql = format!("SELECT id, name FROM \"{schema}\".\"{table}\" ORDER BY id");
    let result = query::execute_by_conn_direct(form.clone(), sql)
        .await
        .expect("execute SELECT should succeed");

    assert!(result.success);
    assert_eq!(result.row_count, 1);
    assert!(!result.data.is_empty());
    let row = result.data.first().unwrap();
    let name = row.get("NAME").and_then(|v| v.as_str());
    assert_eq!(name, Some("DbPaw"));

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_execute_invalid_sql_returns_error() {
    let form = oracle_context::oracle_form_from_test_context();
    let result =
        query::execute_by_conn_direct(form, "SELECT * FROM __dbpaw_no_such_table".to_string())
            .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.trim().is_empty());
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_execute_insert_affects_rows() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_INS").to_uppercase();

    let driver = OracleDriver::connect(&form)
        .await
        .expect("connect for setup");
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" \
             (id NUMBER(10) PRIMARY KEY, name VARCHAR2(64))"
        ))
        .await
        .expect("CREATE TABLE");
    driver.close().await;

    let sql = format!("INSERT INTO \"{schema}\".\"{table}\" (id, name) VALUES (1, 'alpha')");
    let result = query::execute_by_conn_direct(form.clone(), sql)
        .await
        .expect("INSERT should succeed");
    assert!(result.success);
    assert_eq!(result.row_count, 1);

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_get_table_data_pagination_works() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_PAGE").to_uppercase();

    let driver = OracleDriver::connect(&form)
        .await
        .expect("connect for setup");
    let _ = driver
        .execute_query(format!(
            "BEGIN \
               EXECUTE IMMEDIATE 'DROP TABLE \"{schema}\".\"{table}\"'; \
             EXCEPTION WHEN OTHERS THEN NULL; \
             END;"
        ))
        .await;
    driver
        .execute_query(format!(
            "CREATE TABLE \"{schema}\".\"{table}\" (id NUMBER(10) PRIMARY KEY)"
        ))
        .await
        .expect("CREATE TABLE");
    for i in 1..=3 {
        driver
            .execute_query(format!(
                "INSERT INTO \"{schema}\".\"{table}\" (id) VALUES ({i})"
            ))
            .await
            .expect("INSERT");
    }
    driver.close().await;

    let page1 = query::get_table_data_by_conn(form.clone(), schema.clone(), table.clone(), 1, 2)
        .await
        .expect("page 1 should succeed");
    let page2 = query::get_table_data_by_conn(form.clone(), schema.clone(), table.clone(), 2, 2)
        .await
        .expect("page 2 should succeed");

    assert_eq!(page1.total, 3);
    assert_eq!(page1.limit, 2);
    assert_eq!(page1.data.len(), 2);
    assert_eq!(page2.data.len(), 1);

    cleanup_table(&schema, &table, &form).await;
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_import_sql_file_supports_create_or_replace_script() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let state = init_state_with_local_db().await;
    let conn_id = create_oracle_connection_for_state(&state, &form, "import-plsql").await;

    let table = unique_table_name("DBPAW_IMPORT_ORA_TBL").to_uppercase();
    let proc_name = unique_table_name("DBPAW_IMPORT_ORA_PROC").to_uppercase();
    let base = std::env::temp_dir().join(unique_table_name("dbpaw_oracle_import_it"));
    fs::create_dir_all(&base).expect("create temp transfer dir should succeed");
    let import_sql_path = base.join("import.sql");

    let import_sql = format!(
        r#"
CREATE TABLE "{schema}"."{table}" (id NUMBER(10) PRIMARY KEY, name VARCHAR2(64));
CREATE OR REPLACE PROCEDURE "{schema}"."{proc_name}" IS
BEGIN
    INSERT INTO "{schema}"."{table}" (id, name) VALUES (1, 'from_proc');
END;
/
"#
    );
    fs::write(&import_sql_path, import_sql).expect("write import sql file should succeed");

    let import_result = transfer::import_sql_file_direct(
        &state,
        conn_id,
        Some(schema.clone()),
        import_sql_path.to_string_lossy().to_string(),
        "oracle".to_string(),
    )
    .await
    .expect("import_sql_file should succeed");
    assert_eq!(
        import_result.success_statements,
        import_result.total_statements
    );
    assert!(import_result.error.is_none());

    let driver = OracleDriver::connect(&form)
        .await
        .expect("connect for oracle import verification");
    driver
        .execute_query(format!(r#"BEGIN "{schema}"."{proc_name}"(); END;"#))
        .await
        .expect("calling imported oracle procedure should succeed");
    let verify = driver
        .execute_query(format!(r#"SELECT COUNT(*) AS C FROM "{schema}"."{table}""#))
        .await
        .expect("verify oracle imported procedure should succeed");
    let count = verify.data[0]["C"]
        .as_i64()
        .or_else(|| {
            verify.data[0]["C"]
                .as_str()
                .and_then(|v| v.parse::<i64>().ok())
        })
        .expect("oracle count should be numeric");
    assert_eq!(count, 1);

    let _ = driver
        .execute_query(format!(r#"DROP PROCEDURE "{schema}"."{proc_name}""#))
        .await;
    let _ = driver
        .execute_query(format!(r#"DROP TABLE "{schema}"."{table}""#))
        .await;
    driver.close().await;

    let _ = connection::delete_connection_direct(&state, conn_id).await;
    let _ = fs::remove_file(import_sql_path);
    let _ = fs::remove_dir_all(base);
}

#[tokio::test]
#[ignore]
async fn test_oracle_command_get_table_data_invalid_pagination_returns_error() {
    let form = oracle_context::oracle_form_from_test_context();
    let schema = form
        .schema
        .clone()
        .expect("ORACLE_SCHEMA must be set")
        .to_uppercase();
    let table = unique_table_name("DBPAW_CMD_INVP").to_uppercase();
    prepare_test_table(&schema, &table, &form).await;

    let result =
        query::get_table_data_by_conn(form.clone(), schema.clone(), table.clone(), 0, 10).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("[VALIDATION_ERROR]"));

    cleanup_table(&schema, &table, &form).await;
}
