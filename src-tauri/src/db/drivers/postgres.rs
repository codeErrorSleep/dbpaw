use super::{strip_trailing_statement_terminator, DatabaseDriver};
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::{postgres::PgPoolOptions, Column, Executor, Row, TypeInfo};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::ssh::SshTunnel;

pub struct PostgresDriver {
    pub pool: sqlx::PgPool,
    pub ssh_tunnel: Option<SshTunnel>,
    pub ca_cert_path: Option<PathBuf>,
}

fn write_temp_cert_file(prefix: &str, pem: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("dbpaw_certs");
    fs::create_dir_all(&dir).map_err(|e| format!("[SSL_CA_WRITE_ERROR] {e}"))?;
    let path = dir.join(format!("{prefix}_{}.pem", uuid::Uuid::new_v4()));
    fs::write(&path, pem).map_err(|e| format!("[SSL_CA_WRITE_ERROR] {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, perm).map_err(|e| format!("[SSL_CA_WRITE_ERROR] {e}"))?;
    }
    Ok(path)
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for b in value.bytes() {
        let is_unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if is_unreserved {
            encoded.push(b as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", b));
        }
    }
    encoded
}

fn build_verify_ca_query_param(ca_path: &Path) -> String {
    format!(
        "?sslmode=verify-ca&sslrootcert={}",
        percent_encode_query_value(&ca_path.to_string_lossy())
    )
}

fn build_dsn_and_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), String> {
    let host = form
        .host
        .clone()
        .ok_or("[VALIDATION_ERROR] host cannot be empty")?;
    let port = form.port.unwrap_or(5432);
    // Allow database to be empty, default to postgres
    let database = form
        .database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());
    let username = form
        .username
        .clone()
        .ok_or("[VALIDATION_ERROR] username cannot be empty")?;
    let password = form
        .password
        .clone()
        .ok_or("[VALIDATION_ERROR] password cannot be empty")?;
    let username = percent_encode_query_value(&username);
    let password = percent_encode_query_value(&password);
    let mut dsn = format!(
        "postgres://{}:{}@{}:{}/{}",
        username, password, host, port, database
    );

    let mut ca_cert_path = None;
    if form.ssl.unwrap_or(false) {
        let ssl_mode = form
            .ssl_mode
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("require");
        if ssl_mode == "verify_ca" {
            let ca_cert = form
                .ssl_ca_cert
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or("[VALIDATION_ERROR] sslCaCert cannot be empty in verify_ca mode")?;
            let ca_path = write_temp_cert_file("pg_ca", ca_cert)?;
            dsn.push_str(&build_verify_ca_query_param(&ca_path));
            ca_cert_path = Some(ca_path);
        } else {
            dsn.push_str("?sslmode=require");
        }
    }

    Ok((dsn, ca_cert_path))
}

#[cfg(test)]
fn build_dsn(form: &ConnectionForm) -> Result<String, String> {
    Ok(build_dsn_and_ca_path(form)?.0)
}

fn build_dsn_with_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), String> {
    build_dsn_and_ca_path(form)
}

fn cleanup_ca_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn build_postgres_json_projection_query(describe_sql: &str) -> String {
    let sanitized_describe_sql = strip_trailing_statement_terminator(describe_sql);
    format!(
        "SELECT to_jsonb(__dbpaw_row) AS __row_json FROM ({}) AS __dbpaw_row",
        sanitized_describe_sql
    )
}

fn cleanup_ca_file_opt(path: Option<&PathBuf>) {
    if let Some(p) = path {
        cleanup_ca_file(p);
    }
}

impl Drop for PostgresDriver {
    fn drop(&mut self) {
        cleanup_ca_file_opt(self.ca_cert_path.as_ref());
    }
}

impl PostgresDriver {
    fn cleanup_ca_file(&self) {
        cleanup_ca_file_opt(self.ca_cert_path.as_ref());
    }

    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let mut dsn_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            dsn_form.host = Some("127.0.0.1".to_string());
            dsn_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let (dsn, ca_cert_path) = build_dsn_with_ca_path(&dsn_form)?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(&dsn)
            .await
            .map_err(|e| super::conn_failed_error(&e))?;

