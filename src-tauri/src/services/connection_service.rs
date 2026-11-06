use crate::db::drivers;
use crate::error::AppError;
use crate::models::{ConnectionForm, TestConnectionResult};
use crate::state::AppState;
use serde::Deserialize;

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

pub fn validate_database_name(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("Database name cannot be empty"));
    }
    if trimmed.contains('\0') {
        return Err(AppError::validation("Database name contains null byte"));
    }
    if trimmed.len() > 128 {
        return Err(AppError::validation("Database name is too long (max 128)"));
    }
    Ok(trimmed.to_string())
}

fn is_safe_option_token(raw: &str) -> bool {
    !raw.is_empty()
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'))
}

fn normalize_option_token(opt: &Option<String>, field: &str) -> Result<Option<String>, AppError> {
    let Some(value) = opt else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !is_safe_option_token(trimmed) {
        return Err(AppError::validation(format!(
            "Invalid characters in {}",
            field
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn quote_mysql_ident(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}

fn quote_clickhouse_ident(ident: &str) -> String {
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

fn quote_cql_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

pub fn build_mysql_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, AppError> {
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

pub fn build_postgres_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, AppError> {
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

pub fn build_mssql_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, AppError> {
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

pub fn build_clickhouse_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, AppError> {
    if let Some(v) = normalize_option_token(&payload.charset, "charset")? {
        return Err(AppError::unsupported(format!(
            "ClickHouse create database does not support charset option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.collation, "collation")? {
        return Err(AppError::unsupported(format!(
            "ClickHouse create database does not support collation option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.encoding, "encoding")? {
        return Err(AppError::unsupported(format!(
            "ClickHouse create database does not support encoding option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.lc_collate, "lc_collate")? {
        return Err(AppError::unsupported(format!(
            "ClickHouse create database does not support lc_collate option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.lc_ctype, "lc_ctype")? {
        return Err(AppError::unsupported(format!(
            "ClickHouse create database does not support lc_ctype option: {}",
            v
        )));
    }

    let mut sql = String::from("CREATE DATABASE ");
    if payload.if_not_exists.unwrap_or(true) {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&quote_clickhouse_ident(db_name));
    Ok(sql)
}

pub fn build_cassandra_create_database_sql(
    payload: &CreateDatabasePayload,
    db_name: &str,
) -> Result<String, AppError> {
    if let Some(v) = normalize_option_token(&payload.charset, "charset")? {
        return Err(AppError::unsupported(format!(
            "Cassandra create keyspace does not support charset option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.collation, "collation")? {
        return Err(AppError::unsupported(format!(
            "Cassandra create keyspace does not support collation option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.encoding, "encoding")? {
        return Err(AppError::unsupported(format!(
            "Cassandra create keyspace does not support encoding option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.lc_collate, "lc_collate")? {
        return Err(AppError::unsupported(format!(
            "Cassandra create keyspace does not support lc_collate option: {}",
            v
        )));
    }
    if let Some(v) = normalize_option_token(&payload.lc_ctype, "lc_ctype")? {
        return Err(AppError::unsupported(format!(
            "Cassandra create keyspace does not support lc_ctype option: {}",
            v
        )));
    }

    let mut sql = String::from("CREATE KEYSPACE ");
    if payload.if_not_exists.unwrap_or(true) {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&quote_cql_ident(db_name));
    sql.push_str(" WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}");
    Ok(sql)
}

pub fn normalize_create_database_error(err: AppError, db_name: &str) -> AppError {
    let lower = err.to_string().to_lowercase();
    if lower.contains("already exists")
        || lower.contains("duplicate database")
        || lower.contains("database exists")
        || lower.contains("42p04")
        || lower.contains("2714")
    {
        return AppError::already_exists(format!("Database '{}' already exists. {}", db_name, err));
    }
    if lower.contains("permission denied")
        || lower.contains("access denied")
        || lower.contains("not authorized")
        || lower.contains("insufficient privilege")
    {
        return AppError::permission_denied(format!("{}", err));
    }
    err
}

pub async fn list_databases(form: ConnectionForm) -> Result<Vec<String>, AppError> {
    let form =
        crate::connection_input::normalize_connection_form(form).map_err(AppError::internal)?;
    let driver = drivers::connect(&form).await?;
    driver.list_databases().await
}

pub async fn list_databases_by_id(state: &AppState, id: i64) -> Result<Vec<String>, AppError> {
    crate::commands::execute_with_retry_from_app_state(state, id, None, |driver| async move {
        driver.list_databases().await
    })
    .await
}

pub async fn create_database_by_id(
    state: &AppState,
    id: i64,
    payload: CreateDatabasePayload,
) -> Result<(), AppError> {
    let db_name = validate_database_name(&payload.name)?;

    crate::commands::execute_with_retry_from_app_state(state, id, None, |driver| {
        let db_name = db_name.clone();
        let payload = payload.clone();
        async move {
            let form = {
                let local_db = {
                    let lock = state.local_db.lock().await;
                    lock.clone()
                };
                let db = local_db.ok_or_else(|| AppError::internal("Local DB not initialized"))?;
                db.get_connection_form_by_id(id).await?
            };

            let sql = match form.driver.as_str() {
                "mysql" | "mariadb" | "tidb" | "starrocks" | "doris" => {
                    build_mysql_create_database_sql(&payload, &db_name)?
                }
                "postgresql" | "postgres" => {
                    build_postgres_create_database_sql(&payload, &db_name)?
                }
                "sqlserver" | "mssql" => build_mssql_create_database_sql(&payload, &db_name)?,
                "clickhouse" => build_clickhouse_create_database_sql(&payload, &db_name)?,
                "cassandra" => build_cassandra_create_database_sql(&payload, &db_name)?,
                _ => {
                    return Err(AppError::unsupported(format!(
                        "Create database not supported for driver: {}",
                        form.driver
                    )));
                }
            };

            driver
                .execute_query(sql)
                .await
                .map(|_| ())
                .map_err(|e| normalize_create_database_error(e, &db_name))
        }
    })
    .await
}

pub async fn get_mysql_charsets_by_id(state: &AppState, id: i64) -> Result<Vec<String>, AppError> {
    crate::commands::execute_with_retry_from_app_state(state, id, None, |driver| async move {
        let result = driver
            .execute_query("SHOW CHARACTER SET".to_string())
            .await?;
        let mut charsets: Vec<String> = result
            .data
            .iter()
            .filter_map(|row| {
                row.get("Charset")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        charsets.sort();
        Ok::<Vec<String>, AppError>(charsets)
    })
    .await
}

pub async fn get_mysql_collations_by_id(
    state: &AppState,
    id: i64,
    charset: String,
) -> Result<Vec<String>, AppError> {
    let sql = format!(
        "SHOW COLLATION WHERE Charset = '{}'",
        charset.replace('\'', "''")
    );
    crate::commands::execute_with_retry_from_app_state(state, id, None, |driver| {
        let sql = sql.clone();
        async move {
            let result = driver.execute_query(sql).await?;
            let mut collations: Vec<String> = result
                .data
                .iter()
                .filter_map(|row| {
                    row.get("Collation")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            collations.sort();
            Ok::<Vec<String>, AppError>(collations)
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::codes;

    fn empty_payload() -> CreateDatabasePayload {
        CreateDatabasePayload {
            name: "testdb".to_string(),
            if_not_exists: None,
            charset: None,
            collation: None,
            encoding: None,
            lc_collate: None,
            lc_ctype: None,
        }
    }

    // --- validate_database_name ---

    #[test]
    fn validate_database_name_trims_whitespace() {
        assert_eq!(validate_database_name("  mydb  ").unwrap(), "mydb");
    }

    #[test]
    fn validate_database_name_rejects_empty() {
        let err = validate_database_name("   ").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn validate_database_name_rejects_null_byte() {
        let err = validate_database_name("my\0db").unwrap_err();
        assert!(err.to_string().contains("null byte"));
    }

    #[test]
    fn validate_database_name_rejects_too_long() {
        let long_name = "a".repeat(129);
        let err = validate_database_name(&long_name).unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn validate_database_name_accepts_128_chars() {
        let name = "a".repeat(128);
        assert_eq!(validate_database_name(&name).unwrap(), name);
    }

    // --- is_safe_option_token ---

    #[test]
    fn is_safe_option_token_accepts_alphanumeric() {
        assert!(is_safe_option_token("utf8mb4"));
    }

    #[test]
    fn is_safe_option_token_accepts_special_chars() {
        assert!(is_safe_option_token("en_US.UTF-8"));
        assert!(is_safe_option_token("some_option-name@test"));
    }

    #[test]
    fn is_safe_option_token_rejects_empty() {
        assert!(!is_safe_option_token(""));
    }

    #[test]
    fn is_safe_option_token_rejects_spaces() {
        assert!(!is_safe_option_token("has space"));
    }

    #[test]
    fn is_safe_option_token_rejects_semicolons() {
        assert!(!is_safe_option_token("utf8;DROP"));
    }

    #[test]
    fn is_safe_option_token_rejects_quotes() {
        assert!(!is_safe_option_token("utf8'"));
        assert!(!is_safe_option_token("utf8\""));
    }

    // --- normalize_option_token ---

    #[test]
    fn normalize_option_token_returns_none_for_absent() {
        assert_eq!(normalize_option_token(&None, "charset").unwrap(), None);
    }

    #[test]
    fn normalize_option_token_returns_none_for_empty_string() {
        assert_eq!(
            normalize_option_token(&Some("  ".to_string()), "charset").unwrap(),
            None
        );
    }

    #[test]
    fn normalize_option_token_trims_value() {
        assert_eq!(
            normalize_option_token(&Some("  utf8  ".to_string()), "charset").unwrap(),
            Some("utf8".to_string())
        );
    }

    #[test]
    fn normalize_option_token_rejects_unsafe_chars() {
        let err = normalize_option_token(&Some("bad;value".to_string()), "charset").unwrap_err();
        assert!(err.to_string().contains("Invalid characters"));
    }

    // --- quote helpers ---

    #[test]
    fn quote_mysql_ident_basic() {
        assert_eq!(quote_mysql_ident("users"), "`users`");
    }

    #[test]
    fn quote_mysql_ident_escapes_backtick() {
        assert_eq!(quote_mysql_ident("user`s"), "`user``s`");
    }

    #[test]
    fn quote_clickhouse_ident_basic() {
        assert_eq!(quote_clickhouse_ident("events"), "`events`");
    }

    #[test]
    fn quote_pg_ident_basic() {
        assert_eq!(quote_pg_ident("public"), "\"public\"");
    }

    #[test]
    fn quote_pg_ident_escapes_double_quote() {
        assert_eq!(quote_pg_ident("my\"table"), "\"my\"\"table\"");
    }

    #[test]
    fn quote_mssql_ident_basic() {
        assert_eq!(quote_mssql_ident("dbo"), "[dbo]");
    }

    #[test]
    fn quote_mssql_ident_escapes_bracket() {
        assert_eq!(quote_mssql_ident("my]table"), "[my]]table]");
    }

    #[test]
    fn quote_literal_basic() {
        assert_eq!(quote_literal("hello"), "'hello'");
    }

    #[test]
    fn quote_literal_escapes_single_quote() {
        assert_eq!(quote_literal("it's"), "'it''s'");
    }

    #[test]
    fn quote_nliteral_basic() {
        assert_eq!(quote_nliteral("test"), "N'test'");
    }

    #[test]
    fn quote_nliteral_escapes_single_quote() {
        assert_eq!(quote_nliteral("it's"), "N'it''s'");
    }

    #[test]
    fn quote_cql_ident_basic() {
        assert_eq!(quote_cql_ident("ks"), "\"ks\"");
    }

    #[test]
    fn quote_cql_ident_escapes_double_quote() {
        assert_eq!(quote_cql_ident("my\"ks"), "\"my\"\"ks\"");
    }

    // --- build_mysql_create_database_sql ---

    #[test]
    fn build_mysql_create_database_sql_basic() {
        let payload = empty_payload();
        let sql = build_mysql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(sql, "CREATE DATABASE IF NOT EXISTS `mydb`");
    }

    #[test]
    fn build_mysql_create_database_sql_no_if_not_exists() {
        let mut payload = empty_payload();
        payload.if_not_exists = Some(false);
        let sql = build_mysql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(sql, "CREATE DATABASE `mydb`");
    }

    #[test]
    fn build_mysql_create_database_sql_with_charset() {
        let mut payload = empty_payload();
        payload.charset = Some("utf8mb4".to_string());
        let sql = build_mysql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE IF NOT EXISTS `mydb` CHARACTER SET utf8mb4"
        );
    }

    #[test]
    fn build_mysql_create_database_sql_with_collation() {
        let mut payload = empty_payload();
        payload.collation = Some("utf8mb4_unicode_ci".to_string());
        let sql = build_mysql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE IF NOT EXISTS `mydb` COLLATE utf8mb4_unicode_ci"
        );
    }

    #[test]
    fn build_mysql_create_database_sql_with_charset_and_collation() {
        let mut payload = empty_payload();
        payload.charset = Some("utf8mb4".to_string());
        payload.collation = Some("utf8mb4_unicode_ci".to_string());
        let sql = build_mysql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE IF NOT EXISTS `mydb` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        );
    }

    #[test]
    fn build_mysql_create_database_sql_rejects_unsafe_charset() {
        let mut payload = empty_payload();
        payload.charset = Some("bad;injection".to_string());
        assert!(build_mysql_create_database_sql(&payload, "mydb").is_err());
    }

    // --- build_postgres_create_database_sql ---

    #[test]
    fn build_postgres_create_database_sql_basic() {
        let payload = empty_payload();
        let sql = build_postgres_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(sql, "CREATE DATABASE \"mydb\"");
    }

    #[test]
    fn build_postgres_create_database_sql_with_encoding() {
        let mut payload = empty_payload();
        payload.encoding = Some("UTF8".to_string());
        let sql = build_postgres_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(sql, "CREATE DATABASE \"mydb\" WITH ENCODING = 'UTF8'");
    }

    #[test]
    fn build_postgres_create_database_sql_with_all_options() {
        let mut payload = empty_payload();
        payload.encoding = Some("UTF8".to_string());
        payload.lc_collate = Some("en_US.UTF-8".to_string());
        payload.lc_ctype = Some("en_US.UTF-8".to_string());
        let sql = build_postgres_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE \"mydb\" WITH ENCODING = 'UTF8' LC_COLLATE = 'en_US.UTF-8' LC_CTYPE = 'en_US.UTF-8'"
        );
    }

    #[test]
    fn build_postgres_create_database_sql_escapes_name() {
        let payload = empty_payload();
        let sql = build_postgres_create_database_sql(&payload, "my\"db").unwrap();
        assert_eq!(sql, "CREATE DATABASE \"my\"\"db\"");
    }

    // --- build_mssql_create_database_sql ---

    #[test]
    fn build_mssql_create_database_sql_basic_with_guard() {
        let payload = empty_payload();
        let sql = build_mssql_create_database_sql(&payload, "mydb").unwrap();
        assert!(sql.contains("IF DB_ID"));
        assert!(sql.contains("CREATE DATABASE [mydb]"));
    }

    #[test]
    fn build_mssql_create_database_sql_no_guard() {
        let mut payload = empty_payload();
        payload.if_not_exists = Some(false);
        let sql = build_mssql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(sql, "CREATE DATABASE [mydb]");
    }

    #[test]
    fn build_mssql_create_database_sql_with_collation() {
        let mut payload = empty_payload();
        payload.if_not_exists = Some(false);
        payload.collation = Some("SQL_Latin1_General_CP1_CI_AS".to_string());
        let sql = build_mssql_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(
            sql,
            "CREATE DATABASE [mydb] COLLATE SQL_Latin1_General_CP1_CI_AS"
        );
    }

    // --- build_clickhouse_create_database_sql ---

    #[test]
    fn build_clickhouse_create_database_sql_basic() {
        let payload = empty_payload();
        let sql = build_clickhouse_create_database_sql(&payload, "mydb").unwrap();
        assert_eq!(sql, "CREATE DATABASE IF NOT EXISTS `mydb`");
    }

    #[test]
    fn build_clickhouse_create_database_sql_rejects_charset() {
        let mut payload = empty_payload();
        payload.charset = Some("utf8".to_string());
        assert!(build_clickhouse_create_database_sql(&payload, "mydb").is_err());
    }

    #[test]
    fn build_clickhouse_create_database_sql_rejects_collation() {
        let mut payload = empty_payload();
        payload.collation = Some("utf8_general_ci".to_string());
        assert!(build_clickhouse_create_database_sql(&payload, "mydb").is_err());
    }

    #[test]
    fn build_clickhouse_create_database_sql_rejects_encoding() {
        let mut payload = empty_payload();
        payload.encoding = Some("UTF8".to_string());
        assert!(build_clickhouse_create_database_sql(&payload, "mydb").is_err());
    }

    #[test]
    fn build_clickhouse_create_database_sql_rejects_lc_collate() {
        let mut payload = empty_payload();
        payload.lc_collate = Some("en_US".to_string());
        assert!(build_clickhouse_create_database_sql(&payload, "mydb").is_err());
    }

    #[test]
    fn build_clickhouse_create_database_sql_rejects_lc_ctype() {
        let mut payload = empty_payload();
        payload.lc_ctype = Some("en_US".to_string());
        assert!(build_clickhouse_create_database_sql(&payload, "mydb").is_err());
    }

    // --- build_cassandra_create_database_sql ---

    #[test]
    fn build_cassandra_create_database_sql_basic() {
        let payload = empty_payload();
        let sql = build_cassandra_create_database_sql(&payload, "myks").unwrap();
        assert_eq!(
            sql,
            "CREATE KEYSPACE IF NOT EXISTS \"myks\" WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}"
        );
    }

    #[test]
    fn build_cassandra_create_database_sql_no_if_not_exists() {
        let mut payload = empty_payload();
        payload.if_not_exists = Some(false);
        let sql = build_cassandra_create_database_sql(&payload, "myks").unwrap();
        assert_eq!(
            sql,
            "CREATE KEYSPACE \"myks\" WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}"
        );
    }

    #[test]
    fn build_cassandra_create_database_sql_rejects_charset() {
        let mut payload = empty_payload();
        payload.charset = Some("utf8".to_string());
        assert!(build_cassandra_create_database_sql(&payload, "myks").is_err());
    }

    #[test]
    fn build_cassandra_create_database_sql_rejects_collation() {
        let mut payload = empty_payload();
        payload.collation = Some("utf8_general_ci".to_string());
        assert!(build_cassandra_create_database_sql(&payload, "myks").is_err());
    }

    #[test]
    fn build_cassandra_create_database_sql_rejects_encoding() {
        let mut payload = empty_payload();
        payload.encoding = Some("UTF8".to_string());
        assert!(build_cassandra_create_database_sql(&payload, "myks").is_err());
    }

    #[test]
    fn build_cassandra_create_database_sql_rejects_lc_collate() {
        let mut payload = empty_payload();
        payload.lc_collate = Some("en_US".to_string());
        assert!(build_cassandra_create_database_sql(&payload, "myks").is_err());
    }

    #[test]
    fn build_cassandra_create_database_sql_rejects_lc_ctype() {
        let mut payload = empty_payload();
        payload.lc_ctype = Some("en_US".to_string());
        assert!(build_cassandra_create_database_sql(&payload, "myks").is_err());
    }

    // --- normalize_create_database_error ---

    #[test]
    fn normalize_create_database_error_already_exists_mysql() {
        let err = AppError::query_failed("Error 1007: Can't create database; database exists");
        let normalized = normalize_create_database_error(err, "mydb");
        assert_eq!(normalized.code(), codes::ALREADY_EXISTS);
        assert!(normalized.to_string().contains("already exists"));
    }

    #[test]
    fn normalize_create_database_error_already_exists_pg() {
        let err = AppError::query_failed("42P04: database \"mydb\" already exists");
        let normalized = normalize_create_database_error(err, "mydb");
        assert_eq!(normalized.code(), codes::ALREADY_EXISTS);
    }

    #[test]
    fn normalize_create_database_error_already_exists_duplicate() {
        let err = AppError::query_failed("duplicate database name");
        let normalized = normalize_create_database_error(err, "mydb");
        assert_eq!(normalized.code(), codes::ALREADY_EXISTS);
    }

    #[test]
    fn normalize_create_database_error_permission_denied() {
        let err = AppError::query_failed("permission denied for database");
        let normalized = normalize_create_database_error(err, "mydb");
        assert_eq!(normalized.code(), codes::PERMISSION_DENIED);
    }

    #[test]
    fn normalize_create_database_error_access_denied() {
        let err = AppError::query_failed("Access denied for user");
        let normalized = normalize_create_database_error(err, "mydb");
        assert_eq!(normalized.code(), codes::PERMISSION_DENIED);
    }

    #[test]
    fn normalize_create_database_error_passthrough_other() {
        let err = AppError::query_failed("some other error");
        let normalized = normalize_create_database_error(err, "mydb");
        assert_eq!(normalized.code(), codes::QUERY_FAILED);
    }
}

pub async fn test_connection_ephemeral(
    form: ConnectionForm,
) -> Result<TestConnectionResult, AppError> {
    let start = std::time::Instant::now();
    let form =
        crate::connection_input::normalize_connection_form(form).map_err(AppError::internal)?;

    match drivers::connect(&form).await {
        Ok(driver) => {
            let latency = start.elapsed().as_millis() as i64;
            driver.close().await;
            Ok(TestConnectionResult {
                success: true,
                message: "Connection successful".to_string(),
                latency_ms: Some(latency),
            })
        }
        Err(e) => Ok(TestConnectionResult {
            success: false,
            message: e.to_string(),
            latency_ms: None,
        }),
    }
}
