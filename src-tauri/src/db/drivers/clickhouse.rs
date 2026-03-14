use super::DatabaseDriver;
use crate::models::{
    ClickHouseTableExtra, ColumnInfo, ColumnSchema, ConnectionForm, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::ssh::SshTunnel;

pub struct ClickHouseDriver {
    pub client: reqwest::Client,
    pub base_url: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssh_tunnel: Option<SshTunnel>,
}

#[derive(Debug)]
struct ClickHouseConfig {
    base_url: String,
    database: String,
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ClickHouseMeta {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
}

#[derive(Debug, Deserialize)]
struct ClickHouseJsonResponse {
    #[serde(default)]
    meta: Vec<ClickHouseMeta>,
    #[serde(default)]
    data: Vec<Value>,
    rows: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct ClickHouseSummary {
    read_rows: Option<i64>,
    written_rows: Option<i64>,
    result_rows: Option<i64>,
    total_rows_to_read: Option<i64>,
}

#[derive(Debug)]
struct ClickHouseRawResponse {
    body: String,
    summary: Option<ClickHouseSummary>,
}

fn build_config(form: &ConnectionForm) -> Result<ClickHouseConfig, String> {
    let host = form
        .host
        .clone()
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] host cannot be empty")?;

    let ssl = form.ssl.unwrap_or(false);
    let scheme = if ssl { "https" } else { "http" };
    let port = form.port.unwrap_or(8123);
    let database = form
        .database
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let username = form
        .username
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let password = form.password.clone().unwrap_or_default();

    Ok(ClickHouseConfig {
        base_url: format!("{}://{}:{}", scheme, host, port),
        database,
        username,
        password,
    })
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    loop {
        while i < len && (bytes[i].is_ascii_whitespace() || bytes[i] == b';') {
            i += 1;
        }

        if i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= len {
                return None;
            }
            i += 2;
            continue;
        }

        break;
    }

    if i >= len {
        return None;
    }

    let start = i;
    while i < len && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if start == i {
        return None;
    }

    Some(sql[start..i].to_ascii_lowercase())
}

fn quote_ident(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn table_ref(schema: &str, table: &str) -> String {
    let schema = schema.trim();
    if schema.is_empty() {
        quote_ident(table)
    } else {
        format!("{}.{}", quote_ident(schema), quote_ident(table))
    }
}

fn trim_trailing_semicolon(sql: &str) -> &str {
    let trimmed = sql.trim();
    trimmed.trim_end_matches(';').trim_end()
}

fn has_format_clause(sql: &str) -> bool {
    trim_trailing_semicolon(sql)
        .to_ascii_lowercase()
        .contains(" format ")
}

fn is_json_format(sql: &str) -> bool {
    let lower = trim_trailing_semicolon(sql).to_ascii_lowercase();
    // Split by "format" and check if the part after it starts with whitespace + "json"
    if let Some(pos) = lower.find("format") {
        let before = &lower[..pos];
        // Ensure "format" is a separate word (preceded by whitespace or at start)
        if !before.is_empty() && !before.ends_with(|c: char| c.is_ascii_whitespace()) {
            return false;
        }
        let after = &lower[pos + 6..];
        // Must have whitespace after "format", then "json" as a separate word
        let after_trimmed = after.trim_start();
        after_trimmed.starts_with("json") && {
            let after_json = &after_trimmed[4..];
            after_json.is_empty() || after_json.starts_with(|c: char| c.is_ascii_whitespace())
        }
    } else {
        false
    }
}

fn ensure_json_format(sql: &str) -> String {
    if has_format_clause(sql) {
        trim_trailing_semicolon(sql).to_string()
    } else {
        format!("{} FORMAT JSON", trim_trailing_semicolon(sql))
    }
}

fn value_to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(s) => {
            let s = s.trim().to_ascii_lowercase();
            s == "1" || s == "true" || s == "yes"
        }
        _ => false,
    }
}

