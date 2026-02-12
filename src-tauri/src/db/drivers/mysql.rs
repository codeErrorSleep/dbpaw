use super::DatabaseDriver;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, ForeignKeyInfo, IndexInfo, QueryColumn, QueryResult,
    SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use rust_decimal::Decimal;
use sqlx::{mysql::MySqlPoolOptions, Column, Row, TypeInfo};
use std::collections::{HashMap, HashSet};

pub struct MysqlDriver {
    pub form: ConnectionForm,
}

impl MysqlDriver {
    fn conn_string(&self) -> Result<String, String> {
        let host = self
            .form
            .host
            .clone()
            .ok_or("[VALIDATION_ERROR] host 不能为空")?;
        let port = self.form.port.unwrap_or(3306);
        // 允许 database 为空
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
        if let Some(db) = &self.form.database {
            if !db.is_empty() {
                return Ok(format!(
                    "mysql://{}:{}@{}:{}/{}",
                    username, password, host, port, db
                ));
            }
        }
        Ok(format!(
            "mysql://{}:{}@{}:{}",
            username, password, host, port
        ))
    }

    async fn get_pool(&self) -> Result<sqlx::MySqlPool, String> {
        let dsn = self.conn_string()?;
        MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&dsn)
            .await
            .map_err(|e| format!("[CONN_FAILED] {e}"))
    }
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
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
        let rows: Vec<(String,)> = sqlx::query_as("SHOW DATABASES")
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String> {
        let pool = self.get_pool().await?;
        let db = self
            .form
            .database
            .clone()
            .ok_or("[VALIDATION_ERROR] database 不能为空")?;
        let schema = schema.unwrap_or(db.clone());
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT table_schema, table_name, table_type \
             FROM information_schema.tables \
             WHERE table_schema = ? AND table_type IN ('BASE TABLE','VIEW') \
             ORDER BY table_name",
        )
        .bind(&schema)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let mut res = Vec::new();
        for (table_schema, table_name, table_type) in rows {
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
        let pool = self.get_pool().await?;
        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default \
             FROM information_schema.columns \
             WHERE table_schema = ? AND table_name = ? \
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
        let pool = self.get_pool().await?;

        let pk_rows: Vec<(String,)> = sqlx::query_as(
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
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("[QUERY_ERROR] {e}"))?;

        let pk_set: HashSet<String> = pk_rows.into_iter().map(|r| r.0).collect();

        let column_rows = sqlx::query(
            "SELECT column_name, column_type, is_nullable, column_default, column_comment \
             FROM information_schema.columns \
             WHERE table_schema = ? AND table_name = ? \
             ORDER BY ordinal_position",
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
            columns.push(ColumnInfo {
                name: name.clone(),
                r#type: row.try_get(1).unwrap_or_default(),
                nullable: row.try_get::<String, _>(2).unwrap_or_default() == "YES",
                default_value: row.try_get::<Option<String>, _>(3).unwrap_or(None),
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
        .fetch_all(&pool)
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
            let Some(column_name) = column_name else { continue };

            let entry = index_map
                .entry(index_name)
                .or_insert((non_unique == 0, index_type.clone(), Vec::new()));
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
        .fetch_all(&pool)
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
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String> {
        let pool = self.get_pool().await?;
        let qualified = if schema.is_empty() {
            format!("`{}`", table)
        } else {
            format!("`{}`.`{}`", schema, table)
        };
        let query = format!("SHOW CREATE TABLE {}", qualified);
        let row: (String, String) = sqlx::query_as(&query)
            .fetch_one(&pool)
            .await
            .map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(row.1)
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
        let qualified = if schema.is_empty() {
            format!("`{}`", table)
        } else {
            format!("`{}`.`{}`", schema, table)
        };

        // Get total count
        let count_query = format!("SELECT COUNT(*) FROM {}", qualified);
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
            format!(" ORDER BY `{}` {}", col, dir)
        } else {
            String::new()
        };

        let query = format!("SELECT * FROM {}{} LIMIT ? OFFSET ?", qualified, order_clause);
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
                    "TINYINT" | "SMALLINT" | "INT" | "INTEGER" | "MEDIUMINT" | "BIGINT"
                    | "YEAR" => row
                        .try_get::<i64, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "FLOAT" | "DOUBLE" | "REAL" => row
                        .try_get::<f64, _>(name)
                        .ok()
                        .map(serde_json::Value::from)
                        .unwrap_or(serde_json::Value::Null),
                    "DECIMAL" | "NEWDECIMAL" => row
                        .try_get::<Decimal, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "BOOL" | "BOOLEAN" => row
                        .try_get::<bool, _>(name)
                        .ok()
                        .map(serde_json::Value::Bool)
                        .or_else(|| {
                            row.try_get::<i64, _>(name)
                                .ok()
                                .map(|v| serde_json::Value::Bool(v != 0))
                        })
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
                    "DATETIME" | "TIMESTAMP" => row
                        .try_get::<NaiveDateTime, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "JSON" => row
                        .try_get::<sqlx::types::Json<serde_json::Value>, _>(name)
                        .ok()
                        .map(|v| v.0)
                        .unwrap_or(serde_json::Value::Null),
                    "BIT" => row
                        .try_get::<u64, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::Number(v.into()))
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
                    "TINYINT" | "SMALLINT" | "INT" | "INTEGER" | "MEDIUMINT" | "BIGINT"
                    | "YEAR" => row
                        .try_get::<i64, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "FLOAT" | "DOUBLE" | "REAL" => row
                        .try_get::<f64, _>(name)
                        .ok()
                        .map(serde_json::Value::from)
                        .unwrap_or(serde_json::Value::Null),
                    "DECIMAL" | "NEWDECIMAL" => row
                        .try_get::<Decimal, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "BOOL" | "BOOLEAN" => row
                        .try_get::<bool, _>(name)
                        .ok()
                        .map(serde_json::Value::Bool)
                        .or_else(|| {
                            row.try_get::<i64, _>(name)
                                .ok()
                                .map(|v| serde_json::Value::Bool(v != 0))
                        })
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
                    "DATETIME" | "TIMESTAMP" => row
                        .try_get::<NaiveDateTime, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::String(v.to_string()))
                        .unwrap_or(serde_json::Value::Null),
                    "JSON" => row
                        .try_get::<sqlx::types::Json<serde_json::Value>, _>(name)
                        .ok()
                        .map(|v| v.0)
                        .unwrap_or(serde_json::Value::Null),
                    "BIT" => row
                        .try_get::<u64, _>(name)
                        .ok()
                        .map(|v| serde_json::Value::Number(v.into()))
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

        let sql = "SELECT table_schema, table_name, column_name, data_type \
             FROM information_schema.columns"
            .to_string();

        let rows = if let Some(s) = schema {
            sqlx::query(&format!(
                "{} WHERE table_schema = ? ORDER BY table_schema, table_name, ordinal_position",
                sql
            ))
            .bind(s)
            .fetch_all(&pool)
            .await
        } else {
            let db = self.form.database.clone().unwrap_or_default();
            if !db.is_empty() {
                sqlx::query(&format!(
                    "{} WHERE table_schema = ? ORDER BY table_schema, table_name, ordinal_position",
                    sql
                ))
                .bind(db)
                .fetch_all(&pool)
                .await
            } else {
                sqlx::query(&format!("{} WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys') ORDER BY table_schema, table_name, ordinal_position", sql))
                .fetch_all(&pool)
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
            let schema_name: String = row
                .try_get(0)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get table_schema: {}", e))?;
            let table_name: String = row
                .try_get(1)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get table_name: {}", e))?;
            let col_name: String = row
                .try_get(2)
                .map_err(|e| format!("[PARSE_ERROR] Failed to get column_name: {}", e))?;
            let data_type: String = row
                .try_get(3)
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

        let driver = MysqlDriver { form };
        let conn_str = driver.conn_string().unwrap();
        assert_eq!(conn_str, "mysql://root:password@localhost:3306/test_db");
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

        let driver = MysqlDriver { form };
        let conn_str = driver.conn_string().unwrap();
        assert_eq!(conn_str, "mysql://user:pass@127.0.0.1:3307");
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

        let driver = MysqlDriver { form };
        assert!(driver.conn_string().is_err());
    }
}
