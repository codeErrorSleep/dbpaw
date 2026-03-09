use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, QueryColumn, QueryResult, SchemaOverview,
    TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use duckdb::{
    types::{TimeUnit, Value as DuckValue, ValueRef},
    Connection, Row,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DuckdbDriver {
    file_path: String,
}

fn build_file_path(form: &ConnectionForm) -> Result<String, String> {
    form.file_path
        .clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or("[VALIDATION_ERROR] file_path cannot be empty".to_string())
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn duckdb_schema_name(schema: &str) -> String {
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

fn duckdb_table_ref(schema: &str, table: &str) -> String {
    let schema_name = duckdb_schema_name(schema);
    if schema_name == "main" {
        quote_ident(table)
    } else {
        format!("{}.{}", quote_ident(&schema_name), quote_ident(table))
    }
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

fn number_from_f64(v: f64) -> serde_json::Value {
    serde_json::Number::from_f64(v)
        .map(serde_json::Value::Number)
        .unwrap_or(serde_json::Value::Null)
}

fn format_date32(days_since_epoch: i32) -> String {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid epoch date");
    epoch
        .checked_add_signed(Duration::days(days_since_epoch.into()))
        .unwrap_or(epoch)
        .format("%F")
        .to_string()
}

fn format_timestamp(unit: TimeUnit, value: i64) -> String {
    let micros = unit.to_micros(value);
    let seconds = micros.div_euclid(1_000_000);
    let nanos = (micros.rem_euclid(1_000_000) as u32) * 1_000;
    DateTime::<Utc>::from_timestamp(seconds, nanos)
        .map(|dt| dt.naive_utc().format("%F %T%.f").to_string())
        .unwrap_or_else(|| value.to_string())
}

fn format_time64(unit: TimeUnit, value: i64) -> String {
    let micros = unit.to_micros(value);
    let micros_per_day = 86_400_i64 * 1_000_000_i64;
    let normalized_micros = micros.rem_euclid(micros_per_day);
    let seconds = (normalized_micros / 1_000_000) as u32;
    let nanos = ((normalized_micros % 1_000_000) as u32) * 1_000;
    NaiveTime::from_num_seconds_from_midnight_opt(seconds, nanos)
        .map(|t| t.format("%T%.f").to_string())
        .unwrap_or_else(|| value.to_string())
}

fn duckdb_value_key_to_string(value: &DuckValue) -> String {
    match duckdb_value_to_json(value) {
        serde_json::Value::String(v) => v,
        serde_json::Value::Number(v) => v.to_string(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn duckdb_value_to_json(value: &DuckValue) -> serde_json::Value {
    match value {
        DuckValue::Null => serde_json::Value::Null,
        DuckValue::Boolean(v) => serde_json::Value::Bool(*v),
        DuckValue::TinyInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::SmallInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::Int(v) => serde_json::Value::String(v.to_string()),
        DuckValue::BigInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::HugeInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::UTinyInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::USmallInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::UInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::UBigInt(v) => serde_json::Value::String(v.to_string()),
        DuckValue::Float(v) => number_from_f64((*v).into()),
        DuckValue::Double(v) => number_from_f64(*v),
        DuckValue::Decimal(v) => serde_json::Value::String(v.to_string()),
        DuckValue::Timestamp(unit, v) => serde_json::Value::String(format_timestamp(*unit, *v)),
        DuckValue::Date32(v) => serde_json::Value::String(format_date32(*v)),
        DuckValue::Time64(unit, v) => serde_json::Value::String(format_time64(*unit, *v)),
        DuckValue::Text(v) => serde_json::Value::String(v.to_string()),
        DuckValue::Blob(v) => serde_json::Value::String(String::from_utf8_lossy(v).to_string()),
        DuckValue::Interval {
            months,
            days,
            nanos,
        } => {
            let mut obj = serde_json::Map::new();
            obj.insert("months".to_string(), serde_json::Value::from(*months));
            obj.insert("days".to_string(), serde_json::Value::from(*days));
            obj.insert(
                "nanos".to_string(),
                serde_json::Value::String(nanos.to_string()),
            );
            serde_json::Value::Object(obj)
        }
        DuckValue::List(items) | DuckValue::Array(items) => {
            serde_json::Value::Array(items.iter().map(duckdb_value_to_json).collect())
        }
        DuckValue::Enum(v) => serde_json::Value::String(v.to_string()),
        DuckValue::Struct(fields) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in fields.iter() {
                obj.insert(k.clone(), duckdb_value_to_json(v));
            }
            serde_json::Value::Object(obj)
        }
        DuckValue::Map(entries) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in entries.iter() {
                obj.insert(duckdb_value_key_to_string(k), duckdb_value_to_json(v));
            }
            serde_json::Value::Object(obj)
        }
        DuckValue::Union(v) => duckdb_value_to_json(v),
    }
}

fn duckdb_cell_to_json(row: &Row<'_>, idx: usize) -> serde_json::Value {
    let value = match row.get_ref(idx) {
        Ok(v) => v,
        Err(_) => return serde_json::Value::Null,
    };

    match value {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Boolean(v) => serde_json::Value::Bool(v),
        ValueRef::TinyInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::SmallInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::Int(v) => serde_json::Value::String(v.to_string()),
        ValueRef::BigInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::HugeInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::UTinyInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::USmallInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::UInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::UBigInt(v) => serde_json::Value::String(v.to_string()),
        ValueRef::Float(v) => number_from_f64(v.into()),
        ValueRef::Double(v) => number_from_f64(v),
        ValueRef::Decimal(v) => serde_json::Value::String(v.to_string()),
        ValueRef::Timestamp(unit, v) => serde_json::Value::String(format_timestamp(unit, v)),
        ValueRef::Date32(v) => serde_json::Value::String(format_date32(v)),
        ValueRef::Time64(unit, v) => serde_json::Value::String(format_time64(unit, v)),
        ValueRef::Text(v) => serde_json::Value::String(String::from_utf8_lossy(v).to_string()),
        ValueRef::Blob(v) => serde_json::Value::String(String::from_utf8_lossy(v).to_string()),
        ValueRef::Interval {
            months,
            days,
            nanos,
        } => {
            let mut obj = serde_json::Map::new();
            obj.insert("months".to_string(), serde_json::Value::from(months));
            obj.insert("days".to_string(), serde_json::Value::from(days));
            obj.insert(
                "nanos".to_string(),
                serde_json::Value::String(nanos.to_string()),
            );
            serde_json::Value::Object(obj)
        }
        ValueRef::List(_, _)
        | ValueRef::Enum(_, _)
        | ValueRef::Struct(_, _)
        | ValueRef::Array(_, _)
        | ValueRef::Map(_, _)
        | ValueRef::Union(_, _) => duckdb_value_to_json(&value.to_owned()),
    }
}

impl DuckdbDriver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let file_path = build_file_path(form)?;
        let open_path = file_path.clone();
        tokio::task::spawn_blocking(move || {
            Connection::open(&open_path)
                .map(|_| ())
                .map_err(|e| format!("[CONN_FAILED] {e}"))
        })
        .await
        .map_err(|e| format!("[CONN_FAILED] join error: {e}"))??;

        Ok(Self { file_path })
    }

    async fn run_blocking<T, F>(&self, f: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, String> + Send + 'static,
    {
        let file_path = self.file_path.clone();
        tokio::task::spawn_blocking(move || {
            let conn = Connection::open(&file_path).map_err(|e| format!("[CONN_FAILED] {e}"))?;
            f(&conn)
        })
        .await
        .map_err(|e| format!("[QUERY_ERROR] join error: {e}"))?
    }
}

#[async_trait]
impl DatabaseDriver for DuckdbDriver {
    async fn close(&self) {}

    async fn test_connection(&self) -> Result<(), String> {
        self.run_blocking(|conn| {
            conn.execute("SELECT 1", [])
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            Ok(())
        })
        .await
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        self.run_blocking(|conn| {
            let mut out = Vec::new();
            if let Ok(mut stmt) = conn.prepare("SELECT database_name FROM duckdb_databases()") {
                let mut rows = stmt.query([]).map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                while let Some(row) = rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                    let db_name = row
                        .get::<usize, String>(0)
                        .unwrap_or_else(|_| "main".to_string());
                    out.push(db_name);
                }
            }

            if out.is_empty() {
                out.push("main".to_string());
            }
            out.sort();
            out.dedup();
            Ok(out)
        })
        .await
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        self.run_blocking(move |conn| {
            let schema_filter = schema
                .as_deref()
                .map(duckdb_schema_name)
                .filter(|v| !v.trim().is_empty());

            let sql = if let Some(schema_name) = schema_filter {
                format!(
                    "SELECT table_schema, table_name, table_type \
                     FROM information_schema.tables \
                     WHERE table_schema = {} \
                       AND table_schema NOT IN ('pg_catalog', 'information_schema') \
                     ORDER BY table_schema, table_name",
                    quote_literal(&schema_name)
                )
            } else {
                "SELECT table_schema, table_name, table_type \
                 FROM information_schema.tables \
                 WHERE table_schema NOT IN ('pg_catalog', 'information_schema') \
                 ORDER BY table_schema, table_name"
                    .to_string()
            };

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut rows = stmt.query([]).map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut tables = Vec::new();

            while let Some(row) = rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                let schema_name = row
                    .get::<usize, String>(0)
                    .unwrap_or_else(|_| "main".to_string());
                let table_name = row.get::<usize, String>(1).unwrap_or_default();
                let table_type = row
                    .get::<usize, String>(2)
                    .unwrap_or_else(|_| "BASE TABLE".to_string());

                if table_name.is_empty() {
                    continue;
                }

                tables.push(TableInfo {
                    schema: duckdb_schema_name(&schema_name),
                    name: table_name,
                    r#type: if table_type.eq_ignore_ascii_case("view") {
                        "view".to_string()
                    } else {
                        "table".to_string()
                    },
                });
            }

            Ok(tables)
        })
        .await
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        self.run_blocking(move |conn| {
            let schema_name = duckdb_schema_name(&schema);
            let sql = format!(
                "SELECT column_name, data_type, is_nullable, column_default \
                 FROM information_schema.columns \
                 WHERE table_schema = {} AND table_name = {} \
                 ORDER BY ordinal_position",
                quote_literal(&schema_name),
                quote_literal(&table)
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut rows = stmt.query([]).map_err(|e| format!("[QUERY_ERROR] {e}"))?;

            let pk_sql = format!(
                "SELECT kcu.column_name \
                 FROM information_schema.table_constraints tc \
                 JOIN information_schema.key_column_usage kcu \
                   ON tc.constraint_name = kcu.constraint_name \
                  AND tc.table_schema = kcu.table_schema \
                  AND tc.table_name = kcu.table_name \
                 WHERE tc.constraint_type = 'PRIMARY KEY' \
                   AND tc.table_schema = {} \
                   AND tc.table_name = {}",
                quote_literal(&schema_name),
                quote_literal(&table)
            );
            let mut pk_stmt = conn
                .prepare(&pk_sql)
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut pk_rows = pk_stmt
                .query([])
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut pk_cols = std::collections::HashSet::new();
            while let Some(row) = pk_rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                let col_name = row.get::<usize, String>(0).unwrap_or_default();
                if !col_name.is_empty() {
                    pk_cols.insert(col_name);
                }
            }

            let mut columns = Vec::new();
            while let Some(row) = rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                let name = row.get::<usize, String>(0).unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let type_name = row.get::<usize, String>(1).unwrap_or_default();
                let is_nullable = row
                    .get::<usize, String>(2)
                    .unwrap_or_else(|_| "YES".to_string());
                let default_value = row.get::<usize, Option<String>>(3).unwrap_or(None);

                columns.push(ColumnInfo {
                    name: name.clone(),
                    r#type: type_name,
                    nullable: is_nullable.eq_ignore_ascii_case("yes"),
                    default_value,
                    primary_key: pk_cols.contains(&name),
                    comment: None,
                });
            }

            Ok(TableStructure { columns })
        })
        .await
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

        Ok(TableMetadata {
            columns,
            indexes: vec![],
            foreign_keys: vec![],
            clickhouse_extra: None,
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        self.run_blocking(move |conn| {
            let schema_name = duckdb_schema_name(&schema);
            let sql = format!(
                "SELECT sql FROM duckdb_tables() \
                 WHERE schema_name = {} AND table_name = {} \
                 LIMIT 1",
                quote_literal(&schema_name),
                quote_literal(&table)
            );
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            let mut rows = stmt.query([]).map_err(|e| format!("[QUERY_ERROR] {e}"))?;
            if let Some(row) = rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                let ddl = row.get::<usize, Option<String>>(0).unwrap_or(None);
                if let Some(ddl) = ddl.filter(|v| !v.trim().is_empty()) {
                    return Ok(ddl);
                }
            }

            Err(format!("[QUERY_ERROR] Failed to read DDL for '{}'", table))
        })
        .await
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
        self.run_blocking(move |conn| {
            let start = std::time::Instant::now();
            let safe_page = if page < 1 { 1 } else { page };
            let safe_limit = if limit < 1 { 100 } else { limit };
            let offset = (safe_page - 1) * safe_limit;
            let table_ref = duckdb_table_ref(&schema, &table);

            let filter = filter.map(|f| super::normalize_quotes(&f));
            let order_by = order_by.map(|f| super::normalize_quotes(&f));

            let where_clause = match &filter {
                Some(f) if !f.trim().is_empty() => format!(" WHERE {}", f.trim()),
                _ => String::new(),
            };

            let count_query = format!("SELECT COUNT(*) FROM {}{}", table_ref, where_clause);
            let total: i64 = conn
                .query_row(&count_query, [], |row| row.get(0))
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
                "SELECT * FROM {}{}{} LIMIT {} OFFSET {}",
                table_ref, where_clause, order_clause, safe_limit, offset
            );
            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", query, e))?;
            let mut rows = stmt
                .query([])
                .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", query, e))?;
            let col_names: Vec<String> =
                rows.as_ref().map(|s| s.column_names()).unwrap_or_default();

            let mut data = Vec::new();
            while let Some(row) = rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                let mut obj = serde_json::Map::new();
                for (idx, name) in col_names.iter().enumerate() {
                    obj.insert(name.to_string(), duckdb_cell_to_json(row, idx));
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
        })
        .await
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
        self.run_blocking(move |conn| {
            let start = std::time::Instant::now();
            let first_keyword = first_sql_keyword(&sql);
            let sql_lower = sql.to_ascii_lowercase();
            let should_fetch_rows = matches!(
                first_keyword.as_deref(),
                Some("select") | Some("pragma") | Some("with") | Some("explain")
            ) || sql_lower.contains(" returning ");

            if should_fetch_rows {
                let mut stmt = conn
                    .prepare(&sql)
                    .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                let mut rows = stmt.query([]).map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                let columns: Vec<QueryColumn> = rows
                    .as_ref()
                    .map(|s| {
                        s.column_names()
                            .into_iter()
                            .map(|name| QueryColumn {
                                name,
                                r#type: "UNKNOWN".to_string(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let mut data = Vec::new();
                while let Some(row) = rows.next().map_err(|e| format!("[QUERY_ERROR] {e}"))? {
                    let mut obj = serde_json::Map::new();
                    for (idx, col) in columns.iter().enumerate() {
                        obj.insert(col.name.clone(), duckdb_cell_to_json(row, idx));
                    }
                    data.push(serde_json::Value::Object(obj));
                }

                return Ok(QueryResult {
                    row_count: data.len() as i64,
                    data,
                    columns,
                    time_taken_ms: start.elapsed().as_millis() as i64,
                    success: true,
                    error: None,
                });
            }

            let row_count = match conn.execute(&sql, []) {
                Ok(v) => v as i64,
                Err(_) => {
                    conn.execute_batch(&sql)
                        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
                    0
                }
            };

            Ok(QueryResult {
                data: vec![],
                row_count,
                columns: vec![],
                time_taken_ms: start.elapsed().as_millis() as i64,
                success: true,
                error: None,
            })
        })
        .await
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String> {
        let target_schema = duckdb_schema_name(schema.as_deref().unwrap_or("main"));
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
        p.push(format!("dbpaw-duckdb-test-{}.duckdb", Uuid::new_v4()));
        p.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn test_connect_validation_error() {
        let form = ConnectionForm {
            driver: "duckdb".to_string(),
            file_path: None,
            ..Default::default()
        };
        let result = DuckdbDriver::connect(&form).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_execute_query_select_and_dml() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "duckdb".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = DuckdbDriver::connect(&form).await.unwrap();
        driver
            .execute_query("CREATE TABLE items (id INTEGER, name VARCHAR);".to_string())
            .await
            .unwrap();

        let insert_result = driver
            .execute_query("INSERT INTO items VALUES (1, 'a'), (2, 'b');".to_string())
            .await
            .unwrap();
        assert!(insert_result.row_count >= 0);

        let select_result = driver
            .execute_query("SELECT id, name FROM items ORDER BY id;".to_string())
            .await
            .unwrap();
        assert_eq!(select_result.row_count, 2);
        assert_eq!(select_result.columns.len(), 2);

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_list_tables_metadata_and_ddl() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "duckdb".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = DuckdbDriver::connect(&form).await.unwrap();
        driver
            .execute_query(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name VARCHAR, age INTEGER);"
                    .to_string(),
            )
            .await
            .unwrap();

        let tables = driver.list_tables(None).await.unwrap();
        assert!(tables.iter().any(|t| t.name == "users"));

        let structure = driver
            .get_table_structure("main".to_string(), "users".to_string())
            .await
            .unwrap();
        assert!(structure.columns.iter().any(|c| c.name == "name"));

        let ddl = driver
            .get_table_ddl("main".to_string(), "users".to_string())
            .await
            .unwrap();
        assert!(ddl.to_lowercase().contains("create table"));

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_duckdb_cell_to_json_preserves_decimal_and_temporal_values() {
        let path = temp_db_path();
        let form = ConnectionForm {
            driver: "duckdb".to_string(),
            file_path: Some(path.clone()),
            ..Default::default()
        };

        let driver = DuckdbDriver::connect(&form).await.unwrap();
        driver
            .execute_query(
                "CREATE TABLE products (\
                    product_id INTEGER PRIMARY KEY, \
                    price DECIMAL(10,2), \
                    created_date DATE, \
                    created_at TIMESTAMP\
                );"
                .to_string(),
            )
            .await
            .unwrap();
        driver
            .execute_query(
                "INSERT INTO products (product_id, price, created_date, created_at) VALUES \
                 (1, 4236.50, '2026-01-02', '2026-01-02 03:04:05');"
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
        assert_eq!(table_data.data.len(), 1);
        let row = table_data.data.first().unwrap();
        let price = row["price"].as_str().expect("price should be a string");
        assert!(price.contains('.'));
        assert_eq!(price.parse::<f64>().unwrap(), 4236.5);
        assert_eq!(
            row["created_date"],
            serde_json::Value::String("2026-01-02".to_string())
        );
        assert_ne!(
            row["created_date"],
            serde_json::Value::String("20456".to_string())
        );
        let created_at = row["created_at"]
            .as_str()
            .expect("created_at should be a string");
        assert!(created_at.contains("2026-01-02"));

        let query_result = driver
            .execute_query(
                "SELECT price, created_date, created_at FROM products WHERE product_id = 1;"
                    .to_string(),
            )
            .await
            .unwrap();
        assert_eq!(query_result.row_count, 1);
        let query_row = query_result.data.first().unwrap();
        let query_price = query_row["price"].as_str().unwrap();
        assert!(query_price.contains('.'));
        assert_eq!(query_price.parse::<f64>().unwrap(), 4236.5);
        assert_eq!(
            query_row["created_date"],
            serde_json::Value::String("2026-01-02".to_string())
        );
        let query_created_at = query_row["created_at"]
            .as_str()
            .expect("created_at should be a string");
        assert!(query_created_at.contains("2026-01-02"));

        driver.close().await;
        let _ = std::fs::remove_file(path);
    }
}
