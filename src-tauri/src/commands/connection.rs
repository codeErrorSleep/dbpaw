use crate::models::{Connection, ConnectionForm, TestConnectionResult};
use crate::state::AppState;
use serde::Deserialize;
use std::time::Instant;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabasePayload {
    pub name: String,
    pub if_not_exists: Option<bool>,
    pub charset: Option<String>,
    pub collation: Option<String>,
    pub encoding: Option<String>,
    pub lc_collate: Option<String>,
    pub lc_ctype: Option<String>,
}

fn validate_database_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("[VALIDATION_ERROR] Database name cannot be empty".to_string());
    }
    if trimmed.contains('\0') {
        return Err("[VALIDATION_ERROR] Database name contains null byte".to_string());
    }
    if trimmed.len() > 128 {
        return Err("[VALIDATION_ERROR] Database name is too long (max 128)".to_string());
    }
    Ok(trimmed.to_string())
}

fn is_safe_option_token(raw: &str) -> bool {
    !raw.is_empty()
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'))
}

fn normalize_option_token(opt: &Option<String>, field: &str) -> Result<Option<String>, String> {
    let Some(value) = opt else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !is_safe_option_token(trimmed) {
        return Err(format!(
            "[VALIDATION_ERROR] Invalid characters in {}",
            field
        ));
    }
    Ok(Some(trimmed.to_string()))
}

fn quote_mysql_ident(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}

