use async_trait::async_trait;
use crate::models::{ConnectionForm, QueryResult, TableInfo, TableStructure, TableDataResponse, SchemaOverview};
use self::postgres::PostgresDriver;
use self::mysql::MysqlDriver;

pub mod postgres;
pub mod mysql;

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn list_databases(&self) -> Result<Vec<String>, String>;
    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String>;
    async fn get_table_structure(&self, schema: String, table: String) -> Result<TableStructure, String>;
    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String>;
    async fn get_table_data(&self, schema: String, table: String, page: i64, limit: i64) -> Result<TableDataResponse, String>;
    async fn execute_query(&self, sql: String) -> Result<QueryResult, String>;
    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String>;
}

pub fn get_driver(form: &ConnectionForm) -> Result<Box<dyn DatabaseDriver>, String> {
    match form.driver.as_str() {
        "postgres" => Ok(Box::new(PostgresDriver { form: form.clone() })),
        "mysql" => Ok(Box::new(MysqlDriver { form: form.clone() })),
        _ => Err(format!("[UNSUPPORTED] Driver {} not supported", form.driver)),
    }
}