fn value_to_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b {
            "true".to_string()
        } else {
            "false".to_string()
        }),
        _ => None,
    }
}

fn required_i64_from_json_row(
    row: Option<&Value>,
    key: &str,
    context_sql: &str,
) -> Result<i64, String> {
    let value = row
        .and_then(|v| v.get(key))
        .ok_or_else(|| format!("[PARSE_ERROR] Missing '{}' in response for SQL: {}", key, context_sql))?;
    value_to_i64(value).ok_or_else(|| {
        format!(
            "[PARSE_ERROR] Invalid '{}' value {:?} for SQL: {}",
            key, value, context_sql
        )
    })
}

fn parse_summary_header(headers: &reqwest::header::HeaderMap) -> Option<ClickHouseSummary> {
    let header = headers.get("X-ClickHouse-Summary")?;
    let text = header.to_str().ok()?;
    serde_json::from_str::<ClickHouseSummary>(text).ok()
}

fn raw_text_to_query_result(body: String, time_taken_ms: i64) -> QueryResult {
    let trimmed = body.trim_end().to_string();
    if trimmed.is_empty() {
        return QueryResult {
            data: vec![],
            row_count: 0,
            columns: vec![],
            time_taken_ms,
            success: true,
            error: None,
        };
    }

    let mut rows = Vec::new();
    for (idx, line) in trimmed.lines().enumerate() {
        let mut row = serde_json::Map::new();
        row.insert("line_no".to_string(), Value::from((idx + 1) as i64));
        row.insert("raw_line".to_string(), Value::String(line.to_string()));
        rows.push(Value::Object(row));
    }
    QueryResult {
        row_count: rows.len() as i64,
        data: rows,
        columns: vec![
            QueryColumn {
                name: "line_no".to_string(),
                r#type: "Int64".to_string(),
            },
            QueryColumn {
                name: "raw_line".to_string(),
                r#type: "String".to_string(),
            },
        ],
        time_taken_ms,
        success: true,
        error: None,
    }
}