fn quote_pg_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn quote_mssql_ident(ident: &str) -> String {
    format!("[{}]", ident.replace(']', "]]"))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn quote_nliteral(value: &str) -> String {
    format!("N'{}'", value.replace('\'', "''"))
}

fn build_mysql_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, String> {
    let charset = normalize_option_token(&payload.charset, "charset")?;
    let collation = normalize_option_token(&payload.collation, "collation")?;
    let mut sql = String::from("CREATE DATABASE ");
    if payload.if_not_exists.unwrap_or(true) {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&quote_mysql_ident(db_name));
    if let Some(charset) = charset {
        sql.push_str(" CHARACTER SET ");
        sql.push_str(&charset);
    }
    if let Some(collation) = collation {
        sql.push_str(" COLLATE ");
        sql.push_str(&collation);
    }
    Ok(sql)
}

fn build_postgres_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, String> {
    let encoding = normalize_option_token(&payload.encoding, "encoding")?;
    let lc_collate = normalize_option_token(&payload.lc_collate, "lc_collate")?;
    let lc_ctype = normalize_option_token(&payload.lc_ctype, "lc_ctype")?;

    let mut options = Vec::new();
    if let Some(v) = encoding {
        options.push(format!("ENCODING = {}", quote_literal(&v)));
    }
    if let Some(v) = lc_collate {
        options.push(format!("LC_COLLATE = {}", quote_literal(&v)));
    }
    if let Some(v) = lc_ctype {
        options.push(format!("LC_CTYPE = {}", quote_literal(&v)));
    }

    let mut sql = format!("CREATE DATABASE {}", quote_pg_ident(db_name));
    if !options.is_empty() {
        sql.push_str(" WITH ");
        sql.push_str(&options.join(" "));
    }
    Ok(sql)
}

fn build_mssql_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, String> {
    let collation = normalize_option_token(&payload.collation, "collation")?;
    let mut create_sql = format!("CREATE DATABASE {}", quote_mssql_ident(db_name));
    if let Some(collation) = collation {
        create_sql.push_str(" COLLATE ");
        create_sql.push_str(&collation);
    }

    if payload.if_not_exists.unwrap_or(true) {
        return Ok(format!(
            "IF DB_ID({}) IS NULL {}",
            quote_nliteral(db_name),
            create_sql
        ));
    }
    Ok(create_sql)
}

fn normalize_create_database_error(err: String, db_name: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("already exists")
        || lower.contains("duplicate database")
        || lower.contains("database exists")
        || lower.contains("42p04")
        || lower.contains("2714")
    {
        return format!(
            "[ALREADY_EXISTS] Database '{}' already exists. {}",
            db_name, err
        );
    }
    if lower.contains("permission denied")
        || lower.contains("access denied")
        || lower.contains("not authorized")
        || lower.contains("insufficient privilege")
    {
        return format!("[PERMISSION_DENIED] {}", err);
    }
    err
}

#[tauri::command]
pub async fn list_databases(form: ConnectionForm) -> Result<Vec<String>, String> {
    let driver = crate::db::drivers::connect(&form).await?;
    driver.list_databases().await
}

#[tauri::command]
pub async fn list_databases_by_id(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<String>, String> {
    super::execute_with_retry(&state, id, None, |driver| async move {
        driver.list_databases().await
    })
    .await
}

#[tauri::command]
pub async fn create_database_by_id(
    state: State<'_, AppState>,
    id: i64,
    payload: CreateDatabasePayload,
) -> Result<(), String> {
    let db_name = validate_database_name(&payload.name)?;
    let if_not_exists = payload.if_not_exists.unwrap_or(true);
    let driver = {
        let local_db = {
            let lock = state.local_db.lock().await;
            lock.clone()
        };
        let db = local_db.ok_or("Local DB not initialized".to_string())?;
        db.get_connection_form_by_id(id)
            .await?
            .driver
            .to_lowercase()
    };

    if matches!(driver.as_str(), "sqlite" | "duckdb" | "clickhouse") {
        return Err(format!(
            "[UNSUPPORTED] Driver {} does not support creating databases in this flow",
            driver
        ));
    }

    let exec_res = match driver.as_str() {
        "mysql" | "mariadb" | "tidb" => {
            let sql = build_mysql_create_database_sql(&payload, &db_name)?;
            super::execute_with_retry(&state, id, None, |driver| {
                let sql_clone = sql.clone();
                async move { driver.execute_query(sql_clone).await.map(|_| ()) }
            })
            .await
        }
        "postgres" => {
            let create_sql = build_postgres_create_database_sql(&payload, &db_name)?;
            let exists_check_sql = format!(
                "SELECT 1 FROM pg_database WHERE datname = {} LIMIT 1",
                quote_literal(&db_name)
            );
            super::execute_with_retry(&state, id, None, |driver| {
                let exists_sql = exists_check_sql.clone();
                let create_sql = create_sql.clone();
                async move {
                    if if_not_exists {
                        let exists_result = driver.execute_query(exists_sql).await?;
                        if exists_result.row_count > 0 || !exists_result.data.is_empty() {
                            return Ok(());
                        }
                    }
                    driver.execute_query(create_sql).await.map(|_| ())
                }
            })
            .await
        }
        "mssql" => {
            let sql = build_mssql_create_database_sql(&payload, &db_name)?;
            super::execute_with_retry(&state, id, None, |driver| {
                let sql_clone = sql.clone();
                async move { driver.execute_query(sql_clone).await.map(|_| ()) }
            })
            .await
        }
        _ => Err(format!(
            "[UNSUPPORTED] Driver {} not supported for create database",
            driver
        )),
    };

    exec_res.map_err(|e| normalize_create_database_error(e, &db_name))
}

#[tauri::command]
pub async fn test_connection_ephemeral(
    form: ConnectionForm,
) -> Result<TestConnectionResult, String> {
    let start = Instant::now();
    let driver = crate::db::drivers::connect(&form).await?;
    driver.test_connection().await.map_err(|e| e.to_string())?;

    let elapsed = start.elapsed().as_millis() as i64;
    Ok(TestConnectionResult {
        success: true,
        message: "Connection successful".to_string(),
        latency_ms: Some(elapsed),
    })
}

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        db.list_connections().await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[tauri::command]
pub async fn create_connection(
    state: State<'_, AppState>,
    form: ConnectionForm,
) -> Result<Connection, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        db.create_connection(form).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[tauri::command]
pub async fn update_connection(
    state: State<'_, AppState>,
    id: i64,
    form: ConnectionForm,
) -> Result<Connection, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        // If connection is updated, we should remove it from pool so next usage reconnects with new config
        state.pool_manager.remove_by_prefix(&id.to_string()).await;

        db.update_connection(id, form).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        // Remove from pool
        state.pool_manager.remove_by_prefix(&id.to_string()).await;

        db.delete_connection(id).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_mssql_create_database_sql, build_mysql_create_database_sql,
        build_postgres_create_database_sql, validate_database_name, CreateDatabasePayload,
    };

    #[test]
    fn validate_database_name_rejects_empty_and_null() {
        assert!(validate_database_name("  ").is_err());
        assert!(validate_database_name("ab\0cd").is_err());
    }

    #[test]
    fn mysql_sql_contains_if_not_exists_charset_and_collation() {
        let sql = build_mysql_create_database_sql(
            &CreateDatabasePayload {
                name: "foo".to_string(),
                if_not_exists: Some(true),
                charset: Some("utf8mb4".to_string()),
                collation: Some("utf8mb4_general_ci".to_string()),
                encoding: None,
                lc_collate: None,
                lc_ctype: None,
            },
            "foo",
        )
        .unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE IF NOT EXISTS `foo` CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci"
        );
    }

    #[test]
    fn postgres_sql_contains_options() {
        let sql = build_postgres_create_database_sql(
            &CreateDatabasePayload {
                name: "foo".to_string(),
                if_not_exists: Some(true),
                charset: None,
                collation: None,
                encoding: Some("UTF8".to_string()),
                lc_collate: Some("en_US.UTF-8".to_string()),
                lc_ctype: Some("en_US.UTF-8".to_string()),
            },
            "foo",
        )
        .unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE \"foo\" WITH ENCODING = 'UTF8' LC_COLLATE = 'en_US.UTF-8' LC_CTYPE = 'en_US.UTF-8'"
        );
    }

    #[test]
    fn mssql_sql_wraps_with_if_not_exists() {
        let sql = build_mssql_create_database_sql(
            &CreateDatabasePayload {
                name: "foo".to_string(),
                if_not_exists: Some(true),
                charset: None,
                collation: Some("SQL_Latin1_General_CP1_CI_AS".to_string()),
                encoding: None,
                lc_collate: None,
                lc_ctype: None,
            },
            "foo",
        )
        .unwrap();
        assert_eq!(
            sql,
            "IF DB_ID(N'foo') IS NULL CREATE DATABASE [foo] COLLATE SQL_Latin1_General_CP1_CI_AS"
        );
    }
}
