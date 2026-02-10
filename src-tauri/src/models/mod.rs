use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: i64,
    pub database: String,
    pub username: String,
    pub ssl: bool,
    pub file_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub r#type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryColumn {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub data: Vec<serde_json::Value>,
    pub row_count: i64,
    pub columns: Vec<QueryColumn>,
    pub time_taken_ms: i64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataResponse {
    pub data: Vec<serde_json::Value>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub execution_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionForm {
    pub driver: String, // "postgres" | "mysql" | "sqlite"
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl: Option<bool>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteByConnRequest {
    pub form: ConnectionForm,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSchema {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSchema {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaOverview {
    pub tables: Vec<TableSchema>,
}
