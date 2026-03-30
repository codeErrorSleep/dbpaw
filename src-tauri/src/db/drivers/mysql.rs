use super::{strip_trailing_statement_terminator, DatabaseDriver};
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use sqlx::{mysql::MySqlPoolOptions, Column, Executor, Row, TypeInfo};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::ssh::SshTunnel;

pub struct MysqlDriver {
    pub pool: sqlx::MySqlPool,
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
        "?ssl-mode=VERIFY_CA&ssl-ca={}",
        percent_encode_query_value(&ca_path.to_string_lossy())
    )
}

fn normalize_mysql_host_and_port(
    raw_host: &str,
    raw_port: Option<i64>,
) -> Result<(String, u16), String> {
    let mut host = raw_host.trim().to_string();
    if host.is_empty() {
        return Err("[VALIDATION_ERROR] host cannot be empty".to_string());
    }

    let mut port = raw_port.unwrap_or(3306);
    if !host.starts_with('[') && host.matches(':').count() == 1 {
        if let Some((host_part, port_part)) = host.rsplit_once(':') {
            let host_part = host_part.trim();
            let port_part = port_part.trim();
            if !host_part.is_empty() && port_part.chars().all(|c| c.is_ascii_digit()) {
                if raw_port.is_none() {
                    port = port_part.parse::<i64>().unwrap_or(port);
                }
                host = host_part.to_string();
            }
        }
    }

    if host.is_empty() {
        return Err("[VALIDATION_ERROR] host cannot be empty".to_string());
    }
    if !(1..=65535).contains(&port) {
        return Err("[VALIDATION_ERROR] port must be between 1 and 65535".to_string());
    }

    Ok((host, port as u16))
}

fn build_dsn_and_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), String> {
    let raw_host = form
        .host
        .clone()
        .ok_or("[VALIDATION_ERROR] host cannot be empty")?;
    let (host, port) = normalize_mysql_host_and_port(&raw_host, form.port)?;
    // Allow database to be empty
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
    let mut dsn = format!("mysql://{}:{}@{}:{}", username, password, host, port);

    if let Some(db) = &form.database {
        if !db.is_empty() {
            dsn.push('/');
            dsn.push_str(db);
        }
    }

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
            let ca_path = write_temp_cert_file("mysql_ca", ca_cert)?;
            dsn.push_str(&build_verify_ca_query_param(&ca_path));
            ca_cert_path = Some(ca_path);
        } else {
            dsn.push_str("?ssl-mode=REQUIRED");
        }
    } else {
        // Explicitly disable TLS to avoid HandshakeFailure on servers with TLS
        // versions or cipher suites incompatible with rustls (TLS 1.2+ only).
        dsn.push_str("?ssl-mode=DISABLED");
    }

    Ok((dsn, ca_cert_path))
}

#[cfg(test)]
fn build_dsn(form: &ConnectionForm) -> Result<String, String> {
    Ok(build_dsn_and_ca_path(form)?.0)
}

#[cfg(test)]
pub(crate) fn build_test_dsn(form: &ConnectionForm) -> Result<String, String> {
    build_dsn(form)
}

fn build_dsn_with_ca_path(form: &ConnectionForm) -> Result<(String, Option<PathBuf>), String> {
    build_dsn_and_ca_path(form)
}

fn cleanup_ca_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn cleanup_ca_file_opt(path: Option<&PathBuf>) {
    if let Some(p) = path {
        cleanup_ca_file(p);
    }
}

fn is_prepared_protocol_unsupported_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("1295") || lower.contains("prepared statement protocol")
}

impl Drop for MysqlDriver {
    fn drop(&mut self) {
        cleanup_ca_file_opt(self.ca_cert_path.as_ref());
    }
}

impl MysqlDriver {
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
        let pool = MySqlPoolOptions::new()
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

