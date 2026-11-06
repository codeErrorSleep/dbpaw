use super::{DatabaseDriver, DriverCapabilities, DriverResult};
use crate::error::AppError;
use crate::models::{
    ColumnInfo, ColumnSchema, ConnectionForm, IndexInfo, QueryColumn, QueryResult, SchemaOverview,
    SingleResultSet, TableDataResponse, TableInfo, TableMetadata, TableSchema, TableStructure,
};
use async_trait::async_trait;
use mongodb::Client;
use mongodb::bson::{Bson, Document, doc, oid::ObjectId};
use mongodb::options::{ClientOptions, Tls, TlsOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const DEFAULT_MONGODB_PORT: i64 = 27017;
const DEFAULT_CONNECT_TIMEOUT_MS: i64 = 5000;
const SCHEMA_SAMPLE_SIZE: i64 = 100;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn trim_to_option(value: Option<&String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .and_then(|v| if v.is_empty() { None } else { Some(v) })
}

fn normalize_mongo_error(e: impl std::fmt::Display) -> AppError {
    let msg = e.to_string();
    if msg.contains("authentication") || msg.contains("auth") {
        AppError::conn_auth_failed(format!("Authentication failed: {}", msg))
    } else if msg.contains("dns") || msg.contains("resolve") || msg.contains("lookup") {
        AppError::conn_failed(format!("DNS resolution failed: {}", msg), "Check hostname")
    } else if msg.contains("timeout") || msg.contains("timed out") {
        AppError::conn_timeout(format!("Connection timed out: {}", msg))
    } else if msg.contains("refused") {
        AppError::conn_failed(
            format!("Connection refused: {}", msg),
            "Check host and port",
        )
    } else {
        AppError::internal(format!("MongoDB error: {}", msg))
    }
}

fn build_connection_uri(form: &ConnectionForm) -> DriverResult<String> {
    let host = trim_to_option(form.host.as_ref())
        .ok_or_else(|| AppError::validation("host cannot be empty"))?;
    let port = form.port.unwrap_or(DEFAULT_MONGODB_PORT);
    if !(1..=65535).contains(&port) {
        return Err(AppError::validation("port must be between 1 and 65535"));
    }

    let username = trim_to_option(form.username.as_ref());
    let password = trim_to_option(form.password.as_ref());
    let database = trim_to_option(form.database.as_ref());
    let auth_source = trim_to_option(form.auth_source.as_ref());

    let mut uri = String::from("mongodb://");

    if let Some(user) = &username {
        uri.push_str(&urlencoding::encode(user));
        if let Some(pass) = &password {
            uri.push(':');
            uri.push_str(&urlencoding::encode(pass));
        }
        uri.push('@');
    }

    uri.push_str(&host);
    uri.push(':');
    uri.push_str(&port.to_string());

    if let Some(db) = &database {
        uri.push('/');
        uri.push_str(db);
    }

    let mut params = Vec::new();
    if let Some(src) = &auth_source {
        params.push(format!("authSource={}", urlencoding::encode(src)));
    }
    if form.ssl.unwrap_or(false) {
        params.push("ssl=true".to_string());
    }
    if let Some(timeout) = form.connect_timeout_ms {
        params.push(format!("connectTimeoutMS={}", timeout));
    }
    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    Ok(uri)
}

/// Parse a JSON string into a BSON Document, returning a user-friendly error on failure.
fn parse_json_doc(json_str: &str, label: &str) -> DriverResult<Document> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
        return Ok(Document::new());
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| AppError::validation(format!("Invalid {} JSON: {}", label, e)))?;
    mongodb::bson::to_document(&value)
        .map_err(|e| AppError::validation(format!("Failed to convert {} to BSON: {}", label, e)))
}

// ---------------------------------------------------------------------------
// BSON <-> JSON conversion
// ---------------------------------------------------------------------------

