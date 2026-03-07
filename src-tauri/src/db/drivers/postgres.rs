use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::{postgres::PgPoolOptions, Column, Row, TypeInfo};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::ssh::SshTunnel;

pub struct PostgresDriver {
    pub pool: sqlx::PgPool,
    pub ssh_tunnel: Option<SshTunnel>,
}

fn write_temp_cert_file(prefix: &str, pem: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("dbpaw_certs");
    fs::create_dir_all(&dir).map_err(|e| format!("[SSL_CA_WRITE_ERROR] {e}"))?;
    let path = dir.join(format!("{prefix}_{}.pem", uuid::Uuid::new_v4()));
    fs::write(&path, pem).map_err(|e| format!("[SSL_CA_WRITE_ERROR] {e}"))?;
    Ok(path)
}

fn build_dsn(form: &ConnectionForm) -> Result<String, String> {
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
    let mut dsn = format!(
        "postgres://{}:{}@{}:{}/{}",
        username, password, host, port, database
    );

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
            dsn.push_str(&format!(
                "?sslmode=verify-ca&sslrootcert={}",
                ca_path.to_string_lossy()
            ));
        } else {
            dsn.push_str("?sslmode=require");
        }
    }

    Ok(dsn)
}

impl PostgresDriver {
    pub async fn connect(form: &ConnectionForm) -> Result<Self, String> {
        let mut dsn_form = form.clone();
        let mut ssh_tunnel = None;

        if let Some(true) = form.ssh_enabled {
            let tunnel = crate::ssh::start_ssh_tunnel(form)?;
            dsn_form.host = Some("127.0.0.1".to_string());
            dsn_form.port = Some(tunnel.local_port as i64);
            ssh_tunnel = Some(tunnel);
        }

        let dsn = build_dsn(&dsn_form)?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(&dsn)
            .await
            .map_err(|e| format!("[CONN_FAILED] {e}"))?;

        Ok(Self { pool, ssh_tunnel })
    }
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

#[async_trait]
impl DatabaseDriver for PostgresDriver {
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
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let schema = schema.unwrap_or_else(|| "public".to_string());
        let rows = sqlx::query(
            "SELECT table_schema, table_name, table_type \
             FROM information_schema.tables \
             WHERE table_schema = $1 AND table_type IN ('BASE TABLE','VIEW') \
             ORDER BY table_name",
        )
        .bind(&schema)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut res = Vec::new();
        for row in rows {
            res.push(TableInfo {
                schema: decode_postgres_text_cell(&row, 0).unwrap_or_else(|_| schema.clone()),
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
            let name: String = row.try_get(0).unwrap_or_default();
            let comment: Option<String> = row.try_get::<Option<String>, _>(4).unwrap_or(None);
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
                r#type: row.try_get(1).unwrap_or_default(),
                nullable: !not_null,
                default_value: row.try_get::<Option<String>, _>(3).unwrap_or(None),
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

        let query = format!(
            "SELECT * FROM {}.{}{}{} LIMIT $1 OFFSET $2",
            schema, table, where_clause, order_clause
        );
        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] SQL: {} | {}", query, e))?;

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
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut data = Vec::new();
        let mut columns = Vec::new();

        if let Some(first_row) = rows.first() {
            for col in first_row.columns() {
                columns.push(QueryColumn {
                    name: col.name().to_string(),
                    r#type: col.type_info().to_string(),
                });
            }
        }

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

        let duration = start.elapsed();
        Ok(QueryResult {
            data,
            row_count: rows.len() as i64,
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
}
