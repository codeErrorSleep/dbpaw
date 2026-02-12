use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use sqlx::{postgres::PgPoolOptions, Column, Row, TypeInfo};
use std::collections::HashSet;

pub struct PostgresDriver {
    pub form: ConnectionForm,
}

impl PostgresDriver {
    fn conn_string(&self) -> Result<String, String> {
        let host = self
            .form
            .host
            .clone()
            .ok_or("[VALIDATION_ERROR] host 不能为空")?;
        let port = self.form.port.unwrap_or(5432);
        // 允许 database 为空，默认为 postgres
        let database = self
            .form
            .database
            .clone()
            .unwrap_or_else(|| "postgres".to_string());
        let username = self
            .form
            .username
            .clone()
            .ok_or("[VALIDATION_ERROR] username 不能为空")?;
        let password = self
            .form
            .password
            .clone()
            .ok_or("[VALIDATION_ERROR] password 不能为空")?;
        Ok(format!(
            "postgres://{}:{}@{}:{}/{}",
            username, password, host, port, database
        ))
    }

    async fn get_pool(&self) -> Result<sqlx::PgPool, String> {
        let dsn = self.conn_string()?;
        PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&dsn)
            .await
            .map_err(|e| format!("[CONN_FAILED] {e}"))
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn test_connection(&self) -> Result<(), String> {
        let pool = self.get_pool().await?;
        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let pool = self.get_pool().await?;
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let pool = self.get_pool().await?;
        let schema = schema.unwrap_or_else(|| "public".to_string());
        let rows = sqlx::query(
            "SELECT table_schema, table_name, table_type \
             FROM information_schema.tables \
             WHERE table_schema = $1 AND table_type IN ('BASE TABLE','VIEW') \
             ORDER BY table_name",
        )
        .bind(&schema)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut res = Vec::new();
        for row in rows {
            res.push(TableInfo {
                schema: row
                    .try_get::<String, _>("table_schema")
                    .unwrap_or(schema.clone()),
                name: row.try_get::<String, _>("table_name").unwrap_or_default(),
                r#type: row
                    .try_get::<String, _>("table_type")
                    .unwrap_or_else(|_| "table".to_string()),
            });
        }
        Ok(res)
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String> {
        let pool = self.get_pool().await?;
        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default \
             FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&table)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(ColumnInfo {
                name: row.try_get(0).unwrap_or_default(),
                r#type: row.try_get(1).unwrap_or_default(),
                nullable: row.try_get::<String, _>(2).unwrap_or_default() == "YES",
                default_value: row.try_get(3).ok(),
                primary_key: false, // TODO: 需要查询 constraint
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
        let pool = self.get_pool().await?;

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
        .fetch_all(&pool)
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
        .fetch_all(&pool)
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
        .fetch_all(&pool)
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
        .fetch_all(&pool)
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
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let pool = self.get_pool().await?;
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
            .fetch_one(&pool)
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
    ) -> Result<TableDataResponse, String> {
        let start = std::time::Instant::now();
        let pool = self.get_pool().await?;
        let offset = (page - 1) * limit;

        // Get total count
        let count_query = format!("SELECT COUNT(*) FROM {}.{}", schema, table);
        let total: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        // Build ORDER BY clause if sort parameters are provided
        let order_clause = if let Some(ref col) = sort_column {
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
            "SELECT * FROM {}.{}{} LIMIT $1 OFFSET $2",
            schema, table, order_clause
        );
        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

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
                        .unwrap_or(serde_json::Value::Null),
                    "INT2" | "INT4" | "INT8" => row
                        .try_get::<i64, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "FLOAT4" | "FLOAT8" => row
                        .try_get::<f64, _>(name)
                        .ok()
                        .map(serde_json::Value::from)
                        .unwrap_or(serde_json::Value::Null),
                    "NUMERIC" | "MONEY" => row
                        .try_get::<String, _>(name)
                        .ok()
                        .map(serde_json::Value::String)
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
                        .unwrap_or(serde_json::Value::Null),
                    "TIME" => row
                        .try_get::<NaiveTime, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "TIMESTAMP" => row
                        .try_get::<NaiveDateTime, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "TIMESTAMPTZ" => row
                        .try_get::<DateTime<Utc>, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "JSON" | "JSONB" => row
                        .try_get::<sqlx::types::Json<serde_json::Value>, _>(name)
                        .ok()
                        .map(|v| v.0)
                        .unwrap_or(serde_json::Value::Null),
                    _ => row
                        .try_get::<String, _>(name)
                        .ok()
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
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

    async fn execute_query(&self, sql: String) -> Result<QueryResult, String> {
        let start = std::time::Instant::now();
        let pool = self.get_pool().await?;
        let rows = sqlx::query(&sql)
            .fetch_all(&pool)
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
                        .unwrap_or(serde_json::Value::Null),
                    "INT2" | "INT4" | "INT8" => row
                        .try_get::<i64, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "FLOAT4" | "FLOAT8" => row
                        .try_get::<f64, _>(name)
                        .ok()
                        .map(serde_json::Value::from)
                        .unwrap_or(serde_json::Value::Null),
                    "NUMERIC" | "MONEY" => row
                        .try_get::<String, _>(name)
                        .ok()
                        .map(serde_json::Value::String)
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
                        .unwrap_or(serde_json::Value::Null),
                    "TIME" => row
                        .try_get::<NaiveTime, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "TIMESTAMP" => row
                        .try_get::<NaiveDateTime, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "TIMESTAMPTZ" => row
                        .try_get::<DateTime<Utc>, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "JSON" | "JSONB" => row
                        .try_get::<sqlx::types::Json<serde_json::Value>, _>(name)
                        .ok()
                        .map(|v| v.0)
                        .unwrap_or(serde_json::Value::Null),
                    _ => row
                        .try_get::<String, _>(name)
                        .ok()
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
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
        let pool = self.get_pool().await?;

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
            .fetch_all(&pool)
            .await
        } else {
            sqlx::query(
                "SELECT table_schema, table_name, column_name, data_type \
             FROM information_schema.columns \
             WHERE table_schema NOT IN ('information_schema', 'pg_catalog') \
             ORDER BY table_schema, table_name, ordinal_position",
            )
            .fetch_all(&pool)
            .await
        };

        let rows = rows.map_err(|e| {
            eprintln!("[QUERY_ERROR] Raw error: {}", e);
            "[QUERY_ERROR] Failed to fetch schema overview".to_string()
        })?;

        let mut tables_map: std::collections::HashMap<(String, String), Vec<ColumnSchema>> =
            std::collections::HashMap::new();

        for row in rows {
            let schema_name: String = row
                .try_get(0)
                .map_err(|e| format!("[PARSE_ERROR] Postgres table_schema: {}", e))?;
            let table_name: String = row
                .try_get(1)
                .map_err(|e| format!("[PARSE_ERROR] Postgres table_name: {}", e))?;
            let col_name: String = row
                .try_get(2)
                .map_err(|e| format!("[PARSE_ERROR] Postgres column_name: {}", e))?;
            let data_type: String = row
                .try_get(3)
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
        let driver = PostgresDriver { form };
        // 验证生成的连接字符串是否符合预期
        // postgres://postgres:password@localhost:5432/mydb
        let dsn = driver.conn_string().unwrap();
        assert_eq!(dsn, "postgres://postgres:password@localhost:5432/mydb");
    }

    #[test]
    fn test_conn_string_missing_fields() {
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            host: None, // Missing host
            ..Default::default()
        };
        let driver = PostgresDriver { form };
        assert!(driver.conn_string().is_err());
    }
}