        Ok(Self {
            pool,
            ssh_tunnel,
            ca_cert_path,
        })
    }

    async fn describe_query_columns(&self, sql: &str) -> Result<Vec<QueryColumn>, String> {
        let describe = self
            .pool
            .describe(sql)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        Ok(describe
            .columns()
            .iter()
            .map(|col| QueryColumn {
                name: col.name().to_string(),
                r#type: col.type_info().name().to_string(),
            })
            .collect())
    }

    async fn load_high_precision_columns(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<HashSet<String>, String> {
        let rows = sqlx::query(
            "SELECT column_name, data_type, udt_name \
            FROM information_schema.columns \
            WHERE table_schema = $1 AND table_name = $2",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] Failed to load column types: {e}"))?;

        let mut cols = HashSet::new();
        for row in rows {
            let col_name = decode_postgres_text_cell(&row, 0)?;
            let data_type = decode_postgres_text_cell(&row, 1)?;
            let udt_name = decode_postgres_text_cell(&row, 2)?;
            if is_high_precision_pg_type(&data_type, &udt_name) {
                cols.insert(col_name);
            }
        }
        Ok(cols)
    }

    async fn fetch_table_rows_as_json(
        &self,
        schema: &str,
        table: &str,
        where_clause: &str,
        order_clause: &str,
        limit: i64,
        offset: i64,
        high_precision_cols: &HashSet<String>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let query = format!(
            "SELECT to_jsonb(t) AS __row_json FROM {}.{} t{}{} LIMIT $1 OFFSET $2",
            schema, table, where_clause, order_clause
        );
        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", query, e))?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            let mut row_json = row
                .try_get::<sqlx::types::Json<serde_json::Value>, _>("__row_json")
                .map(|v| v.0)
                .map_err(|e| format!("[QUERY_ERROR] Failed to decode __row_json: {e}"))?;
            normalize_postgres_row_json(&mut row_json, high_precision_cols)?;
            data.push(row_json);
        }
        Ok(data)
    }
}

fn is_high_precision_pg_type(data_type: &str, udt_name: &str) -> bool {
    let data_type = data_type.to_ascii_lowercase();
    let udt_name = udt_name.to_ascii_lowercase();
    matches!(
        data_type.as_str(),
        "bigint" | "numeric" | "decimal" | "money"
    ) || matches!(udt_name.as_str(), "int8" | "numeric" | "decimal" | "money")
}

fn normalize_postgres_row_json(
    row_json: &mut serde_json::Value,
    high_precision_cols: &HashSet<String>,
) -> Result<(), String> {
    let obj = row_json
        .as_object_mut()
        .ok_or("[QUERY_ERROR] Expected JSON object row from to_jsonb".to_string())?;

    let mut lookup: HashMap<String, String> = HashMap::new();
    for key in obj.keys() {
        lookup.insert(key.to_ascii_lowercase(), key.clone());
    }

    for col in high_precision_cols {
        let Some(actual_key) = lookup.get(&col.to_ascii_lowercase()) else {
            continue;
        };
        let Some(value) = obj.get_mut(actual_key) else {
            continue;
        };
        if value.is_number() {
            *value = serde_json::Value::String(value.to_string());
        }
    }

    Ok(())
}

fn decode_postgres_text_cell(row: &sqlx::postgres::PgRow, idx: usize) -> Result<String, String> {
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return Ok(String::from_utf8_lossy(&v).to_string());
    }
    Err(format!(
        "[QUERY_ERROR] Failed to decode Postgres text column at index {idx}"
    ))
}

fn decode_postgres_optional_text_cell(
    row: &sqlx::postgres::PgRow,
    idx: usize,
) -> Result<Option<String>, String> {
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return Ok(v.map(|b| String::from_utf8_lossy(&b).to_string()));
    }
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return Ok(Some(v));
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return Ok(Some(String::from_utf8_lossy(&v).to_string()));
    }
    Err(format!(
        "[QUERY_ERROR] Failed to decode Postgres optional text column at index {idx}"
    ))
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

fn skip_ignorable_sql_prefix(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i = skip_line_comment(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i);
            continue;
        }
        break;
    }
    i
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut statements = Vec::new();
    let mut i = 0;
    let mut depth = 0_i32;
    let mut start = skip_ignorable_sql_prefix(bytes, 0);

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
        if b == b';' && depth == 0 {
            let stmt = sql[start..i].trim();
            if !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            start = skip_ignorable_sql_prefix(bytes, i + 1);
        }
        i += 1;
    }

    let tail = sql[start..].trim();
    if !tail.is_empty() {
        statements.push(tail.to_string());
    }

    statements
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    let bytes = sql.as_bytes();
    let start = skip_ignorable_sql_prefix(bytes, 0);
    if start >= bytes.len() {
        return None;
    }
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    if end == start {
        return None;
    }
    Some(sql[start..end].to_ascii_uppercase())
}

fn is_json_projectable_statement(sql: &str) -> bool {
    matches!(
        first_sql_keyword(sql).as_deref(),
        Some("SELECT" | "WITH" | "VALUES" | "TABLE")
    )
}

