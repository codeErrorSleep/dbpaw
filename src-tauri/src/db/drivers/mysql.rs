use async_trait::async_trait;
use sqlx::{mysql::MySqlPoolOptions, Row, Column};
use crate::models::{ConnectionForm, QueryResult, TableInfo, TableStructure, TableDataResponse, ColumnInfo, QueryColumn};
use super::DatabaseDriver;

pub struct MysqlDriver {
    pub form: ConnectionForm,
}

impl MysqlDriver {
    fn conn_string(&self) -> Result<String, String> {
        let host = self.form.host.clone().ok_or("[VALIDATION_ERROR] host 不能为空")?;
        let port = self.form.port.unwrap_or(3306);
        // 允许 database 为空
        let username = self.form.username.clone().ok_or("[VALIDATION_ERROR] username 不能为空")?;
        let password = self.form.password.clone().ok_or("[VALIDATION_ERROR] password 不能为空")?;
        if let Some(db) = &self.form.database {
            if !db.is_empty() {
                return Ok(format!("mysql://{}:{}@{}:{}/{}", username, password, host, port, db));
            }
        }
        Ok(format!("mysql://{}:{}@{}:{}", username, password, host, port))
    }

    async fn get_pool(&self) -> Result<sqlx::MySqlPool, String> {
        let dsn = self.conn_string()?;
        MySqlPoolOptions::new().max_connections(1).connect(&dsn).await.map_err(|e| format!("[CONN_FAILED] {e}"))
    }
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
    async fn test_connection(&self) -> Result<(), String> {
        let pool = self.get_pool().await?;
        sqlx::query("SELECT 1").execute(&pool).await.map_err(|e| format!("[QUERY_ERROR] {e}"))?;
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
        let db = self.form.database.clone().ok_or("[VALIDATION_ERROR] database 不能为空")?;
        let schema = schema.unwrap_or(db.clone());
        let rows = sqlx::query(
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
                name: row.try_get("column_name").unwrap_or_default(),
                r#type: row.try_get("data_type").unwrap_or_default(),
                nullable: row.try_get::<String, _>("is_nullable").unwrap_or_default() == "YES",
                default_value: row.try_get("column_default").ok(),
                primary_key: false, // TODO
                comment: None,
            });
        }
        Ok(TableStructure { columns })
    }

    async fn get_table_data(&self, schema: String, table: String, page: i64, limit: i64) -> Result<TableDataResponse, String> {
        let pool = self.get_pool().await?;
        let offset = (page - 1) * limit;
        let qualified = if schema.is_empty() { format!("`{}`", table) } else { format!("`{}`.`{}`", schema, table) };
        let query = format!("SELECT * FROM {} LIMIT ? OFFSET ?", qualified);
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
                // 简化处理
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
                // 简化处理
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
