use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, Row, Column};
use crate::models::{ConnectionForm, QueryResult, TableInfo, TableStructure, TableDataResponse, ColumnInfo, QueryColumn};
use super::DatabaseDriver;

pub struct PostgresDriver {
    pub form: ConnectionForm,
}

impl PostgresDriver {
    fn conn_string(&self) -> Result<String, String> {
        let host = self.form.host.clone().ok_or("[VALIDATION_ERROR] host 不能为空")?;
        let port = self.form.port.unwrap_or(5432);
        // 允许 database 为空，默认为 postgres
        let database = self.form.database.clone().unwrap_or_else(|| "postgres".to_string());
        let username = self.form.username.clone().ok_or("[VALIDATION_ERROR] username 不能为空")?;
        let password = self.form.password.clone().ok_or("[VALIDATION_ERROR] password 不能为空")?;
        Ok(format!("postgres://{}:{}@{}:{}/{}", username, password, host, port, database))
    }

    async fn get_pool(&self) -> Result<sqlx::PgPool, String> {
        let dsn = self.conn_string()?;
        PgPoolOptions::new().max_connections(1).connect(&dsn).await.map_err(|e| format!("[CONN_FAILED] {e}"))
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn test_connection(&self) -> Result<(), String> {
        let pool = self.get_pool().await?;
        sqlx::query("SELECT 1").execute(&pool).await.map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        Ok(())
    }

    async fn list_databases(&self) -> Result<Vec<String>, String> {
        let pool = self.get_pool().await?;
        let rows: Vec<(String,)> = sqlx::query_as("SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname")
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
                schema: row.try_get::<String, _>("table_schema").unwrap_or(schema.clone()),
                name: row.try_get::<String, _>("table_name").unwrap_or_default(),
                r#type: row.try_get::<String, _>("table_type").unwrap_or_else(|_| "table".to_string()),
            });
        }
        Ok(res)
    }

    async fn get_table_structure(&self, schema: String, table: String) -> Result<TableStructure, String> {
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
                name: row.try_get("column_name").unwrap_or_default(),
                r#type: row.try_get("data_type").unwrap_or_default(),
                nullable: row.try_get::<String, _>("is_nullable").unwrap_or_default() == "YES",
                default_value: row.try_get("column_default").ok(),
                primary_key: false, // TODO: 需要查询 constraint
                comment: None,
            });
        }
        Ok(TableStructure { columns })
    }

    async fn get_table_data(&self, schema: String, table: String, page: i64, limit: i64) -> Result<TableDataResponse, String> {
        let pool = self.get_pool().await?;
        let offset = (page - 1) * limit;
        let query = format!("SELECT * FROM {}.{} LIMIT $1 OFFSET $2", schema, table);
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
                // 简化处理，实际应根据类型转换
                let v_str: Option<String> = row.try_get::<String, _>(name).ok()
                    .or_else(|| row.try_get::<i64, _>(name).ok().map(|v| v.to_string()))
                    .or_else(|| row.try_get::<bool, _>(name).ok().map(|v| v.to_string()));
                
                obj.insert(name.to_string(), v_str.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
            }
            data.push(serde_json::Value::Object(obj));
        }
        Ok(TableDataResponse { data, total: -1, page, limit })
    }

    async fn execute_query(&self, sql: String) -> Result<QueryResult, String> {
        let pool = self.get_pool().await?;
        let rows = sqlx::query(&sql).fetch_all(&pool).await.map_err(|e| format!("[QUERY_ERROR] {e}"))?;
        
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
                 // 简化处理，实际应根据类型转换
                let v_str: Option<String> = row.try_get::<String, _>(name).ok()
                    .or_else(|| row.try_get::<i64, _>(name).ok().map(|v| v.to_string()))
                    .or_else(|| row.try_get::<bool, _>(name).ok().map(|v| v.to_string()));
                obj.insert(name.to_string(), v_str.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
            }
            data.push(serde_json::Value::Object(obj));
        }

        Ok(QueryResult {
            data,
            row_count: rows.len() as i64,
            columns,
            time_taken_ms: 0,
            success: true,
            error: None,
        })
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