fn normalize_optional_sql_expr(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn extract_ttl_expr(create_table_query: &str) -> Option<String> {
    let lower = create_table_query.to_ascii_lowercase();
    let ttl_idx = lower.find(" ttl ")?;
    let after = &create_table_query[ttl_idx + 5..];
    let mut end = after.len();
    for marker in [" SETTINGS ", " COMMENT ", " PRIMARY KEY ", " ORDER BY "] {
        if let Some(pos) = after.to_ascii_uppercase().find(marker) {
            end = end.min(pos);
        }
    }
    let ttl = after[..end].trim().trim_end_matches(';').trim();
    if ttl.is_empty() {
        None
    } else {
        Some(ttl.to_string())
    }
}

impl ClickHouseDriver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let mut dsn_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            dsn_form.host = Some("127.0.0.1".to_string());
            dsn_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let config = build_config(&dsn_form)?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("[CONN_FAILED] Failed to create HTTP client: {e}"))?;

        Ok(Self {
            client,
            base_url: config.base_url,
            database: config.database,
            username: config.username,
            password: config.password,
            ssh_tunnel,
        })
    }

    async fn execute_raw(
        &self,
        sql: &str,
        query_id: Option<&str>,
    ) -> Result<ClickHouseRawResponse, String> {
        let mut request = self
            .client
            .post(&self.base_url)
            .query(&[("database", self.database.as_str())]);
        if let Some(qid) = query_id.filter(|v| !v.trim().is_empty()) {
            request = request.query(&[("query_id", qid)]);
        }
        let response = request
            .basic_auth(&self.username, Some(&self.password))
            .body(sql.to_string())
            .send()
            .await
            .map_err(|e| format!("[QUERY_ERROR] HTTP request failed: {e}"))?;

        let status = response.status();
        let summary = parse_summary_header(response.headers());
        let body = response
            .text()
            .await
            .map_err(|e| format!("[QUERY_ERROR] Failed to read response body: {e}"))?;

        if !status.is_success() {
            let message = body.trim();
            return Err(format!("[QUERY_ERROR] HTTP {}: {}", status, message));
        }

        Ok(ClickHouseRawResponse { body, summary })
    }

    async fn execute_json(
        &self,
        sql: &str,
        query_id: Option<&str>,
    ) -> Result<ClickHouseJsonResponse, String> {
        let raw = self.execute_raw(sql, query_id).await?;
        let body = raw.body;
        serde_json::from_str::<ClickHouseJsonResponse>(&body).map_err(|e| {
            let snippet = if body.len() > 240 {
                format!("{}...", &body[..240])
            } else {
                body
            };
            format!(
                "[PARSE_ERROR] Failed to parse ClickHouse JSON response: {} | body: {}",
                e, snippet
            )
        })
    }

    async fn estimate_total_rows(&self, schema: &str, table: &str) -> Result<Option<i64>, String> {
        let sql = format!(
            "SELECT total_rows FROM system.tables WHERE database = {} AND name = {} FORMAT JSON",
            quote_literal(schema),
            quote_literal(table)
        );
        let resp = self.execute_json(&sql, None).await?;
        let total = resp
            .data
            .first()
            .and_then(|v| v.get("total_rows"))
            .and_then(value_to_i64);
        Ok(total.filter(|v| *v >= 0))
    }

    async fn query_table_extra(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Option<ClickHouseTableExtra>, String> {
        let sql = format!(
            "SELECT engine, partition_key, sorting_key, primary_key, sampling_key, create_table_query \
             FROM system.tables WHERE database = {} AND name = {} FORMAT JSON",
            quote_literal(schema),
            quote_literal(table)
        );
        let resp = self.execute_json(&sql, None).await?;
        let Some(first) = resp.data.first() else {
            return Ok(None);
        };

        let engine = first
            .get("engine")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if engine.is_empty() {
            return Ok(None);
        }

        let create_table_query = normalize_optional_sql_expr(first.get("create_table_query"));
        let ttl_expr = create_table_query.as_deref().and_then(extract_ttl_expr);

        Ok(Some(ClickHouseTableExtra {
            engine,
            partition_key: normalize_optional_sql_expr(first.get("partition_key")),
            sorting_key: normalize_optional_sql_expr(first.get("sorting_key")),
            primary_key_expr: normalize_optional_sql_expr(first.get("primary_key")),
            sampling_key: normalize_optional_sql_expr(first.get("sampling_key")),
            ttl_expr,
            create_table_query,
        }))
    }

    pub async fn kill_query(&self, query_id: &str) -> Result<(), String> {
        let qid = query_id.trim();
        if qid.is_empty() {
            return Err("[VALIDATION_ERROR] query_id cannot be empty".to_string());
        }
        let sql = format!("KILL QUERY WHERE query_id = {} ASYNC", quote_literal(qid));
        self.execute_raw(&sql, None).await.map(|_| ())
    }
}

#[async_trait]
impl DatabaseDriver for ClickHouseDriver {
    async fn close(&self) {
        // reqwest::Client does not require explicit close.
    }

    async fn test_connection(&self) -> Result<(), String> {
        self.execute_raw("SELECT 1", None).await.map(|_| ())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let resp = self
            .execute_json(
                "SELECT name FROM system.databases ORDER BY name FORMAT JSON",
                None,
            )
            .await?;

        let mut out = Vec::new();
        for row in resp.data {
            if let Some(name) = row.get("name").and_then(Value::as_str) {
                out.push(name.to_string());
            }
        }
        Ok(out)
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let target_schema = schema
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.database.clone());

        let sql = format!(
            "SELECT database, name, engine FROM system.tables WHERE database = {} ORDER BY name FORMAT JSON",
            quote_literal(&target_schema)
        );
        let resp = self.execute_json(&sql, None).await?;

