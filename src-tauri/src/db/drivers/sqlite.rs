use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, Column, Executor, Row, TypeInfo, ValueRef};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SqliteDriver {
    pub pool: sqlx::SqlitePool,
}

fn build_dsn(form: &ConnectionForm) -> Result<String, String> {
    let file_path = form
        .file_path
        .clone()
        .filter(|v| !v.trim().is_empty())
        .ok_or("[VALIDATION_ERROR] file_path cannot be empty")?;
    Ok(format!("sqlite://{}?mode=rwc", file_path))
}

impl SqliteDriver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let dsn = build_dsn(form)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(&dsn)
            .await
            .map_err(|e| format!("[CONN_FAILED] {e}"))?;

        Ok(Self { pool })
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
}

fn sqlite_temporal_decl_kind(declared_type: Option<&str>) -> Option<&'static str> {
    let ty = declared_type?.trim().to_ascii_lowercase();
    if ty.contains("datetime") || ty.contains("timestamp") {
        return Some("datetime");
    }
    if ty == "time" || ty.contains(" time") || ty.starts_with("time(") {
        return Some("time");
    }
    if ty.contains("date") {
        return Some("date");
    }
    None
}

fn sqlite_declared_bool(declared_type: Option<&str>) -> bool {
    declared_type
        .map(|ty| {
            let lower = ty.to_ascii_lowercase();
            lower.contains("bool")
        })
        .unwrap_or(false)
}

fn sqlite_format_date_from_days(days_since_epoch: i64) -> Option<String> {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)?;
    epoch
        .checked_add_signed(Duration::days(days_since_epoch))
        .map(|d| d.format("%F").to_string())
}

fn sqlite_format_time_from_seconds_f64(seconds: f64) -> Option<String> {
    let day_secs = 86_400.0_f64;
    let normalized = seconds.rem_euclid(day_secs);
    let sec_int = normalized.trunc() as u32;
    let nanos = ((normalized.fract() * 1_000_000_000.0).round() as u32).min(999_999_999);
    NaiveTime::from_num_seconds_from_midnight_opt(sec_int, nanos)
        .map(|t| t.format("%T%.f").to_string())
}

fn sqlite_format_datetime_from_unix_seconds_f64(seconds: f64) -> Option<String> {
    let sec_int = seconds.trunc() as i64;
    let nanos = ((seconds.fract() * 1_000_000_000.0).round() as u32).min(999_999_999);
    DateTime::<Utc>::from_timestamp(sec_int, nanos)
        .map(|dt| dt.naive_utc().format("%F %T%.f").to_string())
}

