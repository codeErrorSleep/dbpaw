use crate::models::{ConnectionForm, QueryResult, SqlExecutionLog, TableDataResponse};
use crate::state::AppState;
use tauri::{Emitter, State};

const DEFAULT_SELECT_LIMIT: i64 = 1000;

fn normalize_for_guard(sql: &str) -> &str {
    sql.trim()
}

fn skip_single_quote(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

fn skip_double_quote(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

fn skip_backtick_quote(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    i
}

fn skip_line_comment(bytes: &[u8], mut i: usize) -> usize {
    i += 2;
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

fn skip_block_comment(bytes: &[u8], mut i: usize) -> usize {
    i += 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    i
}

fn statement_kind_for_limit_guard(sql: &str) -> Option<&'static str> {
    let tokens = collect_top_level_keywords(sql);
    let first = tokens.first()?.as_str();

    if first == "select" {
        return Some("select");
    }
    if first != "with" {
        return Some("non_select");
    }

    for token in tokens.iter().skip(1) {
        if token == "select" {
            return Some("select");
        }
        if matches!(
            token.as_str(),
            "insert" | "update" | "delete" | "merge" | "replace" | "values"
        ) {
            return Some("non_select");
        }
    }

    Some("non_select")
}

fn is_single_statement(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut depth = 0_i32;

    while i < bytes.len() {
        let b = bytes[i];
        if i + 1 < bytes.len() && b == b'-' && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        if b == b'\'' {
            i = skip_single_quote(bytes, i);
            continue;
        }
        if b == b'"' {
            i = skip_double_quote(bytes, i);
            continue;
        }
        if b == b'`' {
            i = skip_backtick_quote(bytes, i);
            continue;
        }
        if b == b'(' {
            depth += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            depth -= 1;
            if depth < 0 {
                return false;
            }
            i += 1;
            continue;
        }
        if b == b';' && depth == 0 {
            i += 1;

            while i < bytes.len() {
                let c = bytes[i];
                if c.is_ascii_whitespace() || c == b';' {
                    i += 1;
                    continue;
                }
                if i + 1 < bytes.len() && c == b'-' && bytes[i + 1] == b'-' {
                    i = skip_line_comment(bytes, i);
                    continue;
                }
                if i + 1 < bytes.len() && c == b'/' && bytes[i + 1] == b'*' {
                    i = skip_block_comment(bytes, i);
                    continue;
                }
                return false;
            }
            return true;
        }
        i += 1;
    }

    depth == 0
}

fn collect_top_level_keywords(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut depth = 0_i32;
    let mut out = Vec::new();

    while i < bytes.len() {
        let b = bytes[i];
        if i + 1 < bytes.len() && b == b'-' && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        if b == b'\'' {
            i = skip_single_quote(bytes, i);
            continue;
        }
        if b == b'"' {
            i = skip_double_quote(bytes, i);
            continue;
        }
        if b == b'`' {
            i = skip_backtick_quote(bytes, i);
            continue;
        }
        if b == b'(' {
            depth += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            depth = (depth - 1).max(0);
            i += 1;
            continue;
        }
        if depth == 0 && (b.is_ascii_alphabetic() || b == b'_') {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push(sql[start..i].to_ascii_lowercase());
            continue;
        }
        i += 1;
    }

    out
}

fn has_top_level_limit(sql: &str) -> bool {
    fn is_reserved_after_limit(word: &str) -> bool {
        matches!(
            word,
            "from"
                | "where"
                | "group"
                | "having"
                | "order"
                | "union"
                | "intersect"
                | "except"
                | "join"
                | "left"
                | "right"
                | "inner"
                | "outer"
                | "cross"
                | "on"
                | "as"
                | "asc"
                | "desc"
                | "limit"
                | "offset"
                | "fetch"
        )
    }

    fn next_non_comment_token(bytes: &[u8], mut i: usize) -> Option<(bool, String)> {
        while i < bytes.len() {
            let b = bytes[i];
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }
            if i + 1 < bytes.len() && b == b'-' && bytes[i + 1] == b'-' {
                i = skip_line_comment(bytes, i);
                continue;
            }
            if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
                i = skip_block_comment(bytes, i);
                continue;
            }

            if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                return Some((true, String::from_utf8_lossy(&bytes[start..i]).to_ascii_lowercase()));
            }

            if b.is_ascii_digit() {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                return Some((false, String::from_utf8_lossy(&bytes[start..i]).to_string()));
            }

            return Some((false, (b as char).to_string()));
        }

        None
    }

    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut depth = 0_i32;

    while i < bytes.len() {
        let b = bytes[i];
        if i + 1 < bytes.len() && b == b'-' && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && b == b'/' && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        if b == b'\'' {
            i = skip_single_quote(bytes, i);
            continue;
        }
        if b == b'"' {
            i = skip_double_quote(bytes, i);
            continue;
        }
        if b == b'`' {
            i = skip_backtick_quote(bytes, i);
            continue;
        }
        if b == b'(' {
            depth += 1;
            i += 1;
            continue;
        }
        if b == b')' {
            depth = (depth - 1).max(0);
            i += 1;
            continue;
        }

        if depth == 0 && (b.is_ascii_alphabetic() || b == b'_') {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }

            if sql[start..i].eq_ignore_ascii_case("limit") {
                if let Some((is_word, token)) = next_non_comment_token(bytes, i) {
                    if is_word {
                        if !is_reserved_after_limit(&token) {
                            return true;
                        }
                    } else {
                        let ch = token.as_bytes()[0];
                        if ch.is_ascii_digit()
                            || matches!(ch, b'?' | b':' | b'$' | b'@' | b'(' | b'+' | b'-')
                        {
                            return true;
                        }
                    }
                }
            }
            continue;
        }

        i += 1;
    }

    false
}

