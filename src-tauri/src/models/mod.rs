mod network_proxy;
pub use network_proxy::{NetworkProxy, ProxyKind};

use serde::{Deserialize, Serialize};
use std::fmt;

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
    pub ssl_mode: Option<String>,
    pub ssl_ca_cert: Option<String>,
    pub file_path: Option<String>,
    pub ssh_enabled: bool,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<i64>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_key_path: Option<String>,
    #[serde(default)]
    #[sqlx(json(nullable))]
    pub proxy: Option<NetworkProxy>,
    pub mode: Option<String>,
    pub seed_nodes: Option<Vec<String>>,
    pub sentinels: Option<Vec<String>>,
    pub connect_timeout_ms: Option<i64>,
    pub service_name: Option<String>,
    pub sentinel_password: Option<String>,
    pub auth_mode: Option<String>,
    pub api_key_id: Option<String>,
    pub api_key_secret: Option<String>,
    pub api_key_encoded: Option<String>,
    pub cloud_id: Option<String>,
    pub auth_source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SavedQuery {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub description: Option<String>,
    pub connection_id: Option<i64>,
    pub database: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SqlExecutionLog {
    pub id: i64,
    pub sql: String,
    pub source: Option<String>,
    pub connection_id: Option<i64>,
    pub database: Option<String>,
    pub success: bool,
    pub error: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RedisCommandLog {
    pub id: i64,
    pub command: String,
    pub connection_id: Option<i64>,
    pub database: Option<String>,
    pub success: bool,
    pub error: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub is_default: bool,
    pub enabled: bool,
    pub extra_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderPublic {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    pub is_default: bool,
    pub enabled: bool,
    pub extra_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderForm {
    pub name: String,
    pub provider_type: Option<String>,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub is_default: Option<bool>,
    pub enabled: Option<bool>,
    pub extra_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AiConversation {
    pub id: i64,
    pub title: String,
    pub scenario: String,
    pub connection_id: Option<i64>,
    pub database: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AiMessage {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub prompt_version: Option<String>,
    pub model: Option<String>,
    pub token_in: Option<i64>,
    pub token_out: Option<i64>,
    pub latency_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConversationDetail {
    pub conversation: AiConversation,
    pub messages: Vec<AiMessage>,
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
pub struct RoutineInfo {
    pub schema: String,
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInfo {
    pub schema: String,
    pub name: String,
    pub status: String,
    pub event_type: String,
    pub execute_at: Option<String>,
    pub interval_value: Option<String>,
    pub last_executed: Option<String>,
    pub definition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceInfo {
    pub schema: String,
    pub name: String,
    pub data_type: String,
    pub start_value: Option<String>,
    pub increment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeInfo {
    pub schema: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynonymInfo {
    pub schema: String,
    pub name: String,
    pub base_object_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    pub schema: String,
    pub name: String,
    pub object_type: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_constraint_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub unique: bool,
    pub index_type: Option<String>,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyInfo {
    pub name: String,
    pub column: String,
    pub referenced_schema: Option<String>,
    pub referenced_table: String,
    pub referenced_column: String,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickHouseTableExtra {
    pub engine: String,
    pub partition_key: Option<String>,
    pub sorting_key: Option<String>,
    pub primary_key_expr: Option<String>,
    pub sampling_key: Option<String>,
    pub ttl_expr: Option<String>,
    pub create_table_query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CassandraTableExtra {
    pub partition_key: Vec<String>,
    pub clustering_columns: Vec<String>,
    pub compaction_strategy: String,
    pub bloom_filter_fp_chance: f64,
    pub caching: serde_json::Value,
    pub gc_grace_seconds: i64,
    pub default_time_to_live: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialTypeSummary {
    pub column_name: String,
    pub category: String,
    pub type_name: String,
    pub declared_length: Option<String>,
    pub memory_usage_bytes: Option<u64>,
    pub memory_usage_display: Option<String>,
    pub raw_type: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMetadata {
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub clickhouse_extra: Option<ClickHouseTableExtra>,
    pub cassandra_extra: Option<CassandraTableExtra>,
    pub special_type_summaries: Vec<SpecialTypeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryColumn {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleResultSet {
    pub data: Vec<serde_json::Value>,
    pub row_count: i64,
    pub columns: Vec<QueryColumn>,
    pub index: u32,
    pub statement: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_sets: Option<Vec<SingleResultSet>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryExecutionMetadata {
    pub query_id: String,
    pub original_sql: String,
    pub executed_sql: String,
    pub default_limit_applied: bool,
    pub default_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryExecutionResponse {
    #[serde(flatten)]
    pub result: QueryResult,
    pub execution: QueryExecutionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataResponse {
    pub data: Vec<serde_json::Value>,
    pub total: Option<i64>,
    pub page: i64,
    pub limit: i64,
    pub execution_time_ms: i64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionForm {
    pub driver: String, // "postgres" | "mysql" | "tidb" | "mariadb" | "sqlite" | "duckdb" | "clickhouse" | "mssql"
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl: Option<bool>,
    pub ssl_mode: Option<String>,
    pub ssl_ca_cert: Option<String>,
    pub file_path: Option<String>,
    pub ssh_enabled: Option<bool>,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<i64>,
    pub ssh_username: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_key_path: Option<String>,
    pub proxy: Option<NetworkProxy>,
    pub mode: Option<String>,
    pub seed_nodes: Option<Vec<String>>,
    pub sentinels: Option<Vec<String>>,
    pub connect_timeout_ms: Option<i64>,
    pub service_name: Option<String>,
    pub sentinel_password: Option<String>,
    pub auth_mode: Option<String>,
    pub api_key_id: Option<String>,
    pub api_key_secret: Option<String>,
    pub api_key_encoded: Option<String>,
    pub cloud_id: Option<String>,
    pub auth_source: Option<String>,
}

impl fmt::Debug for ConnectionForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let username = self.username.as_ref().map(|_| "<redacted>");
        let password = self.password.as_ref().map(|_| "<redacted>");
        let ssl_ca_cert = self.ssl_ca_cert.as_ref().map(|_| "<redacted>");
        let ssh_username = self.ssh_username.as_ref().map(|_| "<redacted>");
        let ssh_password = self.ssh_password.as_ref().map(|_| "<redacted>");
        let api_key_id = self.api_key_id.as_ref().map(|_| "<redacted>");
        let api_key_secret = self.api_key_secret.as_ref().map(|_| "<redacted>");
        let api_key_encoded = self.api_key_encoded.as_ref().map(|_| "<redacted>");
        let sentinel_password = self.sentinel_password.as_ref().map(|_| "<redacted>");
        f.debug_struct("ConnectionForm")
            .field("driver", &self.driver)
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("schema", &self.schema)
            .field("username", &username)
            .field("password", &password)
            .field("ssl", &self.ssl)
            .field("ssl_mode", &self.ssl_mode)
            .field("ssl_ca_cert", &ssl_ca_cert)
            .field("file_path", &self.file_path)
            .field("ssh_enabled", &self.ssh_enabled)
            .field("ssh_host", &self.ssh_host)
            .field("ssh_port", &self.ssh_port)
            .field("ssh_username", &ssh_username)
            .field("ssh_password", &ssh_password)
            .field("ssh_key_path", &self.ssh_key_path)
            .field("proxy", &self.proxy)
            .field("mode", &self.mode)
            .field("seed_nodes", &self.seed_nodes)
            .field("sentinels", &self.sentinels)
            .field("connect_timeout_ms", &self.connect_timeout_ms)
            .field("service_name", &self.service_name)
            .field("sentinel_password", &sentinel_password)
            .field("auth_mode", &self.auth_mode)
            .field("api_key_id", &api_key_id)
            .field("api_key_secret", &api_key_secret)
            .field("api_key_encoded", &api_key_encoded)
            .field("cloud_id", &self.cloud_id)
            .field("auth_source", &self.auth_source)
            .finish()
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaForeignKey {
    pub name: String,
    pub source_table: String,
    pub source_schema: Option<String>,
    pub source_column: String,
    pub target_table: String,
    pub target_schema: Option<String>,
    pub target_column: String,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::ConnectionForm;

    #[test]
    fn connection_form_debug_redacts_sensitive_fields() {
        let form = ConnectionForm {
            driver: "mysql".to_string(),
            host: Some("127.0.0.1".to_string()),
            username: Some("root".to_string()),
            password: Some("db-password-value".to_string()),
            ssl_ca_cert: Some("cert-data".to_string()),
            ssh_username: Some("jump".to_string()),
            ssh_password: Some("jump-secret".to_string()),
            api_key_id: Some("api-key-id".to_string()),
            api_key_secret: Some("api-key-secret".to_string()),
            api_key_encoded: Some("encoded-api-key".to_string()),
            ..Default::default()
        };

        let printed = format!("{form:?}");
        assert!(!printed.contains("root"));
        assert!(!printed.contains("db-password-value"));
        assert!(!printed.contains("cert-data"));
        assert!(!printed.contains("jump-secret"));
        assert!(!printed.contains("api-key-id"));
        assert!(!printed.contains("api-key-secret"));
        assert!(!printed.contains("encoded-api-key"));
        assert!(printed.contains("<redacted>"));
    }
}