fn sqlite_normalize_temporal_text(value: &str, temporal_kind: &str) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }

    match temporal_kind {
        "date" => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .map(|d| d.format("%F").to_string()),
        "time" => {
            for fmt in ["%H:%M:%S%.f", "%H:%M:%S"] {
                if let Ok(t) = NaiveTime::parse_from_str(text, fmt) {
                    return Some(t.format("%T%.f").to_string());
                }
            }
            None
        }
        "datetime" => {
            if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
                return Some(dt.to_rfc3339());
            }
            for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S%.f"] {
                if let Ok(dt) = NaiveDateTime::parse_from_str(text, fmt) {
                    return Some(dt.format("%F %T%.f").to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn sqlite_cell_to_json(
    row: &sqlx::sqlite::SqliteRow,
    column_name: &str,
    declared_type: Option<&str>,
) -> serde_json::Value {
    let temporal_kind = sqlite_temporal_decl_kind(declared_type);
    let declared_bool = sqlite_declared_bool(declared_type);

    let runtime_type = row
        .try_get_raw(column_name)
        .ok()
        .map(|v| v.type_info().name().to_string())
        .unwrap_or_else(|| "NULL".to_string());

    match runtime_type.as_str() {
        "NULL" => serde_json::Value::Null,
        "INTEGER" => {
            let value = row.try_get::<Option<i64>, _>(column_name).unwrap_or(None);
            match value {
                None => serde_json::Value::Null,
                Some(v) => {
                    if declared_bool {
                        serde_json::Value::Bool(v != 0)
                    } else if let Some(kind) = temporal_kind {
                        match kind {
                            "date" => {
                                let maybe_date = if (-200_000..=200_000).contains(&v) {
                                    sqlite_format_date_from_days(v)
                                } else {
                                    sqlite_format_datetime_from_unix_seconds_f64(v as f64).and_then(
                                        |s| {
                                            NaiveDateTime::parse_from_str(
                                                &s,
                                                "%Y-%m-%d %H:%M:%S%.f",
                                            )
                                            .ok()
                                            .map(|dt| dt.date().format("%F").to_string())
                                        },
                                    )
                                };
                                maybe_date
                                    .map(serde_json::Value::String)
                                    .unwrap_or_else(|| serde_json::Value::String(v.to_string()))
                            }
                            "time" => sqlite_format_time_from_seconds_f64(v as f64)
                                .map(serde_json::Value::String)
                                .unwrap_or_else(|| serde_json::Value::String(v.to_string())),
                            "datetime" => sqlite_format_datetime_from_unix_seconds_f64(v as f64)
                                .map(serde_json::Value::String)
                                .unwrap_or_else(|| serde_json::Value::String(v.to_string())),
                            _ => serde_json::Value::String(v.to_string()),
                        }
                    } else {
                        serde_json::Value::String(v.to_string())
                    }
                }
            }
        }
        "REAL" => {
            let value = row.try_get::<Option<f64>, _>(column_name).unwrap_or(None);
            match value {
                None => serde_json::Value::Null,
                Some(v) => {
                    if let Some(kind) = temporal_kind {
                        match kind {
                            "time" => sqlite_format_time_from_seconds_f64(v)
                                .map(serde_json::Value::String)
                                .unwrap_or_else(|| {
                                    serde_json::Number::from_f64(v)
                                        .map(serde_json::Value::Number)
                                        .unwrap_or(serde_json::Value::Null)
                                }),
                            "datetime" => sqlite_format_datetime_from_unix_seconds_f64(v)
                                .map(serde_json::Value::String)
                                .unwrap_or_else(|| {
                                    serde_json::Number::from_f64(v)
                                        .map(serde_json::Value::Number)
                                        .unwrap_or(serde_json::Value::Null)
                                }),
                            "date" => sqlite_format_datetime_from_unix_seconds_f64(v)
                                .and_then(|s| {
                                    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f")
                                        .ok()
                                        .map(|dt| dt.date().format("%F").to_string())
                                })
                                .map(serde_json::Value::String)
                                .unwrap_or_else(|| {
                                    serde_json::Number::from_f64(v)
                                        .map(serde_json::Value::Number)
                                        .unwrap_or(serde_json::Value::Null)
                                }),
                            _ => serde_json::Number::from_f64(v)
                                .map(serde_json::Value::Number)
                                .unwrap_or(serde_json::Value::Null),
                        }
                    } else {
                        serde_json::Number::from_f64(v)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null)
                    }
                }
            }
        }
        "TEXT" => row
            .try_get::<Option<String>, _>(column_name)
            .ok()
            .flatten()
            .map(|s| {
                if let Some(kind) = temporal_kind {
                    sqlite_normalize_temporal_text(&s, kind)
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::String(s))
                } else {
                    serde_json::Value::String(s)
                }
            })
            .unwrap_or(serde_json::Value::Null),
        "BLOB" => row
            .try_get::<Option<Vec<u8>>, _>(column_name)
            .ok()
            .flatten()
            .map(|x| serde_json::Value::String(String::from_utf8_lossy(&x).to_string()))
            .unwrap_or(serde_json::Value::Null),
        _ => row
            .try_get::<Option<String>, _>(column_name)
            .ok()
            .flatten()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null),
    }
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('\"', "\"\""))
}

fn sqlite_table_ref(schema: &str, table: &str) -> String {
    let table_quoted = quote_ident(table);
    let schema_name = sqlite_schema_name(schema);
    if schema_name == "main" {
        table_quoted
    } else {
        format!("{}.{}", quote_ident(&schema_name), table_quoted)
    }
}

fn sqlite_schema_name(schema: &str) -> String {
    let trimmed = schema.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("public")
        || trimmed.eq_ignore_ascii_case("main")
    {
        "main".to_string()
    } else {
        trimmed.to_string()
    }
}