fn bson_to_json(bson: &Bson) -> Value {
    match bson {
        Bson::Double(v) => serde_json::json!(v),
        Bson::String(v) => Value::String(v.clone()),
        Bson::Array(arr) => Value::Array(arr.iter().map(bson_to_json).collect()),
        Bson::Document(doc) => {
            let map = doc
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_json(v)))
                .collect();
            Value::Object(map)
        }
        Bson::Boolean(v) => serde_json::json!(v),
        Bson::Null => Value::Null,
        Bson::Int32(v) => serde_json::json!(v),
        Bson::Int64(v) => serde_json::json!(v),
        Bson::ObjectId(oid) => Value::String(oid.to_hex()),
        Bson::DateTime(dt) => Value::String(dt.to_string()),
        Bson::Timestamp(ts) => serde_json::json!({ "t": ts.time, "i": ts.increment }),
        Bson::Binary(bin) => Value::String(format!("<Binary len={}>", bin.bytes.len())),
        Bson::RegularExpression(re) => Value::String(format!("/{}/{}", re.pattern, re.options)),
        Bson::JavaScriptCode(code) => Value::String(code.clone()),
        Bson::Symbol(sym) => Value::String(sym.clone()),
        other => Value::String(format!("{:?}", other)),
    }
}

fn bson_type_name(bson: &Bson) -> &'static str {
    match bson {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "object",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binData",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        Bson::Decimal128(_) => "decimal128",
        Bson::Symbol(_) => "symbol",
        Bson::Undefined => "undefined",
        Bson::DbPointer(_) => "dbPointer",
        Bson::MinKey => "minKey",
        Bson::MaxKey => "maxKey",
    }
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

pub struct MongoDBDriver {
    client: Client,
    default_database: String,
    #[allow(dead_code)]
    network_tunnel: Option<crate::network::NetworkTunnel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbConnectionInfo {
    pub version: Option<String>,
    pub node_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbDatabaseInfo {
    pub name: String,
    pub size_on_disk: Option<i64>,
    pub empty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongodbCollectionInfo {
    pub name: String,
    pub database: String,
    pub document_count: Option<i64>,
    pub size: Option<i64>,
}

impl MongoDBDriver {
    pub async fn connect(form: &ConnectionForm) -> DriverResult<Self> {
        let timeout_ms = form
            .connect_timeout_ms
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS);

        let mut effective_form = form.clone();
        let network_tunnel = if crate::network::tunnel_enabled(form) {
            let tunnel = crate::network::start_network_tunnel(form).await?;
            effective_form.host = Some("127.0.0.1".to_string());
            effective_form.port = Some(tunnel.local_port as i64);
            Some(tunnel)
        } else {
            None
        };

        let uri = build_connection_uri(&effective_form)?;
        let mut options = ClientOptions::parse(&uri)
            .await
            .map_err(normalize_mongo_error)?;
        if crate::network::proxy_enabled(form) {
            // Keep topology discovery from bypassing the configured single-host tunnel.
            options.direct_connection = Some(true);
        }
        options.connect_timeout = Some(Duration::from_millis(timeout_ms as u64));

        if effective_form.ssl.unwrap_or(false) {
            let ssl_mode = trim_to_option(effective_form.ssl_mode.as_ref());
            if ssl_mode.as_deref() == Some("verify_ca") {
                let ca_cert =
                    trim_to_option(effective_form.ssl_ca_cert.as_ref()).ok_or_else(|| {
                        AppError::validation("sslCaCert cannot be empty in verify_ca mode")
                    })?;
                let tls_options = TlsOptions::builder()
                    .ca_file_path(std::path::PathBuf::from(ca_cert))
                    .build();
                options.tls = Some(Tls::Enabled(tls_options));
            }
        }

        let client = Client::with_options(options).map_err(normalize_mongo_error)?;
        let default_database = effective_form
            .database
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| "test".to_string());

        Ok(Self {
            client,
            default_database,
            network_tunnel,
        })
    }

    fn get_database(&self, schema: &str) -> mongodb::Database {
        let db_name = if schema.trim().is_empty() {
            &self.default_database
        } else {
            schema
        };
        self.client.database(db_name)
    }

    /// Sample documents to infer the collection's field names and types.
    async fn infer_collection_schema(
        &self,
        db_name: &str,
        collection_name: &str,
    ) -> DriverResult<Vec<ColumnInfo>> {
        let collection = self
            .client
            .database(db_name)
            .collection::<Document>(collection_name);

        let mut cursor = collection
            .find(Document::new())
            .limit(SCHEMA_SAMPLE_SIZE)
            .await
            .map_err(normalize_mongo_error)?;

        let mut field_types: HashMap<String, HashSet<&str>> = HashMap::new();

        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let doc = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            for (key, value) in doc.iter() {
                field_types
                    .entry(key.clone())
                    .or_default()
                    .insert(bson_type_name(value));
            }
        }

        let mut columns: Vec<ColumnInfo> = field_types
            .into_iter()
            .map(|(name, types)| {
                let type_str = if types.len() == 1 {
                    types.into_iter().next().unwrap().to_string()
                } else {
                    let mut sorted: Vec<&str> = types.into_iter().collect();
                    sorted.sort();
                    format!("mixed({})", sorted.join(","))
                };
                ColumnInfo {
                    name,
                    r#type: type_str,
                    nullable: true,
                    default_value: None,
                    primary_key: false,
                    comment: None,
                    default_constraint_name: None,
                }
            })
            .collect();

        columns.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(columns)
    }

