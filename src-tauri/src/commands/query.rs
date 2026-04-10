use crate::models::{ConnectionForm, QueryResult, SqlExecutionLog, TableDataResponse};
use crate::state::AppState;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use tauri::{Emitter, State};
use tokio::sync::Mutex;

const DEFAULT_SELECT_LIMIT: i64 = 1000;
type RunningQueryRegistry = HashMap<i64, HashSet<String>>;

fn running_queries() -> &'static Mutex<RunningQueryRegistry> {
    static RUNNING_QUERIES: OnceLock<Mutex<RunningQueryRegistry>> = OnceLock::new();
    RUNNING_QUERIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_query_id(connection_id: i64, provided: Option<String>) -> String {
    if let Some(id) = provided {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("q-{}-{}", connection_id, ts)
}

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

fn parse_dollar_quote_tag(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'$') {
        return None;
    }
    let mut i = start + 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    if bytes.get(i) == Some(&b'$') {
        Some(i)
    } else {
        None
    }
}

fn skip_dollar_quote(bytes: &[u8], start: usize) -> usize {
    let Some(tag_end) = parse_dollar_quote_tag(bytes, start) else {
        return start + 1;
    };
    let tag = &bytes[start..=tag_end];
    let tag_len = tag.len();
    let mut i = tag_end + 1;

    while i + tag_len <= bytes.len() {
        if &bytes[i..i + tag_len] == tag {
            return i + tag_len;
        }
        i += 1;
    }

    bytes.len()
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
        if b == b'$' {
            let next = skip_dollar_quote(bytes, i);
            if next != i + 1 {
                i = next;
                continue;
            }
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
        if b == b'$' {
            let next = skip_dollar_quote(bytes, i);
            if next != i + 1 {
                i = next;
                continue;
            }
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
                return Some((
                    true,
                    String::from_utf8_lossy(&bytes[start..i]).to_ascii_lowercase(),
                ));
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
        if b == b'$' {
            let next = skip_dollar_quote(bytes, i);
            if next != i + 1 {
                i = next;
                continue;
            }
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
    let has_offset_clause = has_top_level_mssql_offset_clause(trimmed);

    let with_fetch = if has_offset_clause {
        format!("{trimmed} FETCH NEXT {DEFAULT_SELECT_LIMIT} ROWS ONLY")
    } else if has_order_by {
        format!("{trimmed} OFFSET 0 ROWS FETCH NEXT {DEFAULT_SELECT_LIMIT} ROWS ONLY")
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

fn has_top_level_mssql_offset_clause(sql: &str) -> bool {
    let tokens = collect_top_level_keywords(sql);
    let mut order_by_seen = false;
    let mut i = 0;

    while i < tokens.len() {
        if i + 1 < tokens.len() && tokens[i] == "order" && tokens[i + 1] == "by" {
            order_by_seen = true;
            i += 2;
            continue;
        }

        if order_by_seen
            && i + 1 < tokens.len()
            && tokens[i] == "offset"
            && (tokens[i + 1] == "row" || tokens[i + 1] == "rows")
        {
            return true;
        }

        i += 1;
    }

    false
}

fn has_top_level_mssql_top(sql: &str) -> bool {
    let tokens = collect_top_level_keywords(sql);
    if tokens.first().map(|s| s.as_str()) == Some("select") {
        return tokens.iter().skip(1).take(3).any(|t| t == "top");
    }
    false
}

fn has_top_level_clickhouse_format_clause(sql: &str) -> bool {
    let tokens = collect_top_level_keywords(sql);
    tokens
        .iter()
        .enumerate()
        .any(|(idx, token)| token == "format" && idx + 1 < tokens.len() && idx + 3 >= tokens.len())
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
        .map(|d| d.eq_ignore_ascii_case("clickhouse"))
        .unwrap_or(false)
        && has_top_level_clickhouse_format_clause(normalized)
    {
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
    db.get_connection_form_by_id(id)
        .await
        .ok()
        .map(|f| f.driver)
}

async fn register_running_query(connection_id: i64, query_id: &str) {
    let mut guard = running_queries().lock().await;
    guard
        .entry(connection_id)
        .or_default()
        .insert(query_id.to_string());
}

async fn unregister_running_query(connection_id: i64, query_id: &str) {
    let mut guard = running_queries().lock().await;
    if let Some(ids) = guard.get_mut(&connection_id) {
        ids.remove(query_id);
        if ids.is_empty() {
            guard.remove(&connection_id);
        }
    }
}

async fn is_running_query(connection_id: i64, query_id: &str) -> bool {
    let guard = running_queries().lock().await;
    guard
        .get(&connection_id)
        .map(|ids| ids.contains(query_id))
        .unwrap_or(false)
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

async fn append_sql_execution_log_direct(
    state: &AppState,
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

fn validate_page_limit(page: i64, limit: i64) -> Result<(), String> {
    if page <= 0 {
        return Err("[VALIDATION_ERROR] page must be greater than 0".to_string());
    }
    if limit <= 0 {
        return Err("[VALIDATION_ERROR] limit must be greater than 0".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_table_data_by_conn(
    form: ConnectionForm,
    schema: String,
    table: String,
    page: i64,
    limit: i64,
) -> Result<TableDataResponse, String> {
    validate_page_limit(page, limit)?;
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
    query_id: Option<String>,
) -> Result<QueryResult, String> {
    let query_id = make_query_id(id, query_id);
    let _ = app_handle.emit(
        "query.progress",
        serde_json::json!({"queryId": query_id.clone(), "phase": "prepare"}),
    );
    let driver = resolve_driver(&state, id).await;
    let is_clickhouse = driver
        .as_deref()
        .map(|d| d.eq_ignore_ascii_case("clickhouse"))
        .unwrap_or(false);
    let guarded_query = maybe_apply_default_limit(&query, driver.as_deref());
    if is_clickhouse {
        register_running_query(id, &query_id).await;
    }

    let result = super::execute_with_retry(&state, id, database.clone(), |driver| {
        let query_clone = guarded_query.clone();
        let query_id_clone = query_id.clone();
        async move {
            driver
                .execute_query_with_id(
                    query_clone,
                    if is_clickhouse {
                        Some(query_id_clone.as_str())
                    } else {
                        None
                    },
                )
                .await
        }
    })
    .await;
    if is_clickhouse {
        unregister_running_query(id, &query_id).await;
    }

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

async fn resolve_driver_from_app_state(state: &AppState, id: i64) -> Option<String> {
    let db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    }?;
    db.get_connection_form_by_id(id)
        .await
        .ok()
        .map(|f| f.driver)
}

pub async fn execute_query_by_id_direct(
    state: &AppState,
    id: i64,
    query: String,
    database: Option<String>,
    source: Option<String>,
    query_id: Option<String>,
) -> Result<QueryResult, String> {
    let query_id = make_query_id(id, query_id);
    let driver = resolve_driver_from_app_state(state, id).await;
    let is_clickhouse = driver
        .as_deref()
        .map(|d| d.eq_ignore_ascii_case("clickhouse"))
        .unwrap_or(false);
    let guarded_query = maybe_apply_default_limit(&query, driver.as_deref());
    if is_clickhouse {
        register_running_query(id, &query_id).await;
    }

    let result = super::execute_with_retry_from_app_state(state, id, database.clone(), |driver| {
        let query_clone = guarded_query.clone();
        let query_id_clone = query_id.clone();
        async move {
            driver
                .execute_query_with_id(
                    query_clone,
                    if is_clickhouse {
                        Some(query_id_clone.as_str())
                    } else {
                        None
                    },
                )
                .await
        }
    })
    .await;
    if is_clickhouse {
        unregister_running_query(id, &query_id).await;
    }

    if result.is_ok() {
        append_sql_execution_log_direct(
            state,
            guarded_query.clone(),
            source,
            Some(id),
            database,
            true,
            None,
        )
        .await;
    } else if let Err(err) = &result {
        append_sql_execution_log_direct(
            state,
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

pub async fn execute_by_conn_direct(
    form: ConnectionForm,
    sql: String,
) -> Result<QueryResult, String> {
    let guarded_sql = maybe_apply_default_limit(&sql, Some(&form.driver));
    let driver = crate::db::drivers::connect(&form).await?;
    driver.execute_query_with_id(guarded_sql, None).await
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
    validate_page_limit(page, limit)?;
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
pub async fn cancel_query(
    state: State<'_, AppState>,
    uuid: String,
    query_id: String,
) -> Result<bool, String> {
    let connection_id = uuid
        .trim()
        .parse::<i64>()
        .map_err(|_| "[VALIDATION_ERROR] Invalid connection id for cancellation".to_string())?;
    let query_id = query_id.trim().to_string();
    if query_id.is_empty() {
        return Err("[VALIDATION_ERROR] query_id cannot be empty".to_string());
    }
    if !is_running_query(connection_id, &query_id).await {
        return Ok(false);
    }

    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or("Local DB not initialized".to_string())?;
    let form = db.get_connection_form_by_id(connection_id).await?;
    if !form.driver.eq_ignore_ascii_case("clickhouse") {
        return Ok(false);
    }

    let driver = crate::db::drivers::clickhouse::ClickHouseDriver::connect(&form).await?;
    driver.kill_query(&query_id).await?;
    unregister_running_query(connection_id, &query_id).await;
    Ok(true)
}

#[tauri::command]
pub async fn execute_by_conn(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    form: ConnectionForm,
    sql: String,
) -> Result<QueryResult, String> {
    let query_id = make_query_id(-1, None);
    let _ = app_handle.emit(
        "query.progress",
        serde_json::json!({"queryId": query_id.clone(), "phase": "prepare"}),
    );
    let guarded_sql = maybe_apply_default_limit(&sql, Some(&form.driver));

    let database = form.database.clone();
    let driver = crate::db::drivers::connect(&form).await?;
    let result = driver
        .execute_query_with_id(
            guarded_sql.clone(),
            if form.driver.eq_ignore_ascii_case("clickhouse") {
                Some(query_id.as_str())
            } else {
                None
            },
        )
        .await;

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

pub async fn list_sql_execution_logs_direct(
    state: &AppState,
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

pub async fn cancel_query_direct(
    state: &AppState,
    uuid: String,
    query_id: String,
) -> Result<bool, String> {
    let connection_id = uuid
        .trim()
        .parse::<i64>()
        .map_err(|_| "[VALIDATION_ERROR] Invalid connection id for cancellation".to_string())?;
    let query_id = query_id.trim().to_string();
    if query_id.is_empty() {
        return Err("[VALIDATION_ERROR] query_id cannot be empty".to_string());
    }
    if !is_running_query(connection_id, &query_id).await {
        return Ok(false);
    }

    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or("Local DB not initialized".to_string())?;
    let form = db.get_connection_form_by_id(connection_id).await?;
    if !form.driver.eq_ignore_ascii_case("clickhouse") {
        return Ok(false);
    }

    let driver = crate::db::drivers::clickhouse::ClickHouseDriver::connect(&form).await?;
    driver.kill_query(&query_id).await?;
    unregister_running_query(connection_id, &query_id).await;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_sql_execution_logs_limit, collect_top_level_keywords, is_single_statement,
        make_query_id, maybe_apply_default_limit, statement_kind_for_limit_guard,
    };

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
            maybe_apply_default_limit(
                "WITH cte AS (SELECT 1) INSERT INTO t SELECT * FROM cte",
                None
            ),
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
    fn clickhouse_skips_default_limit_when_format_clause_exists() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t FORMAT JSON", Some("clickhouse")),
            "SELECT * FROM t FORMAT JSON"
        );
    }

    #[test]
    fn clickhouse_keeps_default_limit_for_regular_select() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t", Some("clickhouse")),
            "SELECT * FROM t LIMIT 1000"
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
    fn mssql_adds_fetch_to_existing_offset_clause() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t ORDER BY id OFFSET 10 ROWS", Some("mssql")),
            "SELECT * FROM t ORDER BY id OFFSET 10 ROWS FETCH NEXT 1000 ROWS ONLY"
        );
    }

    #[test]
    fn mssql_adds_fetch_to_existing_offset_clause_with_semicolon() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t ORDER BY id OFFSET 10 ROWS;", Some("mssql")),
            "SELECT * FROM t ORDER BY id OFFSET 10 ROWS FETCH NEXT 1000 ROWS ONLY;"
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

    #[test]
    fn is_single_statement_handles_comments_and_quotes() {
        assert!(is_single_statement("SELECT 1 -- comment\n"));
        assert!(is_single_statement("SELECT 'a; b'"));
        assert!(is_single_statement("SELECT \"a; b\""));
        assert!(is_single_statement("SELECT `a; b`"));
        assert!(is_single_statement(
            "CREATE FUNCTION f() RETURNS void AS $$ BEGIN PERFORM 1; END; $$ LANGUAGE plpgsql;"
        ));
        assert!(is_single_statement(
            "CREATE FUNCTION f() RETURNS text AS $tag$ BEGIN RETURN ';'; END; $tag$ LANGUAGE plpgsql;"
        ));
        assert!(!is_single_statement("SELECT 1; SELECT 2"));
    }

    #[test]
    fn is_single_statement_handles_nested_parens_and_unbalanced() {
        assert!(is_single_statement("SELECT (SELECT 1)"));
        assert!(!is_single_statement("SELECT (1;"));
        assert!(!is_single_statement("SELECT 1)"));
    }

    #[test]
    fn collect_top_level_keywords_skips_subqueries_and_strings() {
        let tokens =
            collect_top_level_keywords("WITH cte AS (SELECT 'from' AS v) SELECT * FROM cte");
        assert_eq!(tokens.first().map(String::as_str), Some("with"));
        assert!(tokens.contains(&"select".to_string()));
        assert!(tokens.contains(&"from".to_string()));
    }

    #[test]
    fn collect_top_level_keywords_skips_dollar_quoted_bodies() {
        let tokens = collect_top_level_keywords(
            "CREATE FUNCTION f() RETURNS void AS $$ BEGIN SELECT 1; END; $$ LANGUAGE plpgsql",
        );
        assert_eq!(tokens.first().map(String::as_str), Some("create"));
        assert!(tokens.contains(&"function".to_string()));
        assert!(!tokens
            .iter()
            .any(|token| token == "begin" || token == "end"));
    }

    #[test]
    fn statement_kind_for_limit_guard_classifies_with_queries() {
        assert_eq!(
            statement_kind_for_limit_guard("WITH c AS (SELECT 1) SELECT * FROM c"),
            Some("select")
        );
        assert_eq!(
            statement_kind_for_limit_guard("WITH c AS (SELECT 1) UPDATE t SET a = 1"),
            Some("non_select")
        );
    }

    #[test]
    fn make_query_id_uses_provided_and_falls_back() {
        assert_eq!(
            make_query_id(42, Some(" custom-id ".to_string())),
            "custom-id"
        );

        let generated = make_query_id(7, Some("   ".to_string()));
        assert!(generated.starts_with("q-7-"));
    }
}