fn has_top_level_fetch_first_next_rows_only(sql: &str) -> bool {
    let tokens = collect_top_level_keywords(sql);
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == "fetch"
            && i + 1 < tokens.len()
            && (tokens[i + 1] == "first" || tokens[i + 1] == "next")
        {
            let mut j = i + 2;
            while j < tokens.len() {
                if tokens[j] == "only" {
                    return true;
                }
                if tokens[j] == "row" || tokens[j] == "rows" {
                    j += 1;
                    continue;
                }
                if tokens[j] == "offset" || tokens[j] == "limit" {
                    break;
                }
                j += 1;
            }
        }
        i += 1;
    }
    false
}

fn append_limit_1000(sql: &str) -> String {
    let mut trimmed = sql.trim_end();
    let had_semicolon = trimmed.ends_with(';');
    if had_semicolon {
        trimmed = trimmed.trim_end_matches(';').trim_end();
    }

    if had_semicolon {
        format!("{trimmed} LIMIT {DEFAULT_SELECT_LIMIT};")
    } else {
        format!("{trimmed} LIMIT {DEFAULT_SELECT_LIMIT}")
    }
}

fn append_mssql_fetch_1000(sql: &str) -> String {
    let mut trimmed = sql.trim_end();
    let had_semicolon = trimmed.ends_with(';');
    if had_semicolon {
        trimmed = trimmed.trim_end_matches(';').trim_end();
    }
    let has_order_by = collect_top_level_keywords(trimmed)
        .windows(2)
        .any(|pair| pair[0] == "order" && pair[1] == "by");

    let with_fetch = if has_order_by {
        format!(
            "{trimmed} OFFSET 0 ROWS FETCH NEXT {DEFAULT_SELECT_LIMIT} ROWS ONLY"
        )
    } else {
        format!(
            "{trimmed} ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH NEXT {DEFAULT_SELECT_LIMIT} ROWS ONLY"
        )
    };

    if had_semicolon {
        format!("{with_fetch};")
    } else {
        with_fetch
    }
}

fn has_top_level_mssql_top(sql: &str) -> bool {
    let tokens = collect_top_level_keywords(sql);
    if tokens.first().map(|s| s.as_str()) == Some("select") {
        return tokens.iter().skip(1).take(3).any(|t| t == "top");
    }
    false
}

fn maybe_apply_default_limit(sql: &str, driver: Option<&str>) -> String {
    let normalized = normalize_for_guard(sql);
    if normalized.is_empty() {
        return sql.to_string();
    }
    if !is_single_statement(normalized) {
        return sql.to_string();
    }
    if statement_kind_for_limit_guard(normalized) != Some("select") {
        return sql.to_string();
    }
    if has_top_level_limit(normalized) {
        return sql.to_string();
    }
    if has_top_level_fetch_first_next_rows_only(normalized) {
        return sql.to_string();
    }

    if driver
        .map(|d| d.eq_ignore_ascii_case("mssql"))
        .unwrap_or(false)
    {
        if has_top_level_mssql_top(normalized) {
            return sql.to_string();
        }
        return append_mssql_fetch_1000(normalized);
    }

    append_limit_1000(normalized)
}

async fn resolve_driver(state: &State<'_, AppState>, id: i64) -> Option<String> {
    let db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    }?;
    db.get_connection_form_by_id(id).await.ok().map(|f| f.driver)
}

