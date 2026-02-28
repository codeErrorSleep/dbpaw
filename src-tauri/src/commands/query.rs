use crate::models::{ConnectionForm, QueryResult, TableDataResponse};
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

fn first_keyword(sql: &str) -> Option<String> {
    let bytes = sql.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b';' {
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
        break;
    }

    if i >= bytes.len() || !bytes[i].is_ascii_alphabetic() {
        return None;
    }

    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    Some(sql[start..i].to_ascii_lowercase())
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
    collect_top_level_keywords(sql).iter().any(|t| t == "limit")
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

fn maybe_apply_default_limit(sql: &str) -> String {
    let normalized = normalize_for_guard(sql);
    if normalized.is_empty() {
        return sql.to_string();
    }
    if !is_single_statement(normalized) {
        return sql.to_string();
    }
    if first_keyword(normalized).as_deref() != Some("select") {
        return sql.to_string();
    }
    if has_top_level_limit(normalized) {
        return sql.to_string();
    }
    if has_top_level_fetch_first_next_rows_only(normalized) {
        return sql.to_string();
    }

    append_limit_1000(normalized)
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
) -> Result<QueryResult, String> {
    let query_id = format!("q-{}", id);
    let _ = app_handle.emit(
        "query.progress",
        serde_json::json!({"queryId": query_id, "phase": "prepare"}),
    );
    let guarded_query = maybe_apply_default_limit(&query);

    let result = super::execute_with_retry(&state, id, database, |driver| {
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
    form: ConnectionForm,
    sql: String,
) -> Result<QueryResult, String> {
    let query_id = "q-conn-ephemeral";
    let _ = app_handle.emit(
        "query.progress",
        serde_json::json!({"queryId": query_id, "phase": "prepare"}),
    );
    let guarded_sql = maybe_apply_default_limit(&sql);

    let driver = crate::db::drivers::connect(&form).await?;
    let result = driver.execute_query(guarded_sql).await;

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
    }
    result
}

#[cfg(test)]
mod tests {
    use super::maybe_apply_default_limit;

    #[test]
    fn adds_limit_to_simple_select() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t"),
            "SELECT * FROM t LIMIT 1000"
        );
    }

    #[test]
    fn keeps_existing_limit() {
        assert_eq!(
            maybe_apply_default_limit("select * from t limit 10"),
            "select * from t limit 10"
        );
    }

    #[test]
    fn keeps_fetch_first_rows_only() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t FETCH FIRST 20 ROWS ONLY"),
            "SELECT * FROM t FETCH FIRST 20 ROWS ONLY"
        );
    }

    #[test]
    fn supports_leading_comment() {
        assert_eq!(
            maybe_apply_default_limit("-- c\nSELECT * FROM t"),
            "-- c\nSELECT * FROM t LIMIT 1000"
        );
    }

    #[test]
    fn ignores_subquery_limit() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM (SELECT * FROM t LIMIT 5) s"),
            "SELECT * FROM (SELECT * FROM t LIMIT 5) s LIMIT 1000"
        );
    }

    #[test]
    fn preserves_trailing_semicolon() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t;"),
            "SELECT * FROM t LIMIT 1000;"
        );
    }

    #[test]
    fn skips_multi_statement_sql() {
        assert_eq!(
            maybe_apply_default_limit("SELECT 1; SELECT 2;"),
            "SELECT 1; SELECT 2;"
        );
    }

    #[test]
    fn skips_with_queries_conservatively() {
        assert_eq!(
            maybe_apply_default_limit("WITH cte AS (SELECT 1) SELECT * FROM cte"),
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        );
    }

    #[test]
    fn ignores_limit_inside_string_literal() {
        assert_eq!(
            maybe_apply_default_limit("SELECT * FROM t WHERE name = 'limit x'"),
            "SELECT * FROM t WHERE name = 'limit x' LIMIT 1000"
        );
    }
}
