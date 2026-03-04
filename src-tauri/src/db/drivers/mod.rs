use self::clickhouse::ClickHouseDriver;
use self::mssql::MssqlDriver;
use self::mysql::MysqlDriver;
use self::postgres::PostgresDriver;
use self::sqlite::SqliteDriver;
use crate::models::{
    ConnectionForm, QueryResult, SchemaOverview, TableDataResponse, TableInfo, TableMetadata,
    TableStructure,
};
use async_trait::async_trait;

pub mod clickhouse;
pub mod mssql;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn test_connection(&self) -> Result<(), String>;
    async fn list_databases(&self) -> Result<Vec<String>, String>;
    async fn list_tables(&self, schema: Option<String>) -> Result<Vec<TableInfo>, String>;
    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableStructure, String>;
    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> Result<TableMetadata, String>;
    async fn get_table_ddl(&self, schema: String, table: String) -> Result<String, String>;
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
    ) -> Result<TableDataResponse, String>;
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
    ) -> Result<TableDataResponse, String>;
    async fn execute_query(&self, sql: String) -> Result<QueryResult, String>;
    async fn get_schema_overview(&self, schema: Option<String>) -> Result<SchemaOverview, String>;
    async fn close(&self);
}

/// Normalize macOS smart quotes (U+2018/U+2019/U+201C/U+201D) to ASCII equivalents.
/// WKWebView on macOS inherits the system "Smart Quotes" setting and may
/// automatically replace straight quotes typed by the user.
pub fn normalize_quotes(s: &str) -> String {
    s.replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
}

pub async fn connect(form: &ConnectionForm) -> Result<Box<dyn DatabaseDriver>, String> {
    match form.driver.as_str() {
        "postgres" => {
            let driver = PostgresDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mysql" => {
            let driver = MysqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "sqlite" => {
            let driver = SqliteDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "clickhouse" => {
            let driver = ClickHouseDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mssql" => {
            let driver = MssqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        _ => Err(format!(
            "[UNSUPPORTED] Driver {} not supported",
            form.driver
        )),
    }
}