fn is_high_precision_query_type(type_name: &str) -> bool {
    matches!(
        type_name.trim().to_ascii_uppercase().as_str(),
        "INT8" | "BIGINT" | "NUMERIC" | "DECIMAL" | "MONEY"
    )
}

fn collect_high_precision_query_columns(columns: &[QueryColumn]) -> HashSet<String> {
    columns
        .iter()
        .filter(|col| is_high_precision_query_type(&col.r#type))
        .map(|col| col.name.clone())
        .collect()
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn close(&self) {
        self.pool.close().await;
        self.cleanup_ca_file();
    }

    async fn test_connection(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let rows = if let Some(schema) = schema {
            sqlx::query(
                "SELECT table_schema, table_name, table_type \
                 FROM information_schema.tables \
                 WHERE table_schema = $1 AND table_type IN ('BASE TABLE','VIEW') \
                 ORDER BY table_name",
            )
            .bind(&schema)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?
        } else {
            sqlx::query(
                "SELECT table_schema, table_name, table_type \
                 FROM information_schema.tables \
                 WHERE table_schema NOT IN ('information_schema', 'pg_catalog') \
                   AND table_schema NOT LIKE 'pg_toast%' \
                   AND table_type IN ('BASE TABLE','VIEW') \
                 ORDER BY table_schema, table_name",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?
        };

        let mut res = Vec::new();
        for row in rows {
            res.push(TableInfo {
                schema: decode_postgres_text_cell(&row, 0).unwrap_or_else(|_| "public".to_string()),
                name: decode_postgres_text_cell(&row, 1).unwrap_or_default(),
                r#type: decode_postgres_text_cell(&row, 2).unwrap_or_else(|_| "table".to_string()),
            });
        }
        Ok(res)
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default \
             FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(ColumnInfo {
                name: decode_postgres_text_cell(&row, 0).unwrap_or_default(),
                r#type: decode_postgres_text_cell(&row, 1).unwrap_or_default(),
                nullable: decode_postgres_text_cell(&row, 2).unwrap_or_default() == "YES",
                default_value: decode_postgres_optional_text_cell(&row, 3).ok().flatten(),
                primary_key: false, // TODO: Need to query constraint
                comment: None,
            });
        }
        Ok(TableStructure { columns })
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String> {
        let pk_rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT a.attname
            FROM pg_index i
            JOIN pg_class c ON c.oid = i.indrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
            JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum
            WHERE i.indisprimary = true
              AND n.nspname = $1
              AND c.relname = $2
            ORDER BY k.ord
            "#,
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let pk_set: HashSet<String> = pk_rows.into_iter().map(|r| r.0).collect();

        let column_rows = sqlx::query(
            r#"
            SELECT
              a.attname AS column_name,
              format_type(a.atttypid, a.atttypmod) AS column_type,
              a.attnotnull AS not_null,
              pg_get_expr(ad.adbin, ad.adrelid) AS column_default,
              d.description AS comment
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            JOIN pg_attribute a ON a.attrelid = c.oid
            LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum
            LEFT JOIN pg_description d ON d.objoid = a.attrelid AND d.objsubid = a.attnum
            WHERE n.nspname = $1
              AND c.relname = $2
              AND a.attnum > 0
              AND NOT a.attisdropped
            ORDER BY a.attnum
            "#,
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut columns = Vec::new();
        for row in column_rows {
            let name = decode_postgres_text_cell(&row, 0)?;
            let comment = decode_postgres_optional_text_cell(&row, 4)?;
            let comment = comment.and_then(|c| {
                let trimmed = c.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });
            let not_null: bool = row.try_get(2).unwrap_or(false);

            columns.push(ColumnInfo {
                name: name.clone(),
                r#type: decode_postgres_text_cell(&row, 1)?,
                nullable: !not_null,
                default_value: decode_postgres_optional_text_cell(&row, 3)?,
                primary_key: pk_set.contains(&name),
                comment,
            });
        }

        let index_rows = sqlx::query(
            r#"
            SELECT
              ic.relname AS index_name,
              i.indisunique AS is_unique,
              am.amname AS index_type,
              array_agg(a.attname ORDER BY k.ord) FILTER (WHERE a.attname IS NOT NULL) AS columns
            FROM pg_index i
            JOIN pg_class tc ON tc.oid = i.indrelid
            JOIN pg_namespace n ON n.oid = tc.relnamespace
            JOIN pg_class ic ON ic.oid = i.indexrelid
            JOIN pg_am am ON am.oid = ic.relam
            JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
            LEFT JOIN pg_attribute a ON a.attrelid = tc.oid AND a.attnum = k.attnum
            WHERE n.nspname = $1
              AND tc.relname = $2
            GROUP BY ic.relname, i.indisunique, am.amname
            ORDER BY ic.relname
            "#,
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut indexes = Vec::new();
        for row in index_rows {
            let name: String = row.try_get(0).unwrap_or_default();
            let unique: bool = row.try_get(1).unwrap_or(false);
            let index_type: String = row.try_get(2).unwrap_or_default();
            let columns: Option<Vec<String>> = row.try_get(3).ok();
            indexes.push(IndexInfo {
                name,
                unique,
                index_type: if index_type.is_empty() {
                    None
                } else {
                    Some(index_type)
                },
                columns: columns.unwrap_or_default(),
            });
        }

        let fk_rows = sqlx::query(
            r#"
            SELECT
              con.conname AS constraint_name,
              a.attname AS column_name,
              fn.nspname AS referenced_schema,
              fc.relname AS referenced_table,
              fa.attname AS referenced_column,
              CASE con.confupdtype
                WHEN 'a' THEN 'NO ACTION'
                WHEN 'r' THEN 'RESTRICT'
                WHEN 'c' THEN 'CASCADE'
                WHEN 'n' THEN 'SET NULL'
                WHEN 'd' THEN 'SET DEFAULT'
                ELSE NULL
              END AS on_update,
              CASE con.confdeltype
                WHEN 'a' THEN 'NO ACTION'
                WHEN 'r' THEN 'RESTRICT'
                WHEN 'c' THEN 'CASCADE'
                WHEN 'n' THEN 'SET NULL'
                WHEN 'd' THEN 'SET DEFAULT'
                ELSE NULL
              END AS on_delete
            FROM pg_constraint con
            JOIN pg_class c ON c.oid = con.conrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            JOIN pg_class fc ON fc.oid = con.confrelid
            JOIN pg_namespace fn ON fn.oid = fc.relnamespace
            JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS ck(attnum, ord) ON true
            JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fk(attnum, ord) ON fk.ord = ck.ord
            JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ck.attnum
            JOIN pg_attribute fa ON fa.attrelid = fc.oid AND fa.attnum = fk.attnum
            WHERE con.contype = 'f'
              AND n.nspname = $1
              AND c.relname = $2
            ORDER BY con.conname, ck.ord
            "#,
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut foreign_keys = Vec::new();
        for row in fk_rows {
            let referenced_schema: String = row.try_get(2).unwrap_or_default();
            foreign_keys.push(ForeignKeyInfo {
                name: row.try_get(0).unwrap_or_default(),
                column: row.try_get(1).unwrap_or_default(),
                referenced_schema: if referenced_schema.is_empty() {
                    None
                } else {
                    Some(referenced_schema)
                },
                referenced_table: row.try_get(3).unwrap_or_default(),
                referenced_column: row.try_get(4).unwrap_or_default(),
                on_update: row.try_get::<Option<String>, _>(5).unwrap_or(None),
                on_delete: row.try_get::<Option<String>, _>(6).unwrap_or(None),
            });
        }

        Ok(TableMetadata {
            columns,
            indexes,
            foreign_keys,
            clickhouse_extra: None,
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let query = r#"
            SELECT
                'CREATE TABLE ' || quote_ident(n.nspname) || '.' || quote_ident(c.relname) || ' (' || E'\n' ||
                array_to_string(array_agg(
                    '    ' || quote_ident(a.attname) || ' ' ||
                    format_type(a.atttypid, a.atttypmod) ||
                    CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END ||
                    CASE WHEN a.atthasdef THEN ' DEFAULT ' || pg_get_expr(d.adbin, d.adrelid) ELSE '' END
                    ORDER BY a.attnum
                ), E',\n') || E'\n' ||
                ');'
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            JOIN pg_attribute a ON a.attrelid = c.oid
            LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
            WHERE c.relkind = 'r' AND a.attnum > 0 AND NOT a.attisdropped
            AND n.nspname = $1 AND c.relname = $2
            GROUP BY n.nspname, c.relname;
        "#;

        let row: (String,) = sqlx::query_as(query)
            .bind(&schema)
            .bind(&table)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        Ok(row.0)
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
        let offset = (page - 1) * limit;

        // Normalize smart quotes from macOS input
        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        // Build WHERE clause from filter
        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        // Get total count (with filter applied)
        let count_query = format!("SELECT COUNT(*) FROM {}.{}{}", schema, table, where_clause);
        let total: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", count_query, e))?;

        // Build ORDER BY clause: order_by (raw) takes priority over sort_column/sort_direction
        let order_clause = if let Some(ref ob) = order_by {
            if !ob.trim().is_empty() {
                format!(" ORDER BY {}", ob.trim())
            } else {
                String::new()
            }
        } else if let Some(ref col) = sort_column {
            // Validate column name to prevent SQL injection
            if !col.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err("[VALIDATION_ERROR] Invalid sort column name".to_string());
            }
            let dir = match sort_direction.as_deref() {
                Some("desc") => "DESC",
                _ => "ASC",
            };
            format!(" ORDER BY \"{}\" {}", col, dir)
        } else {
            String::new()
        };

        let high_precision_cols = self.load_high_precision_columns(&schema, &table).await?;
        let data = self
            .fetch_table_rows_as_json(
                &schema,
                &table,
                &where_clause,
                &order_clause,
                limit,
                offset,
                &high_precision_cols,
            )
            .await?;

        let duration = start.elapsed();
        Ok(TableDataResponse {
            data,
            total,
            page,
            limit,
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
        let start = std::time::Instant::now();
        let statements = split_sql_statements(&sql);
        if statements.is_empty() {
            return Err("[QUERY_ERROR] Empty SQL statement".to_string());
        }
        let describe_sql = statements
            .last()
            .cloned()
            .unwrap_or_else(|| sql.trim().to_string());

        if statements.len() > 1 {
            for statement in statements.iter().take(statements.len() - 1) {
                sqlx::query(statement)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            }
        }

        let (columns, data, row_count) = if is_json_projectable_statement(&describe_sql) {
            let columns = self.describe_query_columns(&describe_sql).await?;
            let high_precision_cols = collect_high_precision_query_columns(&columns);
            let json_query = build_postgres_json_projection_query(&describe_sql);
            let rows = sqlx::query(&json_query)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", json_query, e))?;
            let mut data = Vec::with_capacity(rows.len());
            for row in rows {
                let mut row_json = row
                    .try_get::<sqlx::types::Json<serde_json::Value>, _>("__row_json")
                    .map(|v| v.0)
                    .map_err(|e| format!("[QUERY_ERROR] Failed to decode __row_json: {e}"))?;
                normalize_postgres_row_json(&mut row_json, &high_precision_cols)?;
                data.push(row_json);
            }
            let row_count = data.len() as i64;
            (columns, data, row_count)
        } else {
            let rows = sqlx::query(&describe_sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let columns = if let Some(first_row) = rows.first() {
                first_row
                    .columns()
                    .iter()
                    .map(|col| QueryColumn {
                        name: col.name().to_string(),
                        r#type: col.type_info().to_string(),
                    })
                    .collect()
            } else {
                self.describe_query_columns(&describe_sql).await?
            };

            let mut data = Vec::new();
            for row in &rows {
                let mut obj = serde_json::Map::new();
                for col in row.columns() {
                    let name = col.name();
                    let type_name = col.type_info().name();
                    let value = match type_name {
                        "BOOL" => row
                            .try_get::<bool, _>(name)
                            .ok()
                            .map(serde_json::Value::Bool)
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "INT2" | "INT4" | "INT8" => row
                            .try_get::<i64, _>(name)
                            .ok()
                            .map(|v| serde_json::Value::String(v.to_string()))
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "FLOAT4" | "FLOAT8" => row
                            .try_get::<f64, _>(name)
                            .ok()
                            .map(serde_json::Value::from)
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "NUMERIC" | "MONEY" => row
                            .try_get::<Decimal, _>(name)
                            .ok()
                            .map(|v| serde_json::Value::String(v.to_string()))
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" | "UUID" => row
                            .try_get::<String, _>(name)
                            .ok()
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null),
                        "DATE" => row
                            .try_get::<NaiveDate, _>(name)
                            .ok()
                            .map(|v| serde_json::Value::String(v.to_string()))
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "TIME" | "TIMETZ" | "INTERVAL" => row
                            .try_get::<NaiveTime, _>(name)
                            .ok()
                            .map(|v| serde_json::Value::String(v.to_string()))
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "TIMESTAMP" => row
                            .try_get::<NaiveDateTime, _>(name)
                            .ok()
                            .map(|v| serde_json::Value::String(v.to_string()))
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "TIMESTAMPTZ" => row
                            .try_get::<DateTime<Utc>, _>(name)
                            .ok()
                            .map(|v| serde_json::Value::String(v.to_string()))
                            .or_else(|| {
                                row.try_get::<String, _>(name)
                                    .ok()
                                    .map(serde_json::Value::String)
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "JSON" | "JSONB" => row
                            .try_get::<sqlx::types::Json<serde_json::Value>, _>(name)
                            .ok()
                            .map(|v| v.0)
                            .unwrap_or(serde_json::Value::Null),
                        // PostgreSQL array types (element type prefixed with _)
                        "_BOOL" => row
                            .try_get::<Vec<Option<bool>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(b) => serde_json::Value::Bool(b),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_INT2" => row
                            .try_get::<Vec<Option<i16>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(n) => serde_json::Value::Number(n.into()),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_INT4" => row
                            .try_get::<Vec<Option<i32>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(n) => serde_json::Value::Number(n.into()),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_INT8" => row
                            .try_get::<Vec<Option<i64>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(n) => serde_json::Value::Number(n.into()),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_FLOAT4" => row
                            .try_get::<Vec<Option<f32>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(f) => serde_json::Number::from_f64(f as f64)
                                                .map(serde_json::Value::Number)
                                                .unwrap_or_else(|| {
                                                    serde_json::Value::String(f.to_string())
                                                }),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_FLOAT8" => row
                            .try_get::<Vec<Option<f64>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(f) => serde_json::Number::from_f64(f)
                                                .map(serde_json::Value::Number)
                                                .unwrap_or_else(|| {
                                                    serde_json::Value::String(f.to_string())
                                                }),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_NUMERIC" => row
                            .try_get::<Vec<Option<Decimal>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(d) => serde_json::Value::String(d.to_string()),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_TEXT" | "_VARCHAR" | "_BPCHAR" | "_NAME" | "_UUID" => row
                            .try_get::<Vec<Option<String>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| match o {
                                            Some(s) => serde_json::Value::String(s),
                                            None => serde_json::Value::Null,
                                        })
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        "_JSON" | "_JSONB" => row
                            .try_get::<Vec<Option<serde_json::Value>>, _>(name)
                            .ok()
                            .map(|v| {
                                serde_json::Value::Array(
                                    v.into_iter()
                                        .map(|o| o.unwrap_or(serde_json::Value::Null))
                                        .collect(),
                                )
                            })
                            .unwrap_or(serde_json::Value::Null),
                        _ => {
                            if let Ok(v) = row.try_get::<String, _>(name) {
                                serde_json::Value::String(v)
                            } else if let Ok(v) = row.try_get::<Vec<u8>, _>(name) {
                                serde_json::Value::String(String::from_utf8_lossy(&v).to_string())
                            } else {
                                serde_json::Value::Null
                            }
                        }
                    };
                    obj.insert(name.to_string(), value);
                }
                data.push(serde_json::Value::Object(obj));
            }
            let row_count = rows.len() as i64;
            (columns, data, row_count)
        };

        let duration = start.elapsed();
        Ok(QueryResult {
            data,
            row_count,
            columns,
            time_taken_ms: duration.as_millis() as i64,
            success: true,
            error: None,
        })
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        // Note: Using a simpler approach for now since sqlx QueryBuilder needs specific DB type setup
        // and I don't want to overcomplicate.

        let rows = if let Some(s) = schema {
            sqlx::query(
                "SELECT table_schema, table_name, column_name, data_type \
             FROM information_schema.columns \
             WHERE table_schema = $1 \
             ORDER BY table_schema, table_name, ordinal_position",
            )
            .bind(s)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                "SELECT table_schema, table_name, column_name, data_type \
             FROM information_schema.columns \
             WHERE table_schema NOT IN ('information_schema', 'pg_catalog') \
             ORDER BY table_schema, table_name, ordinal_position",
            )
            .fetch_all(&self.pool)
            .await
        };

        let rows = rows.map_err(|e| {
            eprintln!("[QUERY_ERROR] Raw error: {}", e);
            "[QUERY_ERROR] Failed to fetch schema overview".to_string()
        })?;

        let mut tables_map: std::collections::HashMap<(String, String), Vec<ColumnSchema>> =
            std::collections::HashMap::new();

        for row in rows {
            let schema_name = decode_postgres_text_cell(&row, 0)
                .map_err(|e| format!("[PARSE_ERROR] Postgres table_schema: {}", e))?;
            let table_name = decode_postgres_text_cell(&row, 1)
                .map_err(|e| format!("[PARSE_ERROR] Postgres table_name: {}", e))?;
            let col_name = decode_postgres_text_cell(&row, 2)
                .map_err(|e| format!("[PARSE_ERROR] Postgres column_name: {}", e))?;
            let data_type = decode_postgres_text_cell(&row, 3)
                .map_err(|e| format!("[PARSE_ERROR] Postgres data_type: {}", e))?;

            let key = (schema_name, table_name);
            tables_map.entry(key).or_default().push(ColumnSchema {
                name: col_name,
                r#type: data_type,
            });
        }

        let mut tables = Vec::new();
        for ((schema_name, table_name), columns) in tables_map {
            tables.push(TableSchema {
                schema: schema_name,
                name: table_name,
                columns,
            });
        }

        tables.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));

        Ok(SchemaOverview { tables })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conn_string_generation() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ..Default::default()
        };
        // Use build_dsn directly
        let dsn = build_dsn(&form).unwrap();
        assert_eq!(dsn, "postgres://postgres:password@localhost:5432/mydb");
    }

    #[test]
    fn test_conn_string_encodes_credentials() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("user@name".to_string()),
            password: Some("p@ss:word#?".to_string()),
            database: Some("mydb".to_string()),
            ..Default::default()
        };

        let dsn = build_dsn(&form).unwrap();
        assert_eq!(
            dsn,
            "postgres://user%40name:p%40ss%3Aword%23%3F@localhost:5432/mydb"
        );
    }

    #[test]
    fn test_conn_string_encodes_credentials_when_ssh_rewrites_target_host() {
        let mut form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(5432),
            username: Some("user@name".to_string()),
            password: Some("p#ss*@)".to_string()),
            database: Some("mydb".to_string()),
            ssh_enabled: Some(true),
            ssh_host: Some("bastion.internal".to_string()),
            ssh_port: Some(22),
            ssh_username: Some("jump".to_string()),
            ssh_password: Some("ssh#pass".to_string()),
            ..Default::default()
        };

        // Match the production flow after the SSH tunnel assigns a local endpoint.
        form.host = Some("127.0.0.1".to_string());
        form.port = Some(55432);

        let dsn = build_dsn(&form).unwrap();
        assert_eq!(
            dsn,
            "postgres://user%40name:p%23ss%2A%40%29@127.0.0.1:55432/mydb"
        );
    }

    #[test]
    fn test_conn_string_missing_fields() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: None, // Missing host
            ..Default::default()
        };
        assert!(build_dsn(&form).is_err());
    }

    #[test]
    fn test_conn_string_with_ssl() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ssl: Some(true),
            ..Default::default()
        };
        let dsn = build_dsn(&form).unwrap();
        assert_eq!(
            dsn,
            "postgres://postgres:password@localhost:5432/mydb?sslmode=require"
        );
    }

    #[test]
    fn test_conn_string_with_ssl_false_does_not_explicitly_disable_tls() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ssl: Some(false),
            ..Default::default()
        };
        let dsn = build_dsn(&form).unwrap();
        assert_eq!(dsn, "postgres://postgres:password@localhost:5432/mydb");
        assert!(!dsn.contains("sslmode="));
        assert!(!dsn.contains("sslmode=disable"));
    }

    #[test]
    fn test_conn_string_with_ssl_verify_ca_requires_ca() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: Some("localhost".to_string()),
            port: Some(5432),
            username: Some("postgres".to_string()),
            password: Some("password".to_string()),
            database: Some("mydb".to_string()),
            ssl: Some(true),
            ssl_mode: Some("verify_ca".to_string()),
            ssl_ca_cert: None,
            ..Default::default()
        };
        assert!(build_dsn(&form).is_err());
    }

    #[test]
    fn test_verify_ca_query_param_encodes_path() {
        let path = PathBuf::from("/tmp/a b&c#d?.pem");
        let query = build_verify_ca_query_param(&path);
        assert_eq!(
            query,
            "?sslmode=verify-ca&sslrootcert=%2Ftmp%2Fa%20b%26c%23d%3F.pem"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_temp_cert_file_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = write_temp_cert_file("pg_ca_perm_test", "pem-data").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = fs::remove_file(&path);
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_cleanup_ca_file_opt_removes_file() {
        let path = write_temp_cert_file("pg_ca_cleanup_test", "pem-data").unwrap();
        assert!(path.exists());
        cleanup_ca_file_opt(Some(&path));
        assert!(!path.exists());
    }

    #[test]
    fn test_split_sql_statements_multi_ddl() {
        let sql = "CREATE TYPE mood_enum AS ENUM ('sad', 'ok'); CREATE TYPE address_type AS (street VARCHAR(100));";
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "CREATE TYPE mood_enum AS ENUM ('sad', 'ok')");
        assert_eq!(
            statements[1],
            "CREATE TYPE address_type AS (street VARCHAR(100))"
        );
    }

    #[test]
    fn test_split_sql_statements_ignores_semicolon_in_literal_and_comment() {
        let sql = "SELECT ';' AS x; -- noop ;\nSELECT 1;";
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "SELECT ';' AS x");
        assert_eq!(statements[1], "SELECT 1");
    }

    #[test]
    fn test_split_sql_statements_handles_domain_check_and_table_ddl() {
        let sql = "
CREATE DOMAIN email_domain AS VARCHAR(255)
    CHECK (VALUE ~* '^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}$');

CREATE TABLE pg_data_type_test (
    id BIGSERIAL PRIMARY KEY,
    col_domain email_domain
);";
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
        assert!(statements[0].starts_with("CREATE DOMAIN email_domain"));
        assert!(statements[1].starts_with("CREATE TABLE pg_data_type_test"));
    }

    #[test]
    fn test_split_sql_statements_keeps_postgres_dollar_quoted_function_intact() {
        let sql = r#"
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER LANGUAGE PLPGSQL AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$;
"#;
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("NEW.updated_at = CURRENT_TIMESTAMP;"));
        assert!(statements[0].ends_with("$$"));
    }

    #[test]
    fn test_split_sql_statements_keeps_tagged_dollar_quoted_function_intact() {
        let sql = r#"
CREATE FUNCTION demo()
RETURNS text LANGUAGE plpgsql AS $body$
BEGIN
    RETURN 'ok';
END;
$body$;
"#;
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 1);
        assert!(statements[0].contains("RETURN 'ok';"));
        assert!(statements[0].ends_with("$body$"));
    }

    #[test]
    fn test_is_high_precision_pg_type() {
        assert!(is_high_precision_pg_type("bigint", "int8"));
        assert!(is_high_precision_pg_type("numeric", "numeric"));
        assert!(is_high_precision_pg_type("money", "money"));
        assert!(!is_high_precision_pg_type("integer", "int4"));
        assert!(!is_high_precision_pg_type("text", "text"));
    }

    #[test]
    fn test_normalize_postgres_row_json_stringifies_high_precision_numbers() {
        let mut row = serde_json::json!({
            "col_bigint": 9007199254740993_i64,
            "col_numeric": 1234.56,
            "col_text": "hello",
            "col_null": null
        });
        let high_precision_cols =
            HashSet::from(["col_bigint".to_string(), "COL_NUMERIC".to_string()]);

        normalize_postgres_row_json(&mut row, &high_precision_cols).unwrap();

        assert_eq!(
            row.get("col_bigint").and_then(|v| v.as_str()),
            Some("9007199254740993")
        );
        assert_eq!(
            row.get("col_numeric").and_then(|v| v.as_str()),
            Some("1234.56")
        );
        assert_eq!(row.get("col_text").and_then(|v| v.as_str()), Some("hello"));
        assert!(row.get("col_null").unwrap().is_null());
    }

    #[test]
    fn test_normalize_postgres_row_json_requires_object() {
        let mut row = serde_json::json!(["a", "b"]);
        let high_precision_cols = HashSet::from(["id".to_string()]);
        assert!(normalize_postgres_row_json(&mut row, &high_precision_cols).is_err());
    }

    #[test]
    fn test_is_json_projectable_statement() {
        assert!(is_json_projectable_statement("SELECT 1"));
        assert!(is_json_projectable_statement(
            "  -- a\nWITH t AS (SELECT 1) SELECT * FROM t"
        ));
        assert!(is_json_projectable_statement("VALUES (1), (2)"));
        assert!(is_json_projectable_statement("TABLE my_table"));
        assert!(!is_json_projectable_statement("INSERT INTO t VALUES (1)"));
        assert!(!is_json_projectable_statement("UPDATE t SET a = 1"));
    }

    #[test]
    fn test_collect_high_precision_query_columns() {
        let columns = vec![
            QueryColumn {
                name: "id".to_string(),
                r#type: "INT8".to_string(),
            },
            QueryColumn {
                name: "amount".to_string(),
                r#type: "NUMERIC".to_string(),
            },
            QueryColumn {
                name: "title".to_string(),
                r#type: "TEXT".to_string(),
            },
        ];
        let picked = collect_high_precision_query_columns(&columns);
        assert!(picked.contains("id"));
        assert!(picked.contains("amount"));
        assert!(!picked.contains("title"));
    }

    #[test]
    fn test_build_postgres_json_projection_query_strips_trailing_semicolon() {
        let sql = build_postgres_json_projection_query("SELECT * FROM t LIMIT 1000;");
        assert!(sql.contains("FROM (SELECT * FROM t LIMIT 1000) AS __dbpaw_row"));
        assert!(!sql.contains(";) AS __dbpaw_row"));
    }

    #[test]
    fn test_build_postgres_json_projection_query_strips_multiple_trailing_semicolons() {
        let sql = build_postgres_json_projection_query("SELECT * FROM t;;;");
        assert!(sql.contains("FROM (SELECT * FROM t) AS __dbpaw_row"));
        assert!(!sql.contains(";) AS __dbpaw_row"));
    }
}