        let mut out = Vec::new();
        for row in resp.data {
            let schema_name = row
                .get("database")
                .and_then(Value::as_str)
                .unwrap_or(target_schema.as_str())
                .to_string();
            let table_name = row
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let table_type = row
                .get("engine")
                .and_then(Value::as_str)
                .unwrap_or("table")
                .to_string();

            if !table_name.is_empty() {
                out.push(TableInfo {
                    schema: schema_name,
                    name: table_name,
                    r#type: table_type,
                });
            }
        }

        Ok(out)
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let columns = self.get_table_metadata(schema, table).await?.columns;
        Ok(TableStructure { columns })
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String> {
        let target_schema = if schema.trim().is_empty() {
            self.database.clone()
        } else {
            schema
        };

        let sql = format!(
            "SELECT name, type, default_expression, comment, is_in_primary_key \
             FROM system.columns \
             WHERE database = {} AND table = {} \
             ORDER BY position FORMAT JSON",
            quote_literal(&target_schema),
            quote_literal(&table)
        );

        let resp = self.execute_json(&sql, None).await?;

        let mut columns = Vec::new();
        for row in resp.data {
            let name = row
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                continue;
            }

            let type_name = row
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let nullable = type_name.starts_with("Nullable(");
            let default_value = row
                .get("default_expression")
                .and_then(Value::as_str)
                .and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });
            let comment = row.get("comment").and_then(Value::as_str).and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });

            let primary_key = row
                .get("is_in_primary_key")
                .map(value_to_bool)
                .unwrap_or(false);

            columns.push(ColumnInfo {
                name,
                r#type: type_name,
                nullable,
                default_value,
                primary_key,
                comment,
            });
        }

        let clickhouse_extra = self.query_table_extra(&target_schema, &table).await?;

        Ok(TableMetadata {
            columns,
            indexes: vec![],
            foreign_keys: vec![],
            clickhouse_extra,
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let target_schema = if schema.trim().is_empty() {
            self.database.clone()
        } else {
            schema
        };
        let sql = format!(
            "SHOW CREATE TABLE {} FORMAT JSON",
            table_ref(&target_schema, &table)
        );
        let resp = self.execute_json(&sql, None).await?;

        if let Some(first) = resp.data.first() {
            for key in ["statement", "create_table_query", "result"] {
                if let Some(v) = first.get(key).and_then(Value::as_str) {
                    return Ok(v.to_string());
                }
            }

            if let Some(obj) = first.as_object() {
                for value in obj.values() {
                    if let Some(text) = value.as_str() {
                        return Ok(text.to_string());
                    }
                }
            }
        }

        Err("[QUERY_ERROR] SHOW CREATE TABLE returned empty result".to_string())
    }

    async fn get_table_data(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String> {
        let start = std::time::Instant::now();

        let target_schema = if schema.trim().is_empty() {
            self.database.clone()
        } else {
            schema
        };
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 10_000);
        let offset = (safe_page - 1) * safe_limit;
        let qualified = table_ref(&target_schema, &table);

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let total = if where_clause.is_empty() {
            match self.estimate_total_rows(&target_schema, &table).await? {
                Some(estimated) => estimated,
                None => {
                    let count_sql = format!(
                        "SELECT count() AS total FROM {}{} FORMAT JSON",
                        qualified, where_clause
                    );
                    let count_resp = self.execute_json(&count_sql, None).await?;
                    required_i64_from_json_row(count_resp.data.first(), "total", &count_sql)?
                }
            }
        } else {
            let count_sql = format!(
                "SELECT count() AS total FROM {}{} FORMAT JSON",
                qualified, where_clause
            );
            let count_resp = self.execute_json(&count_sql, None).await?;
            required_i64_from_json_row(count_resp.data.first(), "total", &count_sql)?
        };

        let order_clause = if let Some(ref ob) = order_by {
            if ob.trim().is_empty() {
                String::new()
            } else {
                format!(" ORDER BY {}", ob.trim())
            }
        } else if let Some(ref col) = sort_column {
            if !col.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err("[VALIDATION_ERROR] Invalid sort column name".to_string());
            }
            let dir = match sort_direction.as_deref() {
                Some("desc") => "DESC",
                _ => "ASC",
            };
            format!(" ORDER BY {} {}", quote_ident(col), dir)
        } else {
            String::new()
        };

        let sql = format!(
            "SELECT * FROM {}{}{} LIMIT {} OFFSET {} FORMAT JSON",
            qualified, where_clause, order_clause, safe_limit, offset
        );
        let resp = self.execute_json(&sql, None).await?;

        let mut rows = Vec::new();
        for row in resp.data {
            match row {
                Value::Object(_) => rows.push(row),
                other => {
                    let mut obj = serde_json::Map::new();
                    obj.insert("value".to_string(), other);
                    rows.push(Value::Object(obj));
                }
            }
        }

        let duration = start.elapsed();
        Ok(TableDataResponse {
            data: rows,
            total,
            page: safe_page,
            limit: safe_limit,
            execution_time_ms: duration.as_millis() as i64,
        })
    }

    async fn get_table_data_chunk(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> Result<TableDataResponse, String> {
        self.get_table_data(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
        )
        .await
    }

    async fn execute_query(&self, sql: String) -> Result<QueryResult, String> {
        self.execute_query_with_id(sql, None).await
    }

    async fn execute_query_with_id(
        &self,
        sql: String,
        query_id: Option<&str>,
    ) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let keyword = first_sql_keyword(&sql);
        let should_fetch_rows = matches!(
            keyword.as_deref(),
            Some("select")
                | Some("show")
                | Some("describe")
                | Some("desc")
                | Some("with")
                | Some("explain")
        );

        if should_fetch_rows {
            if has_format_clause(&sql) && !is_json_format(&sql) {
                let raw = self.execute_raw(&sql, query_id).await?;
                let duration = start.elapsed();
                return Ok(raw_text_to_query_result(
                    raw.body,
                    duration.as_millis() as i64,
                ));
            }

            let query_sql = ensure_json_format(&sql);
            let resp = self.execute_json(&query_sql, query_id).await?;

            let columns = resp
                .meta
                .into_iter()
                .map(|m| QueryColumn {
                    name: m.name,
                    r#type: m.type_name,
                })
                .collect::<Vec<_>>();

            let row_count = resp.rows.unwrap_or(resp.data.len() as u64) as i64;
            let duration = start.elapsed();
            return Ok(QueryResult {
                data: resp.data,
                row_count,
                columns,
                time_taken_ms: duration.as_millis() as i64,
                success: true,
                error: None,
            });
        }

        let raw = self.execute_raw(&sql, query_id).await?;
        let summary = raw.summary.unwrap_or_default();
        let affected_opt = summary
            .written_rows
            .or(summary.result_rows)
            .or(summary.read_rows)
            .or(summary.total_rows_to_read);
        let affected = if let Some(v) = affected_opt {
            v
        } else if raw.body.trim().is_empty() {
            0
        } else if let Ok(v) = raw.body.trim().parse::<i64>() {
            v
        } else {
            let snippet = if raw.body.len() > 200 {
                format!("{}...", &raw.body[..200])
            } else {
                raw.body.clone()
            };
            return Err(format!(
                "[PARSE_ERROR] Unable to determine affected rows from ClickHouse response. body: {}",
                snippet
            ));
        };
        let duration = start.elapsed();
        Ok(QueryResult {
            data: vec![],
            row_count: affected,
            columns: vec![],
            time_taken_ms: duration.as_millis() as i64,
            success: true,
            error: None,
        })
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let base = "SELECT database, table, name, type FROM system.columns";
        let sql = if let Some(s) = schema.filter(|s| !s.trim().is_empty()) {
            format!(
                "{} WHERE database = {} ORDER BY database, table, position FORMAT JSON",
                base,
                quote_literal(&s)
            )
        } else {
            format!("{} ORDER BY database, table, position FORMAT JSON", base)
        };

        let resp = self.execute_json(&sql, None).await?;
        let mut grouped: HashMap<(String, String), Vec<ColumnSchema>> = HashMap::new();

        for row in resp.data {
            let schema_name = row
                .get("database")
                .and_then(value_to_string)
                .unwrap_or_default();
            let table_name = row
                .get("table")
                .and_then(value_to_string)
                .unwrap_or_default();
            let col_name = row
                .get("name")
                .and_then(value_to_string)
                .unwrap_or_default();
            let col_type = row
                .get("type")
                .and_then(value_to_string)
                .unwrap_or_default();

            if schema_name.is_empty() || table_name.is_empty() || col_name.is_empty() {
                continue;
            }

            grouped
                .entry((schema_name, table_name))
                .or_default()
                .push(ColumnSchema {
                    name: col_name,
                    r#type: col_type,
                });
        }

        let mut tables = grouped
            .into_iter()
            .map(|((schema_name, table_name), columns)| TableSchema {
                schema: schema_name,
                name: table_name,
                columns,
            })
            .collect::<Vec<_>>();

        tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));

        Ok(SchemaOverview { tables })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_uses_defaults() {
        let form = ConnectionForm {
            driver: "clickhouse".to_string(),
            host: Some("localhost".to_string()),
            ..Default::default()
        };

        let cfg = build_config(&form).unwrap();
        assert_eq!(cfg.base_url, "http://localhost:8123");
        assert_eq!(cfg.database, "default");
        assert_eq!(cfg.username, "default");
    }

    #[test]
    fn build_config_respects_ssl_and_custom_values() {
        let form = ConnectionForm {
            driver: "clickhouse".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(9440),
            database: Some("analytics".to_string()),
            username: Some("app".to_string()),
            password: Some("secret".to_string()),
            ssl: Some(true),
            ..Default::default()
        };

        let cfg = build_config(&form).unwrap();
        assert_eq!(cfg.base_url, "https://db.internal:9440");
        assert_eq!(cfg.database, "analytics");
        assert_eq!(cfg.username, "app");
        assert_eq!(cfg.password, "secret");
    }

    #[test]
    fn ensure_json_format_appends_only_when_missing() {
        assert_eq!(
            ensure_json_format("SELECT * FROM t"),
            "SELECT * FROM t FORMAT JSON"
        );
        assert_eq!(
            ensure_json_format("SELECT * FROM t FORMAT JSON"),
            "SELECT * FROM t FORMAT JSON"
        );
    }

    #[test]
    fn table_ref_quotes_schema_and_table() {
        assert_eq!(table_ref("analytics", "events"), "`analytics`.`events`");
        assert_eq!(table_ref("", "events"), "`events`");
    }

    #[test]
    fn raw_text_to_query_result_splits_lines() {
        let result = raw_text_to_query_result("a\nb\n".to_string(), 12);
        assert_eq!(result.row_count, 2);
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.data[0]["line_no"], Value::from(1));
        assert_eq!(result.data[0]["raw_line"], Value::String("a".to_string()));
        assert_eq!(result.data[1]["line_no"], Value::from(2));
        assert_eq!(result.data[1]["raw_line"], Value::String("b".to_string()));
    }

    #[test]
    fn required_i64_from_json_row_errors_on_missing_or_invalid() {
        let row = serde_json::json!({ "total": "abc" });
        let invalid = required_i64_from_json_row(Some(&row), "total", "SELECT count()");
        assert!(invalid.is_err());

        let missing = required_i64_from_json_row(None, "total", "SELECT count()");
        assert!(missing.is_err());
    }
}