async fn append_sql_execution_log(
    state: &State<'_, AppState>,
    sql: String,
    source: Option<String>,
    connection_id: Option<i64>,
    database: Option<String>,
    success: bool,
    error: Option<String>,
) {
    let db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };

    if let Some(local_db) = db {
        if let Err(e) = local_db
            .insert_sql_execution_log(sql, source, connection_id, database, success, error)
            .await
        {
            eprintln!("[SQL_LOG_APPEND_ERROR] {}", e);
        }
    }
}

#[tauri::command]
pub async fn get_table_data_by_conn(
    form: ConnectionForm,
    schema: String,
    table: String,
    page: i64,
    limit: i64,
) -> Result<TableDataResponse, String> {
    let driver = crate::db::drivers::connect(&form).await?;
    driver
        .get_table_data(schema, table, page, limit, None, None, None, None)
        .await
}

#[tauri::command]
pub async fn execute_query(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
    query: String,
    database: Option<String>,
    source: Option<String>,
) -> Result<QueryResult, String> {
    let query_id = format!("q-{}", id);
    let _ = app_handle.emit(
        "query.progress",
        serde_json::json!({"queryId": query_id, "phase": "prepare"}),
    );
    let driver = resolve_driver(&state, id).await;
    let guarded_query = maybe_apply_default_limit(&query, driver.as_deref());

    let result = super::execute_with_retry(&state, id, database.clone(), |driver| {
        let query_clone = guarded_query.clone();
        async move { driver.execute_query(query_clone).await }
    })
    .await;

    if let Ok(res) = &result {
        // Stream first chunk for UX (simulated)
        if !res.data.is_empty() {
            let _ = app_handle.emit(
                "query.chunk",
                serde_json::json!({
                    "queryId": query_id,
                    "rows": res.data.iter().take(50).collect::<Vec<_>>()
                }),
            );
        }

        append_sql_execution_log(
            &state,
            guarded_query.clone(),
            source,
            Some(id),
            database,
            true,
            None,
        )
        .await;
    } else if let Err(err) = &result {
        append_sql_execution_log(
            &state,
            guarded_query.clone(),
            source,
            Some(id),
            database,
            false,
            Some(err.clone()),
        )
        .await;
    }

    result
}

#[tauri::command]
pub async fn get_table_data(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    schema: String,
    table: String,
    page: i64,
    limit: i64,
    filter: Option<String>,
    sort_column: Option<String>,
    sort_direction: Option<String>,
    order_by: Option<String>,
) -> Result<TableDataResponse, String> {
    super::execute_with_retry(&state, id, database, |driver| {
        let schema_clone = schema.clone();
        let table_clone = table.clone();
        let filter_clone = filter.clone();
        let sort_col_clone = sort_column.clone();
        let sort_dir_clone = sort_direction.clone();
        let order_by_clone = order_by.clone();
        async move {
            driver
                .get_table_data(
                    schema_clone,
                    table_clone,
                    page,
                    limit,
                    sort_col_clone,
                    sort_dir_clone,
                    filter_clone,
                    order_by_clone,
                )
                .await
        }
    })
    .await
}

#[tauri::command]
pub async fn cancel_query(_uuid: String, _query_id: String) -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub async fn execute_by_conn(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    form: ConnectionForm,
    sql: String,
) -> Result<QueryResult, String> {
    let query_id = "q-conn-ephemeral";
    let _ = app_handle.emit(
        "query.progress",
        serde_json::json!({"queryId": query_id, "phase": "prepare"}),
    );
    let guarded_sql = maybe_apply_default_limit(&sql, Some(&form.driver));

    let database = form.database.clone();
    let driver = crate::db::drivers::connect(&form).await?;
    let result = driver.execute_query(guarded_sql.clone()).await;

    if let Ok(res) = &result {
        if !res.data.is_empty() {
            let _ = app_handle.emit(
                "query.chunk",
                serde_json::json!({
                    "queryId": query_id,
                    "rows": res.data.iter().take(50).collect::<Vec<_>>()
                }),
            );
        }

        append_sql_execution_log(
            &state,
            guarded_sql.clone(),
            Some("execute_by_conn".to_string()),
            None,
            database,
            true,
            None,
        )
        .await;
    } else if let Err(err) = &result {
        append_sql_execution_log(
            &state,
            guarded_sql.clone(),
            Some("execute_by_conn".to_string()),
            None,
            database,
            false,
            Some(err.clone()),
        )
        .await;
    }
    result
}