    async fn resolve_schema_name(&self, schema: &str) -> Result<String, String> {
        if !schema.trim().is_empty() {
            return Ok(schema.to_string());
        }
        let row = sqlx::query("SELECT DATABASE()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] Failed to resolve current database: {e}"))?;
        decode_mysql_optional_text_cell(&row, 0)?
            .ok_or("[QUERY_ERROR] No active MySQL database selected".to_string())
    }

    async fn load_table_columns(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let rows = sqlx::query(
            "SELECT column_name, data_type \
            FROM information_schema.columns \
            WHERE table_schema = ? AND table_name = ? \
            ORDER BY ordinal_position",
        )
        .bind(schema)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] Failed to load MySQL column metadata: {e}"))?;

        let mut columns = Vec::with_capacity(rows.len());
        for row in rows {
            let name = decode_mysql_text_cell(&row, 0)?;
            let data_type = decode_mysql_text_cell(&row, 1)?;
            columns.push((name, data_type));
        }
        Ok(columns)
    }

    async fn fetch_rows_as_json(
        &self,
        base_query: &str,
        binds: &[i64],
        json_expr: &str,
        high_precision_cols: &HashSet<String>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let query = build_mysql_json_projection_query(base_query, json_expr);

        let mut q = sqlx::query(&query);
        for bind in binds {
            q = q.bind(*bind);
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", query, e))?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            let mut row_json = decode_mysql_json_cell(&row, "__row_json")?;
            normalize_mysql_row_json(&mut row_json, high_precision_cols)?;
            data.push(row_json);
        }
        Ok(data)
    }
}

fn sanitize_mysql_subquery_sql(sql: &str) -> &str {
    strip_trailing_statement_terminator(sql)
}

fn build_mysql_json_projection_query(base_query: &str, json_expr: &str) -> String {
    let sanitized_base_query = sanitize_mysql_subquery_sql(base_query);
    format!(
        "SELECT {} AS __row_json FROM ({}) AS {}",
        json_expr,
        sanitized_base_query,
        quote_mysql_ident("__dbpaw_row")
    )
}

fn decode_mysql_text_cell(row: &sqlx::mysql::MySqlRow, idx: usize) -> Result<String, String> {
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return Ok(String::from_utf8_lossy(&v).to_string());
    }
    Err(format!(
        "[QUERY_ERROR] Failed to decode MySQL text column at index {idx}"
    ))
}

fn decode_mysql_optional_text_cell(
    row: &sqlx::mysql::MySqlRow,
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
        "[QUERY_ERROR] Failed to decode MySQL optional text column at index {idx}"
    ))
}

fn quote_mysql_ident(ident: &str) -> String {
    format!("`{}`", ident.replace('`', "``"))
}

fn quote_mysql_json_key(key: &str) -> String {
    format!("'{}'", key.replace('\'', "''"))
}

fn mysql_qualified_table(schema: &str, table: &str) -> String {
    if schema.is_empty() {
        quote_mysql_ident(table)
    } else {
        format!("{}.{}", quote_mysql_ident(schema), quote_mysql_ident(table))
    }
}

fn is_high_precision_mysql_data_type(data_type: &str) -> bool {
    matches!(
        data_type.trim().to_ascii_lowercase().as_str(),
        "bigint" | "decimal" | "numeric"
    )
}

fn is_high_precision_mysql_query_type(type_name: &str) -> bool {
    let type_name = type_name.trim().to_ascii_uppercase();
    type_name == "BIGINT" || type_name == "BIGINT UNSIGNED" || type_name.starts_with("DECIMAL")
}