    /// Convert a MongoDB cursor into a QueryResult.
    async fn cursor_to_query_result(
        &self,
        mut cursor: mongodb::Cursor<Document>,
        statement: &str,
        start: Instant,
    ) -> DriverResult<QueryResult> {
        let mut data = Vec::new();
        let mut columns_set = HashSet::new();

        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let doc = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            for key in doc.keys() {
                columns_set.insert(key.clone());
            }
            data.push(bson_to_json(&Bson::Document(doc)));
        }

        let mut columns: Vec<QueryColumn> = columns_set
            .into_iter()
            .map(|name| QueryColumn {
                name,
                r#type: "mixed".to_string(),
            })
            .collect();
        columns.sort_by(|a, b| a.name.cmp(&b.name));

        let row_count = data.len() as i64;
        let duration_ms = start.elapsed().as_millis() as i64;

        Ok(QueryResult {
            row_count,
            columns: columns.clone(),
            time_taken_ms: duration_ms,
            success: true,
            error: None,
            result_sets: Some(vec![SingleResultSet {
                data: data.clone(),
                row_count,
                columns,
                index: 0,
                statement: statement.to_string(),
            }]),
            data,
        })
    }

    /// Collect all documents from a cursor into a Vec<Value>.
    async fn collect_cursor(
        &self,
        mut cursor: mongodb::Cursor<Document>,
    ) -> DriverResult<Vec<Value>> {
        let mut rows = Vec::new();
        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let doc = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            rows.push(bson_to_json(&Bson::Document(doc)));
        }
        Ok(rows)
    }

    pub async fn test_connection_info(&self) -> DriverResult<MongodbConnectionInfo> {
        let db = self.get_database("admin");
        let result = db
            .run_command(doc! { "serverStatus": 1 })
            .await
            .map_err(normalize_mongo_error)?;

        let version = result.get_str("version").ok().map(|s| s.to_string());
        let node_count = result
            .get_document("connections")
            .ok()
            .and_then(|c| c.get_i32("current").ok());

        Ok(MongodbConnectionInfo {
            version,
            node_count,
        })
    }

    pub async fn list_databases_info(&self) -> DriverResult<Vec<MongodbDatabaseInfo>> {
        let databases = self
            .client
            .list_databases()
            .await
            .map_err(normalize_mongo_error)?;
        Ok(databases
            .into_iter()
            .map(|db| MongodbDatabaseInfo {
                name: db.name,
                size_on_disk: Some(db.size_on_disk as i64),
                empty: Some(db.empty),
            })
            .collect())
    }

    pub async fn list_collections_info(
        &self,
        database: &str,
    ) -> DriverResult<Vec<MongodbCollectionInfo>> {
        let db = self.client.database(database);
        let mut cursor = db.list_collections().await.map_err(normalize_mongo_error)?;

        let mut result = Vec::new();
        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let collection = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            result.push(MongodbCollectionInfo {
                name: collection.name,
                database: database.to_string(),
                document_count: None,
                size: None,
            });
        }

        Ok(result)
    }

    pub async fn find_documents(
        &self,
        database: &str,
        collection: &str,
        filter: Option<&str>,
        page: Option<i64>,
        page_size: Option<i64>,
    ) -> DriverResult<Value> {
        let safe_page = page.unwrap_or(1).max(1);
        let safe_page_size = page_size.unwrap_or(50).clamp(1, 10_000);
        let skip = (safe_page - 1) * safe_page_size;

        let db = self.client.database(database);
        let coll = db.collection::<Document>(collection);

        let filter_doc = match filter {
            Some(f) => parse_json_doc(f, "filter")?,
            None => Document::new(),
        };

        let total = coll
            .count_documents(filter_doc.clone())
            .await
            .map_err(normalize_mongo_error)? as i64;

        let cursor = coll
            .find(filter_doc)
            .skip(skip as u64)
            .limit(safe_page_size)
            .await
            .map_err(normalize_mongo_error)?;

        let documents = self.collect_cursor(cursor).await?;

        Ok(serde_json::json!({
            "documents": documents,
            "total": total,
            "page": safe_page,
            "pageSize": safe_page_size,
        }))
    }

    pub async fn get_document(
        &self,
        database: &str,
        collection: &str,
        document_id: &str,
    ) -> DriverResult<Value> {
        let db = self.client.database(database);
        let coll = db.collection::<Document>(collection);

        let oid = ObjectId::parse_str(document_id)
            .map_err(|e| AppError::validation(format!("Invalid document ID '{}': {}", document_id, e)))?;

        let doc = coll
            .find_one(doc! { "_id": oid })
            .await
            .map_err(normalize_mongo_error)?;

        match doc {
            Some(d) => Ok(bson_to_json(&Bson::Document(d))),
            None => Err(AppError::not_found(format!(
                "Document '{}' not found in {}.{}",
                document_id, database, collection
            ))),
        }
    }

    pub async fn insert_document(
        &self,
        database: &str,
        collection: &str,
        document: &Value,
    ) -> DriverResult<Value> {
        let db = self.client.database(database);
        let coll = db.collection::<Document>(collection);

        let bson_doc = mongodb::bson::to_document(document)
            .map_err(|e| AppError::validation(format!("Invalid document: {}", e)))?;

        let result = coll
            .insert_one(bson_doc)
            .await
            .map_err(normalize_mongo_error)?;

        let inserted_id = match result.inserted_id {
            Bson::ObjectId(oid) => oid.to_hex(),
            other => format!("{}", other),
        };

        Ok(serde_json::json!({
            "success": true,
            "insertedId": inserted_id,
        }))
    }

    pub async fn update_document(
        &self,
        database: &str,
        collection: &str,
        document_id: &str,
        update: &Value,
    ) -> DriverResult<Value> {
        let db = self.client.database(database);
        let coll = db.collection::<Document>(collection);

        let oid = ObjectId::parse_str(document_id)
            .map_err(|e| AppError::validation(format!("Invalid document ID '{}': {}", document_id, e)))?;

        let update_doc = mongodb::bson::to_document(update)
            .map_err(|e| AppError::validation(format!("Invalid update document: {}", e)))?;

        let result = coll
            .update_one(doc! { "_id": oid }, doc! { "$set": update_doc })
            .await
            .map_err(normalize_mongo_error)?;

        Ok(serde_json::json!({
            "success": true,
            "modifiedCount": result.modified_count as i64,
        }))
    }

    pub async fn delete_document(
        &self,
        database: &str,
        collection: &str,
        document_id: &str,
    ) -> DriverResult<Value> {
        let db = self.client.database(database);
        let coll = db.collection::<Document>(collection);

        let oid = ObjectId::parse_str(document_id)
            .map_err(|e| AppError::validation(format!("Invalid document ID '{}': {}", document_id, e)))?;

        let result = coll
            .delete_one(doc! { "_id": oid })
            .await
            .map_err(normalize_mongo_error)?;

        Ok(serde_json::json!({
            "success": true,
            "deletedCount": result.deleted_count as i64,
        }))
    }
}