fn sqlite_master_ref(schema: &str) -> String {
    let schema_name = sqlite_schema_name(schema);
    if schema_name == "main" {
        "sqlite_master".to_string()
    } else {
        format!("{}.sqlite_master", quote_ident(&schema_name))
    }
}

fn pragma_table_info_sql(schema: &str, table: &str) -> String {
    format!(
        "PRAGMA {}.table_info({})",
        quote_ident(&sqlite_schema_name(schema)),
        quote_ident(table)
    )
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

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn close(&self) {
        self.pool.close().await;
    }

    async fn test_connection(&self) -> Result<(), String> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        Ok(vec!["main".to_string()])
    }

    async fn list_tables(&self, _schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let schema = _schema.unwrap_or_else(|| "main".to_string());
        let master_ref = sqlite_master_ref(&schema);
        let rows = sqlx::query(&format!(
            "SELECT name, type \
             FROM {} \
             WHERE type IN ('table', 'view') \
               AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
            master_ref
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut tables = Vec::new();
        for row in rows {
            let table_type: String = row.try_get("type").unwrap_or_else(|_| "table".to_string());
            tables.push(TableInfo {
                schema: sqlite_schema_name(&schema),
                name: row.try_get("name").unwrap_or_default(),
                r#type: if table_type.eq_ignore_ascii_case("view") {
                    "view".to_string()
                } else {
                    "table".to_string()
                },
            });
        }

        Ok(tables)
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let rows = sqlx::query(&pragma_table_info_sql(&schema, &table))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut columns = Vec::new();
        for row in rows {
            let nullable = row.try_get::<i64, _>("notnull").unwrap_or(0) == 0;
            let pk = row.try_get::<i64, _>("pk").unwrap_or(0) > 0;
            columns.push(ColumnInfo {
                name: row.try_get("name").unwrap_or_default(),
                r#type: row.try_get("type").unwrap_or_default(),
                nullable,
                default_value: row
                    .try_get::<Option<String>, _>("dflt_value")
                    .unwrap_or(None),
                primary_key: pk,
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
        let columns = self
            .get_table_structure(schema.clone(), table.clone())
            .await?
            .columns;

        let idx_rows = sqlx::query(&format!(
            "PRAGMA {}.index_list({})",
            quote_ident(&sqlite_schema_name(&schema)),
            quote_ident(&table)
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut indexes = Vec::new();
        for idx_row in idx_rows {
            let index_name: String = idx_row.try_get("name").unwrap_or_default();
            if index_name.is_empty() {
                continue;
            }
            let unique = idx_row.try_get::<i64, _>("unique").unwrap_or(0) == 1;

            let info_rows = sqlx::query(&format!(
                "PRAGMA {}.index_info({})",
                quote_ident(&sqlite_schema_name(&schema)),
                quote_ident(&index_name)
            ))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

            let mut ordered: Vec<(i64, String)> = Vec::new();
            for r in info_rows {
                let seqno = r.try_get::<i64, _>("seqno").unwrap_or(0);
                let col_name: String = r.try_get("name").unwrap_or_default();
                if !col_name.is_empty() {
                    ordered.push((seqno, col_name));
                }
            }
            ordered.sort_by_key(|x| x.0);

            indexes.push(IndexInfo {
                name: index_name,
                unique,
                index_type: Some("btree".to_string()),
                columns: ordered.into_iter().map(|x| x.1).collect(),
            });
        }
        indexes.sort_by(|a, b| a.name.cmp(&b.name));

        let fk_rows = sqlx::query(&format!(
            "PRAGMA {}.foreign_key_list({})",
            quote_ident(&sqlite_schema_name(&schema)),
            quote_ident(&table)
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut foreign_keys = Vec::new();
        for row in fk_rows {
            let id = row.try_get::<i64, _>("id").unwrap_or(0);
            let from_col: String = row.try_get("from").unwrap_or_default();
            let ref_table: String = row.try_get("table").unwrap_or_default();
            let ref_col: String = row.try_get("to").unwrap_or_default();
            if from_col.is_empty() || ref_table.is_empty() {
                continue;
            }
            foreign_keys.push(ForeignKeyInfo {
                name: format!("fk_{}_{}", id, from_col),
                column: from_col,
                referenced_schema: None,
                referenced_table: ref_table,
                referenced_column: ref_col,
                on_update: row
                    .try_get::<Option<String>, _>("on_update")
                    .unwrap_or(None),
                on_delete: row
                    .try_get::<Option<String>, _>("on_delete")
                    .unwrap_or(None),
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
        let row = sqlx::query(&format!(
            "SELECT sql \
             FROM {} \
             WHERE name = ? AND type IN ('table', 'view') \
             LIMIT 1",
            sqlite_master_ref(&schema)
        ))
        .bind(&table)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let row =
            row.ok_or_else(|| format!("[QUERY_ERROR] Table or view '{}' not found", table))?;
        let sql: Option<String> = row.try_get("sql").unwrap_or(None);
        sql.ok_or_else(|| format!("[QUERY_ERROR] Failed to read DDL for '{}'", table))
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
        let safe_page = if page < 1 { 1 } else { page };
        let safe_limit = if limit < 1 { 100 } else { limit };
        let offset = (safe_page - 1) * safe_limit;
        let table_ref = sqlite_table_ref(&schema, &table);

        let filter = filter.map(|f| super::normalize_quotes(&f));
        let order_by = order_by.map(|f| super::normalize_quotes(&f));

        let where_clause = match &filter {
            Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
            _ => String::new(),
        };

        let count_query = format!("SELECT COUNT(*) FROM {}{}", table_ref, where_clause);
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

        let query = format!(
            "SELECT * FROM {}{}{} LIMIT ? OFFSET ?",
            table_ref, where_clause, order_clause
        );
        let rows = sqlx::query(&query)
            .bind(safe_limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", query, e))?;

        let mut data = Vec::new();
        for row in &rows {
            let mut obj = serde_json::Map::new();
            for col in row.columns() {
                let name = col.name();
                obj.insert(
                    name.to_string(),
                    sqlite_cell_to_json(row, name, Some(col.type_info().name())),
                );
            }
            data.push(serde_json::Value::Object(obj));
        }

        let duration = start.elapsed();
        Ok(TableDataResponse {
            data,
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
        let start = std::time::Instant::now();
        let first_keyword = first_sql_keyword(&sql);
        let sql_lower = sql.to_lowercase();
        let should_fetch_rows = matches!(
            first_keyword.as_deref(),
            Some("select") | Some("pragma") | Some("with") | Some("explain")
        ) || sql_lower.contains(" returning ");

        if should_fetch_rows {
            let rows = sqlx::query(&sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

            let mut data = Vec::new();
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
                self.describe_query_columns(&sql).await?
            };

            for row in &rows {
                let mut obj = serde_json::Map::new();
                for col in row.columns() {
                    let name = col.name();
                    obj.insert(
                        name.to_string(),
                        sqlite_cell_to_json(row, name, Some(col.type_info().name())),
                    );
                }
                data.push(serde_json::Value::Object(obj));
            }

            let duration = start.elapsed();
            return Ok(QueryResult {
                data,
                row_count: rows.len() as i64,
                columns,
                time_taken_ms: duration.as_millis() as i64,
                success: true,
                error: None,
            });
        }

        let exec = sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let duration = start.elapsed();
        Ok(QueryResult {
            data: vec![],
            row_count: exec.rows_affected() as i64,
            columns: vec![],
            time_taken_ms: duration.as_millis() as i64,
            success: true,
            error: None,
        })
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let target_schema = sqlite_schema_name(schema.as_deref().unwrap_or("main"));
        let tables = self.list_tables(Some(target_schema.clone())).await?;
        let mut map: HashMap<(String, String), Vec<ColumnSchema>> = HashMap::new();

        for t in tables {
            let structure = self
                .get_table_structure(target_schema.clone(), t.name.clone())
                .await?;
            let cols = structure
                .columns
                .into_iter()
                .map(|c| ColumnSchema {
                    name: c.name,
                    r#type: c.r#type,
                })
                .collect::<Vec<_>>();
            map.insert((target_schema.clone(), t.name), cols);
        }

        let mut out = Vec::new();
        for ((schema_name, table_name), columns) in map {
            out.push(TableSchema {
                schema: schema_name,
                name: table_name,
                columns,
            });
        }
        out.sort_by(|a, b| a.schema.cmp(&b.schema).then(a.name.cmp(&b.name)));
        Ok(SchemaOverview { tables: out })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_db_path() -> String {
        let mut p = std::env::temp_dir();
        p.push(format!("dbpaw-sqlite-test-{}.db", Uuid::new_v4()));
        p.to_string_lossy().to_string()
    }

    #[test]
    fn test_connect_missing_file_path() {
        let _form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: None,
            ..Default::default()
        };
        // We need to run this async, but since it returns a future, we can just check the result behavior
        // Or simpler: verify the logic fails before creating the future if possible,
        // but our logic is inside async connect.
        // So we use tokio::test for this.
    }

    #[tokio::test]
    async fn test_connect_validation_error() {
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: None,
            ..Default::default()
        };
        let result = SqliteDriver::connect(&form).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_connect_with_space_in_path() {
        let mut path = std::env::temp_dir();
        path.push(format!("dbpaw test space {}.db", Uuid::new_v4()));
        let path_str = path.to_string_lossy().to_string();

        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path_str.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form)
            .await
            .expect("Should connect to path with spaces");
        driver
            .test_connection()
            .await
            .expect("Should execute query");
        driver.close().await;

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_connect_and_test_connection() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver.test_connection().await.unwrap();
        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_list_databases_returns_main() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        let dbs = driver.list_databases().await.unwrap();
        assert_eq!(dbs, vec!["main".to_string()]);
        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_list_tables_includes_tables_and_views() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver
            .execute_query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);".to_string())
            .await
            .unwrap();
        driver
            .execute_query("CREATE VIEW users_view AS SELECT name FROM users;".to_string())
            .await
            .unwrap();

        let tables = driver.list_tables(None).await.unwrap();
        assert!(tables
            .iter()
            .any(|t| t.name == "users" && t.r#type == "table"));
        assert!(tables
            .iter()
            .any(|t| t.name == "users_view" && t.r#type == "view"));
        assert!(!tables.iter().any(|t| t.name.starts_with("sqlite_")));
        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_execute_query_select_and_dml() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver
            .execute_query("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);".to_string())
            .await
            .unwrap();

        let insert_result = driver
            .execute_query("INSERT INTO items (name) VALUES ('a'), ('b');".to_string())
            .await
            .unwrap();
        assert_eq!(insert_result.row_count, 2);

        let update_result = driver
            .execute_query("UPDATE items SET name = 'c' WHERE id = 1;".to_string())
            .await
            .unwrap();
        assert_eq!(update_result.row_count, 1);

        let select_result = driver
            .execute_query("SELECT id, name FROM items ORDER BY id;".to_string())
            .await
            .unwrap();
        assert_eq!(select_result.row_count, 2);
        assert_eq!(select_result.columns.len(), 2);
        assert_eq!(
            select_result.data[0]["name"],
            serde_json::Value::String("c".to_string())
        );

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_execute_query_select_with_leading_line_comment() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver
            .execute_query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);".to_string())
            .await
            .unwrap();
        driver
            .execute_query("INSERT INTO users (name) VALUES ('alice'), ('bob');".to_string())
            .await
            .unwrap();

        let result = driver
            .execute_query(
                "-- leading comment\nSELECT id, name FROM users ORDER BY id DESC;".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(result.row_count, 2);
        assert_eq!(result.columns.len(), 2);
        assert_eq!(
            result.data[0]["name"],
            serde_json::Value::String("bob".to_string())
        );

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_execute_query_empty_result_keeps_columns() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver
            .execute_query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);".to_string())
            .await
            .unwrap();
        driver
            .execute_query("INSERT INTO users (name) VALUES ('alice'), ('bob');".to_string())
            .await
            .unwrap();

        let result = driver
            .execute_query("SELECT id, name FROM users WHERE id < 0;".to_string())
            .await
            .unwrap();

        assert_eq!(result.row_count, 0);
        assert_eq!(result.data.len(), 0);
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "name");

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_get_table_data_supports_public_schema_alias() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver
            .execute_query(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); \
                 INSERT INTO users (name) VALUES ('alice'), ('bob');"
                    .to_string(),
            )
            .await
            .unwrap();

        let result = driver
            .get_table_data(
                "public".to_string(),
                "users".to_string(),
                1,
                100,
                Some("id".to_string()),
                Some("asc".to_string()),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.total, 2);
        assert_eq!(result.data.len(), 2);
        assert_eq!(
            result.data[0]["name"],
            serde_json::Value::String("alice".to_string())
        );

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_metadata_ddl_and_schema_overview() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };
        let driver = SqliteDriver::connect(&form).await.unwrap();

        driver
            .execute_query(
                "CREATE TABLE parents (id INTEGER PRIMARY KEY, name TEXT); \
                 CREATE TABLE children (id INTEGER PRIMARY KEY, parent_id INTEGER, name TEXT, \
                 FOREIGN KEY(parent_id) REFERENCES parents(id)); \
                 CREATE INDEX idx_children_name ON children(name);"
                    .to_string(),
            )
            .await
            .unwrap();

        let structure = driver
            .get_table_structure("public".to_string(), "children".to_string())
            .await
            .unwrap();
        assert!(structure
            .columns
            .iter()
            .any(|c| c.name == "id" && c.primary_key));

        let metadata = driver
            .get_table_metadata("public".to_string(), "children".to_string())
            .await
            .unwrap();
        assert!(metadata.columns.iter().any(|c| c.name == "parent_id"));
        assert!(metadata
            .indexes
            .iter()
            .any(|i| i.name == "idx_children_name"));
        assert!(metadata
            .foreign_keys
            .iter()
            .any(|fk| fk.column == "parent_id" && fk.referenced_table == "parents"));

        let ddl = driver
            .get_table_ddl("public".to_string(), "children".to_string())
            .await
            .unwrap();
        assert!(ddl.to_lowercase().contains("create table"));

        let overview = driver.get_schema_overview(None).await.unwrap();
        assert!(overview.tables.iter().any(|t| t.name == "children"));

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_sqlite_raw_dispatch_and_temporal_normalization() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = SqliteDriver::connect(&form).await.unwrap();
        driver
            .execute_query(
                "CREATE TABLE products (\
                    id INTEGER PRIMARY KEY, \
                    price NUMERIC, \
                    created_date DATE, \
                    created_at DATETIME, \
                    is_active BOOLEAN\
                );"
                .to_string(),
            )
            .await
            .unwrap();
        driver
            .execute_query(
                "INSERT INTO products (id, price, created_date, created_at, is_active) \
                 VALUES (1, 4236.50, '2026-01-02', '2026-01-02T03:04:05.120Z', 1);"
                    .to_string(),
            )
            .await
            .unwrap();

        let table_data = driver
            .get_table_data(
                "main".to_string(),
                "products".to_string(),
                1,
                100,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(table_data.total, 1);
        let row = table_data.data.first().unwrap();
        assert!(
            row["price"].is_number(),
            "price should keep numeric semantics"
        );
        assert_eq!(
            row["created_date"],
            serde_json::Value::String("2026-01-02".to_string())
        );
        assert!(
            row["created_at"]
                .as_str()
                .map(|v| v.starts_with("2026-01-02T03:04:05"))
                .unwrap_or(false),
            "created_at should be normalized"
        );
        assert_eq!(row["is_active"], serde_json::Value::Bool(true));

        let query_result = driver
            .execute_query(
                "SELECT price, created_date, created_at, is_active FROM products WHERE id = 1;"
                    .to_string(),
            )
            .await
            .unwrap();
        assert_eq!(query_result.row_count, 1);
        let query_row = query_result.data.first().unwrap();
        assert!(query_row["price"].is_number());
        assert_eq!(
            query_row["created_date"],
            serde_json::Value::String("2026-01-02".to_string())
        );
        assert!(query_row["created_at"]
            .as_str()
            .map(|v| v.starts_with("2026-01-02T03:04:05"))
            .unwrap_or(false));
        assert_eq!(query_row["is_active"], serde_json::Value::Bool(true));

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }
}