fn normalize_mysql_row_json(
    row_json: &mut serde_json::Value,
    high_precision_cols: &HashSet<String>,
) -> Result<(), String> {
    let obj = row_json
        .as_object_mut()
        .ok_or("[QUERY_ERROR] Expected JSON object row from JSON_OBJECT".to_string())?;

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

fn decode_mysql_json_cell(
    row: &sqlx::mysql::MySqlRow,
    column_name: &str,
) -> Result<serde_json::Value, String> {
    if let Ok(v) = row.try_get::<sqlx::types::Json<serde_json::Value>, _>(column_name) {
        return Ok(v.0);
    }
    if let Ok(v) = row.try_get::<String, _>(column_name) {
        return serde_json::from_str(&v)
            .map_err(|e| format!("[QUERY_ERROR] Failed to parse JSON cell: {e}"));
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(column_name) {
        return serde_json::from_slice(&v)
            .map_err(|e| format!("[QUERY_ERROR] Failed to parse JSON bytes cell: {e}"));
    }
    Err("[QUERY_ERROR] Failed to decode MySQL JSON cell".to_string())
}

fn build_mysql_json_object_expr(columns: &[(String, String)], table_alias: Option<&str>) -> String {
    if columns.is_empty() {
        return "JSON_OBJECT()".to_string();
    }

    let alias = table_alias.map(quote_mysql_ident);
    let mut args = Vec::with_capacity(columns.len() * 2);
    for (name, data_type) in columns {
        args.push(quote_mysql_json_key(name));
        let base_ref = if let Some(alias) = &alias {
            format!("{}.{}", alias, quote_mysql_ident(name))
        } else {
            quote_mysql_ident(name)
        };
        if is_high_precision_mysql_data_type(data_type) {
            args.push(format!("CAST({base_ref} AS CHAR)"));
        } else {
            args.push(base_ref);
        }
    }
    format!("JSON_OBJECT({})", args.join(", "))
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    let trimmed = sql.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let mut end = 0usize;
    for (idx, ch) in trimmed.char_indices() {
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            break;
        }
        end = idx + ch.len_utf8();
    }
    if end == 0 {
        None
    } else {
        Some(trimmed[..end].to_ascii_uppercase())
    }
}

fn is_json_projectable_statement(sql: &str) -> bool {
    matches!(first_sql_keyword(sql).as_deref(), Some("SELECT" | "WITH"))
}

fn is_affected_rows_statement(sql: &str) -> bool {
    matches!(
        first_sql_keyword(sql).as_deref(),
        Some("INSERT" | "UPDATE" | "DELETE" | "REPLACE")
    )
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
    async fn close(&self) {
        self.pool.close().await;
        self.cleanup_ca_file();
    }

    async fn test_connection(&self) -> Result<(), String> {
        if let Err(e) = sqlx::query("SELECT 1").execute(&self.pool).await {
            let error_text = e.to_string();
            if is_prepared_protocol_unsupported_error(&error_text) {
                sqlx::raw_sql("SELECT 1")
                    .execute(&self.pool)
                    .await
                    .map_err(|raw_err| format!("[QUERY_ERROR] {raw_err}"))?;
            } else {
                return Err(format!("[QUERY_ERROR] {e}"));
            }
        }
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let rows = sqlx::query("SHOW DATABASES")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        rows.into_iter()
            .map(|row| decode_mysql_text_cell(&row, 0))
            .collect()
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        // For MySQL, schema is usually the database name.
        // If schema is provided, use it. If not, use the current database (which might be in the DSN).
        // However, list_tables implementation used self.form.database to fallback.
        // Since we don't store form anymore, we should rely on the pool's current DB or the passed schema.
        // But the original code relied on `form.database`.
        // If schema is None, we need to know the current database.
        // We can query it: SELECT DATABASE()

        let target_schema = if let Some(s) = schema {
            s
        } else {
            // Fallback: try to get current database
            let row = sqlx::query("SELECT DATABASE()")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| format!("[QUERY_ERROR] Failed to get current database: {e}"))?;
            decode_mysql_optional_text_cell(&row, 0)?
                .ok_or("[QUERY_ERROR] No database selected and no schema provided")?
        };

        let rows = sqlx::query(
            "SELECT table_schema, table_name, table_type \
             FROM information_schema.tables \
             WHERE table_schema = ? AND table_type IN ('BASE TABLE','VIEW') \
             ORDER BY table_name",
        )
        .bind(&target_schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut res = Vec::new();
        for row in rows {
            let table_schema = decode_mysql_text_cell(&row, 0)?;
            let table_name = decode_mysql_text_cell(&row, 1)?;
            let table_type = decode_mysql_text_cell(&row, 2)?;
            res.push(TableInfo {
                schema: table_schema,
                name: table_name,
                r#type: if table_type == "VIEW" {
                    "view".to_string()
                } else {
                    "table".to_string()
                },
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
             WHERE table_schema = ? AND table_name = ? \
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
                name: decode_mysql_text_cell(&row, 0).unwrap_or_default(),
                r#type: decode_mysql_text_cell(&row, 1).unwrap_or_default(),
                nullable: decode_mysql_text_cell(&row, 2).unwrap_or_default() == "YES",
                default_value: decode_mysql_optional_text_cell(&row, 3).ok().flatten(),
                primary_key: false, // TODO
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
        let pk_rows = sqlx::query(
            "SELECT kcu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
              AND tc.table_schema = kcu.table_schema \
              AND tc.table_name = kcu.table_name \
             WHERE tc.constraint_type = 'PRIMARY KEY' \
               AND tc.table_schema = ? \
               AND tc.table_name = ? \
             ORDER BY kcu.ordinal_position",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut pk_set: HashSet<String> = HashSet::new();
        for row in pk_rows {
            pk_set.insert(decode_mysql_text_cell(&row, 0)?);
        }

        let column_rows = sqlx::query(
            "SELECT column_name, column_type, is_nullable, column_default, column_comment \
             FROM information_schema.columns \
             WHERE table_schema = ? AND table_name = ? \
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut columns = Vec::new();
        for row in column_rows {
            let name = decode_mysql_text_cell(&row, 0)?;
            let comment = decode_mysql_optional_text_cell(&row, 4)?;
            let comment = comment.and_then(|c| {
                let trimmed = c.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });
            columns.push(ColumnInfo {
                name: name.clone(),
                r#type: decode_mysql_text_cell(&row, 1)?,
                nullable: decode_mysql_text_cell(&row, 2)? == "YES",
                default_value: decode_mysql_optional_text_cell(&row, 3)?,
                primary_key: pk_set.contains(&name),
                comment,
            });
        }

        let index_rows = sqlx::query(
            "SELECT index_name, non_unique, index_type, seq_in_index, column_name \
             FROM information_schema.statistics \
             WHERE table_schema = ? AND table_name = ? \
             ORDER BY index_name, seq_in_index",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut index_map: HashMap<String, (bool, Option<String>, Vec<(i64, String)>)> =
            HashMap::new();
        for row in index_rows {
            let index_name: String = row.try_get(0).unwrap_or_default();
            let non_unique: i64 = row.try_get(1).unwrap_or(1);
            let index_type: Option<String> = row.try_get::<Option<String>, _>(2).unwrap_or(None);
            let seq: i64 = row.try_get(3).unwrap_or(0);
            let column_name: Option<String> = row.try_get::<Option<String>, _>(4).unwrap_or(None);
            let Some(column_name) = column_name else {
                continue;
            };

            let entry = index_map.entry(index_name).or_insert((
                non_unique == 0,
                index_type.clone(),
                Vec::new(),
            ));
            entry.0 = non_unique == 0;
            if entry.1.is_none() {
                entry.1 = index_type;
            }
            entry.2.push((seq, column_name));
        }

        let mut indexes = index_map
            .into_iter()
            .map(|(name, (unique, index_type, mut cols))| {
                cols.sort_by_key(|c| c.0);
                IndexInfo {
                    name,
                    unique,
                    index_type,
                    columns: cols.into_iter().map(|c| c.1).collect(),
                }
            })
            .collect::<Vec<_>>();
        indexes.sort_by(|a, b| a.name.cmp(&b.name));

        let fk_rows = sqlx::query(
            "SELECT \
               kcu.constraint_name, \
               kcu.column_name, \
               kcu.referenced_table_schema, \
               kcu.referenced_table_name, \
               kcu.referenced_column_name, \
               rc.update_rule, \
               rc.delete_rule \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
              AND tc.table_schema = kcu.table_schema \
              AND tc.table_name = kcu.table_name \
             LEFT JOIN information_schema.referential_constraints rc \
               ON rc.constraint_name = tc.constraint_name \
              AND rc.constraint_schema = tc.table_schema \
             WHERE tc.constraint_type = 'FOREIGN KEY' \
               AND tc.table_schema = ? \
               AND tc.table_name = ? \
             ORDER BY kcu.constraint_name, kcu.ordinal_position",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut foreign_keys = Vec::new();
        for row in fk_rows {
            foreign_keys.push(ForeignKeyInfo {
                name: row.try_get(0).unwrap_or_default(),
                column: row.try_get(1).unwrap_or_default(),
                referenced_schema: row.try_get::<Option<String>, _>(2).unwrap_or(None),
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
        let qualified = if schema.is_empty() {
            format!("`{}`", table)
        } else {
            format!("`{}`.`{}`", schema, table)
        };
        let query = format!("SHOW CREATE TABLE {}", qualified);
        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        decode_mysql_text_cell(&row, 1)
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
        let qualified = mysql_qualified_table(&schema, &table);

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let count_query = format!("SELECT COUNT(*) FROM {}{}", qualified, where_clause);
        let total: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", count_query, e))?;

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
            format!(" ORDER BY {} {}", quote_mysql_ident(col), dir)
        } else {
            String::new()
        };

        let target_schema = self.resolve_schema_name(&schema).await?;
        let table_columns = self.load_table_columns(&target_schema, &table).await?;
        let high_precision_cols: HashSet<String> = table_columns
            .iter()
            .filter(|(_, data_type)| is_high_precision_mysql_data_type(data_type))
            .map(|(name, _)| name.clone())
            .collect();
        let json_expr = build_mysql_json_object_expr(&table_columns, Some("__dbpaw_row"));
        let base_query = format!(
            "SELECT * FROM {}{}{} LIMIT ? OFFSET ?",
            qualified, where_clause, order_clause
        );
        let data = self
            .fetch_rows_as_json(
                &base_query,
                &[limit, offset],
                &json_expr,
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
        let (columns, data, row_count) = if is_json_projectable_statement(&sql) {
            let columns = self.describe_query_columns(&sql).await?;
            let high_precision_cols: HashSet<String> = columns
                .iter()
                .filter(|col| is_high_precision_mysql_query_type(&col.r#type))
                .map(|col| col.name.clone())
                .collect();
            let query_columns: Vec<(String, String)> = columns
                .iter()
                .map(|col| (col.name.clone(), col.r#type.clone()))
                .collect();
            let json_expr = build_mysql_json_object_expr(&query_columns, Some("__dbpaw_row"));
            let data = self
                .fetch_rows_as_json(&sql, &[], &json_expr, &high_precision_cols)
                .await?;
            let row_count = data.len() as i64;
            (columns, data, row_count)
        } else if is_affected_rows_statement(&sql) {
            let result = sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            (Vec::new(), Vec::new(), result.rows_affected() as i64)
        } else {
            let mut executed_with_raw_sql = false;
            let rows = match sqlx::query(&sql).fetch_all(&self.pool).await {
                Ok(rows) => rows,
                Err(e) => {
                    let error_text = e.to_string();
                    if is_prepared_protocol_unsupported_error(&error_text) {
                        sqlx::raw_sql(&sql)
                            .execute(&self.pool)
                            .await
                            .map_err(|raw_err| format!("[QUERY_ERROR] {raw_err}"))?;
                        executed_with_raw_sql = true;
                        Vec::new()
                    } else {
                        return Err(format!("[QUERY_ERROR] {e}"));
                    }
                }
            };
            let columns = if let Some(first_row) = rows.first() {
                first_row
                    .columns()
                    .iter()
                    .map(|col| QueryColumn {
                        name: col.name().to_string(),
                        r#type: col.type_info().to_string(),
                    })
                    .collect()
            } else if executed_with_raw_sql {
                Vec::new()
            } else {
                self.describe_query_columns(&sql).await?
            };
            let mut data = Vec::new();
            for row in &rows {
                let mut obj = serde_json::Map::new();
                for col in row.columns() {
                    let name = col.name();
                    if let Ok(v) = row.try_get::<String, _>(name) {
                        obj.insert(name.to_string(), serde_json::Value::String(v));
                    } else if let Ok(v) = row.try_get::<Vec<u8>, _>(name) {
                        obj.insert(
                            name.to_string(),
                            serde_json::Value::String(String::from_utf8_lossy(&v).to_string()),
                        );
                    } else {
                        obj.insert(name.to_string(), serde_json::Value::Null);
                    }
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
        let sql = "SELECT table_schema, table_name, column_name, data_type \
             FROM information_schema.columns"
            .to_string();

        let rows = if let Some(s) = schema {
            sqlx::query(&format!(
                "{} WHERE table_schema = ? ORDER BY table_schema, table_name, ordinal_position",
                sql
            ))
            .bind(s)
            .fetch_all(&self.pool)
            .await
        } else {
            // Try to use current DB if available in pool, otherwise exclude system schemas
            // Since we don't have form.database easily available, we check if we can query without specific schema.
            // But the original code had fallback logic.
            // Let's assume if no schema provided, we list all non-system schemas OR just the current one if connected to one.
            // If connected to a specific DB, `SHOW TABLES` works for that DB. But we query `information_schema`.

            // We can query SELECT DATABASE() first.
            let db_row = sqlx::query("SELECT DATABASE()").fetch_one(&self.pool).await;

            if let Ok(row) = db_row {
                let current_db = decode_mysql_optional_text_cell(&row, 0).ok().flatten();
                if let Some(db) = current_db {
                    sqlx::query(&format!(
                    "{} WHERE table_schema = ? ORDER BY table_schema, table_name, ordinal_position",
                    sql
                ))
                    .bind(db)
                    .fetch_all(&self.pool)
                    .await
                } else {
                    sqlx::query(&format!("{} WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys') ORDER BY table_schema, table_name, ordinal_position", sql))
                        .fetch_all(&self.pool)
                        .await
                }
            } else {
                sqlx::query(&format!("{} WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys') ORDER BY table_schema, table_name, ordinal_position", sql))
                .fetch_all(&self.pool)
                .await
            }
        };

        let rows = rows.map_err(|e| {
            eprintln!("[QUERY_ERROR] Raw error: {}", e);
            "[QUERY_ERROR] Failed to fetch schema overview".to_string()
        })?;

        let mut tables_map: std::collections::HashMap<(String, String), Vec<ColumnSchema>> =
            std::collections::HashMap::new();

        for row in rows {
            let schema_name = decode_mysql_text_cell(&row, 0)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get table_schema: {}", e))?;
            let table_name = decode_mysql_text_cell(&row, 1)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get table_name: {}", e))?;
            let col_name = decode_mysql_text_cell(&row, 2)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get column_name: {}", e))?;
            let data_type = decode_mysql_text_cell(&row, 3)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get data_type: {}", e))?;

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
    use crate::models::ConnectionForm;

    #[test]
    fn test_sanitize_mysql_subquery_sql_trims_trailing_semicolon() {
        assert_eq!(
            sanitize_mysql_subquery_sql("select * from t_business LIMIT 1000;"),
            "select * from t_business LIMIT 1000"
        );
        assert_eq!(
            sanitize_mysql_subquery_sql("select * from t_business LIMIT 1000;   "),
            "select * from t_business LIMIT 1000"
        );
        assert_eq!(
            sanitize_mysql_subquery_sql("select * from t_business"),
            "select * from t_business"
        );
    }

    #[test]
    fn test_conn_string_generation() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3306/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_without_db() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(3307),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            database: None,
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user:pass@127.0.0.1:3307?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_allows_empty_password_when_present() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1".to_string()),
            port: Some(3307),
            username: Some("user".to_string()),
            password: Some(String::new()),
            database: None,
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(conn_str, "mysql://user:@127.0.0.1:3307?ssl-mode=DISABLED");
    }

    #[test]
    fn test_conn_string_strips_host_embedded_port() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1:3307".to_string()),
            port: Some(3307),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            database: None,
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user:pass@127.0.0.1:3307?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_accepts_host_embedded_port_when_port_missing() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost:3308".to_string()),
            port: None,
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3308/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_encodes_credentials() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("user@name".to_string()),
            password: Some("p@ss:word#?".to_string()),
            database: Some("test_db".to_string()),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user%40name:p%40ss%3Aword%23%3F@localhost:3306/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_encodes_credentials_when_ssh_rewrites_target_host() {
        let mut form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("db.internal".to_string()),
            port: Some(3306),
            username: Some("user@name".to_string()),
            password: Some("p#ss*@)".to_string()),
            database: Some("test_db".to_string()),
            ssh_enabled: Some(true),
            ssh_host: Some("bastion.internal".to_string()),
            ssh_port: Some(22),
            ssh_username: Some("jump".to_string()),
            ssh_password: Some("ssh#pass".to_string()),
            ..Default::default()
        };

        // Match the production flow after the SSH tunnel assigns a local endpoint.
        form.host = Some("127.0.0.1".to_string());
        form.port = Some(4406);

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://user%40name:p%23ss%2A%40%29@127.0.0.1:4406/test_db?ssl-mode=DISABLED"
        );
    }

    #[test]
    fn test_conn_string_missing_fields() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: None, // Missing host
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test".to_string()),
            ..Default::default()
        };

        assert!(build_dsn(&form).is_err());
    }

    #[test]
    fn test_conn_string_with_ssl() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ssl: Some(true),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3306/test_db?ssl-mode=REQUIRED"
        );
    }

    #[test]
    fn test_conn_string_with_ssl_false_explicitly_disables_tls() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
            ssl: Some(false),
            ..Default::default()
        };

        let conn_str = build_dsn(&form).unwrap();
        assert_eq!(
            conn_str,
            "mysql://root:password@localhost:3306/test_db?ssl-mode=DISABLED"
        );
        assert!(conn_str.contains("ssl-mode=DISABLED"));
    }

    #[test]
    fn test_conn_string_with_ssl_verify_ca_requires_ca() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("localhost".to_string()),
            port: Some(3306),
            username: Some("root".to_string()),
            password: Some("password".to_string()),
            database: Some("test_db".to_string()),
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
            "?ssl-mode=VERIFY_CA&ssl-ca=%2Ftmp%2Fa%20b%26c%23d%3F.pem"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_temp_cert_file_sets_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = write_temp_cert_file("mysql_ca_perm_test", "pem-data").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = fs::remove_file(&path);
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_cleanup_ca_file_opt_removes_file() {
        let path = write_temp_cert_file("mysql_ca_cleanup_test", "pem-data").unwrap();
        assert!(path.exists());
        cleanup_ca_file_opt(Some(&path));
        assert!(!path.exists());
    }

    #[test]
    fn test_is_json_projectable_statement() {
        assert!(is_json_projectable_statement("SELECT 1"));
        assert!(is_json_projectable_statement(
            "  WITH t AS (SELECT 1) SELECT * FROM t"
        ));
        assert!(!is_json_projectable_statement("SHOW TABLES"));
        assert!(!is_json_projectable_statement("UPDATE t SET a = 1"));
    }

    #[test]
    fn test_is_high_precision_mysql_data_type() {
        assert!(is_high_precision_mysql_data_type("bigint"));
        assert!(is_high_precision_mysql_data_type("DECIMAL"));
        assert!(is_high_precision_mysql_data_type("numeric"));
        assert!(!is_high_precision_mysql_data_type("int"));
        assert!(!is_high_precision_mysql_data_type("varchar"));
    }

    #[test]
    fn test_is_high_precision_mysql_query_type() {
        assert!(is_high_precision_mysql_query_type("BIGINT"));
        assert!(is_high_precision_mysql_query_type("BIGINT UNSIGNED"));
        assert!(is_high_precision_mysql_query_type("DECIMAL(18,2)"));
        assert!(!is_high_precision_mysql_query_type("INT"));
    }

    #[test]
    fn test_normalize_mysql_row_json_stringifies_high_precision_numbers() {
        let mut row = serde_json::json!({
            "id": 9223372036854775807_i64,
            "amount": 1234.56,
            "name": "demo",
            "nullable": null
        });
        let high_precision_cols = HashSet::from(["ID".to_string(), "amount".to_string()]);

        normalize_mysql_row_json(&mut row, &high_precision_cols).unwrap();

        assert_eq!(
            row.get("id").and_then(|v| v.as_str()),
            Some("9223372036854775807")
        );
        assert_eq!(row.get("amount").and_then(|v| v.as_str()), Some("1234.56"));
        assert_eq!(row.get("name").and_then(|v| v.as_str()), Some("demo"));
        assert!(row.get("nullable").unwrap().is_null());
    }

    #[test]
    fn test_build_mysql_json_projection_query_strips_trailing_semicolon() {
        let sql = build_mysql_json_projection_query("SELECT * FROM t LIMIT 1000;", "JSON_OBJECT()");
        assert!(sql.contains("FROM (SELECT * FROM t LIMIT 1000) AS `__dbpaw_row`"));
        assert!(!sql.contains(";) AS `__dbpaw_row`"));
    }

    #[test]
    fn test_build_mysql_json_projection_query_strips_multiple_trailing_semicolons() {
        let sql = build_mysql_json_projection_query("SELECT * FROM t;;;", "JSON_OBJECT()");
        assert!(sql.contains("FROM (SELECT * FROM t) AS `__dbpaw_row`"));
        assert!(!sql.contains(";) AS `__dbpaw_row`"));
    }

    #[test]
    fn test_is_prepared_protocol_unsupported_error() {
        assert!(is_prepared_protocol_unsupported_error(
            "error returned from database: 1295 (HY000): This command is not supported in the prepared statement protocol yet"
        ));
        assert!(is_prepared_protocol_unsupported_error(
            "prepared statement protocol is unsupported"
        ));
        assert!(!is_prepared_protocol_unsupported_error(
            "syntax error near ...",
        ));
    }
}