// ---------------------------------------------------------------------------
// DatabaseDriver implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DatabaseDriver for MongoDBDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::empty()
    }

    async fn close(&self) {
        // MongoDB client manages connections via its internal connection pool.
    }

    async fn test_connection(&self) -> DriverResult<()> {
        let db = self.get_database("admin");
        db.run_command(doc! { "ping": 1 })
            .await
            .map_err(normalize_mongo_error)?;
        Ok(())
    }

    async fn list_databases(&self) -> DriverResult<Vec<String>> {
        let databases = self
            .client
            .list_databases()
            .await
            .map_err(normalize_mongo_error)?;
        Ok(databases.into_iter().map(|db| db.name).collect())
    }

    async fn list_tables(&self, schema: Option<String>) -> DriverResult<Vec<TableInfo>> {
        let db_name = schema
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.default_database.clone());

        let db = self.client.database(&db_name);
        let mut cursor = db.list_collections().await.map_err(normalize_mongo_error)?;

        let mut result = Vec::new();
        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let collection = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            result.push(TableInfo {
                schema: db_name.clone(),
                name: collection.name,
                r#type: "collection".to_string(),
            });
        }

        Ok(result)
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> DriverResult<TableStructure> {
        let columns = self.infer_collection_schema(&schema, &table).await?;
        Ok(TableStructure { columns })
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> DriverResult<TableMetadata> {
        let columns = self.infer_collection_schema(&schema, &table).await?;

        let db = self.get_database(&schema);
        let collection = db.collection::<Document>(&table);
        let mut cursor = collection
            .list_indexes()
            .await
            .map_err(normalize_mongo_error)?;

        let mut indexes = Vec::new();
        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let index = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            let options = index.options.unwrap_or_default();
            indexes.push(IndexInfo {
                name: options.name.unwrap_or_else(|| "unknown".to_string()),
                unique: options.unique.unwrap_or(false),
                index_type: None,
                columns: index.keys.keys().cloned().collect(),
            });
        }

        Ok(TableMetadata {
            columns,
            indexes,
            foreign_keys: vec![],
            clickhouse_extra: None,
            cassandra_extra: None,
            special_type_summaries: vec![],
        })
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> DriverResult<String> {
        let db = self.get_database(&schema);
        let mut cursor = db.list_collections().await.map_err(normalize_mongo_error)?;

        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let info = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;
            if info.name == table {
                return serde_json::to_string_pretty(&info)
                    .map_err(|e| AppError::internal(format!("Serialization failed: {}", e)));
            }
        }

        Err(AppError::not_found(format!(
            "Collection '{}' not found",
            table
        )))
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
        include_total: bool,
    ) -> DriverResult<TableDataResponse> {
        let start = Instant::now();
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 10_000);
        let skip = (safe_page - 1) * safe_limit;

        let collection = self.get_database(&schema).collection::<Document>(&table);
        let filter_doc = match filter {
            Some(f) => parse_json_doc(&f, "filter")?,
            None => Document::new(),
        };

        let total = if include_total {
            Some(
                collection
                    .count_documents(filter_doc.clone())
                    .await
                    .map_err(normalize_mongo_error)? as i64,
            )
        } else {
            None
        };

        let sort_doc = if let Some(ref ob) = order_by {
            parse_json_doc(ob, "sort")?
        } else if let Some(col) = sort_column {
            let dir = match sort_direction.as_deref() {
                Some("desc") => -1,
                _ => 1,
            };
            doc! { col: dir }
        } else {
            Document::new()
        };

        let mut find_builder = collection.find(filter_doc);
        find_builder = find_builder.skip(skip as u64).limit(safe_limit);
        if !sort_doc.is_empty() {
            find_builder = find_builder.sort(sort_doc);
        }

        let cursor = find_builder.await.map_err(normalize_mongo_error)?;
        let rows = self.collect_cursor(cursor).await?;

        Ok(TableDataResponse {
            data: rows,
            total,
            page: safe_page,
            limit: safe_limit,
            execution_time_ms: start.elapsed().as_millis() as i64,
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
    ) -> DriverResult<TableDataResponse> {
        self.get_table_data(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
            true,
        )
        .await
    }

    async fn execute_query(&self, query: String) -> DriverResult<QueryResult> {
        let start = Instant::now();
        let trimmed = query.trim();

        if trimmed.is_empty() {
            return Err(AppError::query_failed("Empty query"));
        }

        let parsed: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| AppError::query_failed(format!("Invalid JSON: {}", e)))?;

        let obj = parsed
            .as_object()
            .ok_or_else(|| AppError::query_failed("Query must be a JSON object"))?;

        let db_name = obj
            .get("$db")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_database);

        // --- find command ---
        if let Some(collection_name) = obj.get("find").and_then(|v| v.as_str()) {
            let filter = obj
                .get("filter")
                .and_then(|v| mongodb::bson::to_document(v).ok())
                .unwrap_or_default();
            let sort = obj
                .get("sort")
                .and_then(|v| mongodb::bson::to_document(v).ok());
            let limit = obj.get("limit").and_then(|v| v.as_i64()).unwrap_or(100);
            let skip = obj.get("skip").and_then(|v| v.as_i64()).unwrap_or(0);

            let collection = self
                .client
                .database(db_name)
                .collection::<Document>(collection_name);
            let mut builder = collection.find(filter).skip(skip as u64).limit(limit);
            if let Some(sort_doc) = sort {
                builder = builder.sort(sort_doc);
            }

            let cursor = builder.await.map_err(normalize_mongo_error)?;
            return self.cursor_to_query_result(cursor, trimmed, start).await;
        }

        // --- aggregate command ---
        if let Some(collection_name) = obj.get("aggregate").and_then(|v| v.as_str()) {
            let pipeline = obj
                .get("pipeline")
                .and_then(|v| v.as_array())
                .ok_or_else(|| AppError::query_failed("aggregate requires 'pipeline' array"))?;

            let bson_pipeline: Vec<Document> = pipeline
                .iter()
                .map(|stage| {
                    mongodb::bson::to_document(stage).map_err(|e| {
                        AppError::query_failed(format!("Invalid pipeline stage: {}", e))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let collection = self
                .client
                .database(db_name)
                .collection::<Document>(collection_name);
            let cursor = collection
                .aggregate(bson_pipeline)
                .await
                .map_err(normalize_mongo_error)?;
            return self.cursor_to_query_result(cursor, trimmed, start).await;
        }

        Err(AppError::query_failed(
            "Unsupported query format. Use {\"find\": \"collection\", ...} or {\"aggregate\": \"collection\", \"pipeline\": [...]}",
        ))
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> DriverResult<SchemaOverview> {
        let db_name = schema
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.default_database.clone());

        let db = self.client.database(&db_name);
        let mut cursor = db.list_collections().await.map_err(normalize_mongo_error)?;

        let mut tables = Vec::new();
        while cursor.advance().await.map_err(normalize_mongo_error)? {
            let collection = cursor
                .deserialize_current()
                .map_err(normalize_mongo_error)?;

            let columns = self
                .infer_collection_schema(&db_name, &collection.name)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|c| ColumnSchema {
                    name: c.name,
                    r#type: c.r#type,
                })
                .collect();

            tables.push(TableSchema {
                schema: db_name.clone(),
                name: collection.name,
                columns,
            });
        }

        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(SchemaOverview { tables })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_form(driver: &str, host: Option<&str>, port: Option<i64>) -> ConnectionForm {
        ConnectionForm {
            driver: driver.to_string(),
            host: host.map(|s| s.to_string()),
            port,
            ..Default::default()
        }
    }

    #[test]
    fn build_uri_basic() {
        let form = make_form("mongodb", Some("localhost"), Some(27017));
        let uri = build_connection_uri(&form).unwrap();
        assert_eq!(uri, "mongodb://localhost:27017");
    }

    #[test]
    fn build_uri_with_auth() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            username: Some("admin".to_string()),
            password: Some("pass word".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.starts_with("mongodb://admin:pass%20word@localhost:27017"));
    }

    #[test]
    fn build_uri_with_database() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            database: Some("mydb".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert_eq!(uri, "mongodb://localhost:27017/mydb");
    }

    #[test]
    fn build_uri_with_auth_source() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            username: Some("admin".to_string()),
            password: Some("pass".to_string()),
            auth_source: Some("admin".to_string()),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("authSource=admin"));
    }

    #[test]
    fn build_uri_with_ssl() {
        let form = ConnectionForm {
            driver: "mongodb".to_string(),
            host: Some("localhost".to_string()),
            port: Some(27017),
            ssl: Some(true),
            ..Default::default()
        };
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("ssl=true"));
    }

    #[test]
    fn build_uri_default_port() {
        let form = make_form("mongodb", Some("localhost"), None);
        let uri = build_connection_uri(&form).unwrap();
        assert!(uri.contains("localhost:27017"));
    }

    #[test]
    fn build_uri_missing_host() {
        let form = make_form("mongodb", None, None);
        assert!(build_connection_uri(&form).is_err());
    }

    #[test]
    fn build_uri_invalid_port() {
        let form = make_form("mongodb", Some("localhost"), Some(99999));
        assert!(build_connection_uri(&form).is_err());
    }

    #[test]
    fn normalize_error_categorization() {
        let auth_err = normalize_mongo_error("authentication failed");
        assert!(auth_err.to_string().contains("Authentication failed"));

        let dns_err = normalize_mongo_error("dns resolve error");
        assert!(dns_err.to_string().contains("DNS resolution failed"));

        let timeout_err = normalize_mongo_error("connection timed out");
        assert!(timeout_err.to_string().contains("Connection timed out"));

        let refused_err = normalize_mongo_error("connection refused");
        assert!(refused_err.to_string().contains("Connection refused"));

        let other_err = normalize_mongo_error("some other error");
        assert!(other_err.to_string().contains("MongoDB error"));
    }
}