fn clamp_sql_execution_logs_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(100).clamp(1, 100)
}

#[tauri::command]
pub async fn list_sql_execution_logs(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<SqlExecutionLog>, String> {
    let safe_limit = clamp_sql_execution_logs_limit(limit);
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };

    if let Some(db) = local_db {
        db.list_sql_execution_logs(safe_limit).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_sql_execution_logs_limit, maybe_apply_default_limit};

    #[test]
    fn adds_limit_to_simple_select() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t", None),
            "SELECT * FROM t LIMIT 1000"
        );
    }

    #[test]
    fn keeps_existing_limit() {
        assert_eq!(
            maybe_apply_default_limit("select * from t limit 10", None),
            "select * from t limit 10"
        );
    }

    #[test]
    fn ignores_limit_column_name() {
        assert_eq!(
            maybe_apply_default_limit("SELECT limit FROM t", None),
            "SELECT limit FROM t LIMIT 1000"
        );
    }

    #[test]
    fn ignores_limit_alias() {
        assert_eq!(
            maybe_apply_default_limit("SELECT a AS limit FROM t", None),
            "SELECT a AS limit FROM t LIMIT 1000"
        );
    }

    #[test]
    fn ignores_limit_identifier_in_where() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t WHERE limit > 10", None),
            "SELECT * FROM t WHERE limit > 10 LIMIT 1000"
        );
    }

    #[test]
    fn keeps_fetch_first_rows_only() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t FETCH FIRST 20 ROWS ONLY", None),
            "SELECT * FROM t FETCH FIRST 20 ROWS ONLY"
        );
    }

    #[test]
    fn supports_leading_comment() {
        assert_eq!(
            maybe_apply_default_limit("-- c\nSELECT * FROM t", None),
            "-- c\nSELECT * FROM t LIMIT 1000"
        );
    }

    #[test]
    fn ignores_subquery_limit() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM (SELECT * FROM t LIMIT 5) s", None),
            "SELECT * FROM (SELECT * FROM t LIMIT 5) s LIMIT 1000"
        );
    }

    #[test]
    fn preserves_trailing_semicolon() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t;", None),
            "SELECT * FROM t LIMIT 1000;"
        );
    }

    #[test]
    fn skips_multi_statement_sql() {
        assert_eq!(
            maybe_apply_default_limit("SELECT 1; SELECT 2;", None),
            "SELECT 1; SELECT 2;"
        );
    }

    #[test]
    fn applies_to_with_select_queries() {
        assert_eq!(
            maybe_apply_default_limit("WITH cte AS (SELECT 1) SELECT * FROM cte", None),
            "WITH cte AS (SELECT 1) SELECT * FROM cte LIMIT 1000"
        );
    }

    #[test]
    fn skips_with_non_select_queries() {
        assert_eq!(
            maybe_apply_default_limit("WITH cte AS (SELECT 1) INSERT INTO t SELECT * FROM cte", None),
            "WITH cte AS (SELECT 1) INSERT INTO t SELECT * FROM cte"
        );
    }

    #[test]
    fn ignores_limit_inside_string_literal() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t WHERE name = 'limit x'", None),
            "SELECT * FROM t WHERE name = 'limit x' LIMIT 1000"
        );
    }

    #[test]
    fn mssql_adds_fetch_with_default_order() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t", Some("mssql")),
            "SELECT * FROM t ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH NEXT 1000 ROWS ONLY"
        );
    }

    #[test]
    fn mssql_adds_fetch_with_existing_order() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t ORDER BY id DESC", Some("mssql")),
            "SELECT * FROM t ORDER BY id DESC OFFSET 0 ROWS FETCH NEXT 1000 ROWS ONLY"
        );
    }

    #[test]
    fn mssql_keeps_existing_top() {
        assert_eq!(
            maybe_apply_default_limit("SELECT TOP 20 * FROM t", Some("mssql")),
            "SELECT TOP 20 * FROM t"
        );
    }

    #[test]
    fn sql_logs_limit_defaults_to_100() {
        assert_eq!(clamp_sql_execution_logs_limit(None), 100);
    }

    #[test]
    fn sql_logs_limit_clamps_lower_bound() {
        assert_eq!(clamp_sql_execution_logs_limit(Some(0)), 1);
        assert_eq!(clamp_sql_execution_logs_limit(Some(-5)), 1);
    }

    #[test]
    fn sql_logs_limit_clamps_upper_bound() {
        assert_eq!(clamp_sql_execution_logs_limit(Some(101)), 100);
        assert_eq!(clamp_sql_execution_logs_limit(Some(9999)), 100);
    }
}
