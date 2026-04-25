use crate::db::drivers::conn_failed_error;
use crate::models::ConnectionForm;
use redis::aio::ConnectionLike;
use redis::aio::MultiplexedConnection;
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use redis::cluster_routing::{MultipleNodeRoutingInfo, ResponsePolicy, RoutingInfo, SingleNodeRoutingInfo};
use redis::{
    from_redis_value, Cmd, ConnectionAddr, ConnectionInfo, FromRedisValue, ProtocolVersion,
    RedisConnectionInfo, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use base64::Engine;
use tokio::sync::Mutex as TokioMutex;

const DEFAULT_REDIS_PORT: i64 = 6379;
const DEFAULT_SCAN_LIMIT: u32 = 100;
const MAX_SCAN_LIMIT: u32 = 1000;
const PAGE_SIZE: isize = 200;

/// Shareable Redis connection handle.
/// Standalone uses MultiplexedConnection (Clone, shared underlying TCP).
/// Cluster wraps ClusterConnection in Arc<Mutex> so it can be shared across commands.
#[derive(Clone)]
pub enum RedisConnection {
    Standalone(MultiplexedConnection),
    Cluster(Arc<TokioMutex<ClusterConnection>>),
}

pub struct RedisConnectionCache {
    connections: HashMap<String, RedisConnection>,
}

impl RedisConnectionCache {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<RedisConnection> {
        self.connections.get(key).cloned()
    }

    pub fn insert(&mut self, key: String, conn: RedisConnection) {
        self.connections.insert(key, conn);
    }

    pub fn remove(&mut self, key: &str) {
        self.connections.remove(key);
    }

    /// Remove all cached connections that belong to `connection_id`
    /// (keys are formatted as `"{id}:{db}"` or `"{id}:cluster"`).
    pub fn remove_by_connection_id(&mut self, connection_id: i64) {
        let prefix = format!("{connection_id}:");
        self.connections.retain(|k, _| !k.starts_with(&prefix));
    }
}

impl RedisConnection {
    pub fn is_cluster(&self) -> bool {
        matches!(self, RedisConnection::Cluster(_))
    }

    pub async fn query<T: FromRedisValue>(&mut self, cmd: Cmd) -> Result<T, String> {
        match self {
            RedisConnection::Standalone(inner) => query_on(inner, cmd).await,
            RedisConnection::Cluster(arc) => {
                let mut conn = arc.lock().await;
                query_on(&mut *conn, cmd).await
            }
        }
    }

    pub async fn route_all_masters_combine_arrays<T: FromRedisValue>(
        &mut self,
        cmd: &Cmd,
    ) -> Result<T, String> {
        let RedisConnection::Cluster(arc) = self else {
            return Err("[REDIS_ERROR] all-master routing requires Redis Cluster".to_string());
        };
        let mut cluster = arc.lock().await;
        let value = cluster
            .route_command(
                cmd,
                RoutingInfo::MultiNode((
                    MultipleNodeRoutingInfo::AllMasters,
                    Some(ResponsePolicy::CombineArrays),
                )),
            )
            .await
            .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
        from_redis_value(&value).map_err(|e| format!("[REDIS_ERROR] {e}"))
    }

    pub async fn pipe_query<T: FromRedisValue>(
        &mut self,
        pipe: &mut redis::Pipeline,
    ) -> Result<T, String> {
        match self {
            RedisConnection::Standalone(inner) => pipe
                .query_async(inner)
                .await
                .map_err(|e| format!("[REDIS_ERROR] {e}")),
            RedisConnection::Cluster(arc) => {
                let mut conn = arc.lock().await;
                pipe.query_async(&mut *conn)
                    .await
                    .map_err(|e| format!("[REDIS_ERROR] {e}"))
            }
        }
    }

    pub async fn query_on_node<T: FromRedisValue>(
        &mut self,
        host: &str,
        port: u16,
        cmd: Cmd,
    ) -> Result<T, String> {
        let RedisConnection::Cluster(arc) = self else {
            return self.query(cmd).await;
        };
        let mut cluster = arc.lock().await;
        let routing = RoutingInfo::SingleNode(SingleNodeRoutingInfo::ByAddress {
            host: host.to_string(),
            port,
        });
        let value = cluster
            .route_command(&cmd, routing)
            .await
            .map_err(|e| format!("[REDIS_ERROR] {e}"))?;
        from_redis_value(&value).map_err(|e| format!("[REDIS_ERROR] {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisDatabaseInfo {
    pub index: i64,
    pub name: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyInfo {
    pub key: String,
    pub key_type: String,
    pub ttl: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisScanResponse {
    pub cursor: String,
    pub keys: Vec<RedisKeyInfo>,
    pub is_partial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisZSetMember {
    pub member: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum RedisValue {
    String(String),
    Hash(BTreeMap<String, String>),
    List(Vec<String>),
    Set(Vec<String>),
    ZSet(Vec<RedisZSetMember>),
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyValue {
    pub key: String,
    pub key_type: String,
    pub ttl: i64,
    pub value: RedisValue,
    pub value_total_len: Option<u64>,
    pub value_offset: u64,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSetKeyPayload {
    pub key: String,
    pub value: RedisValue,
    pub ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisMutationResult {
    pub success: bool,
    pub affected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisListSetItem {
    pub index: usize,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyPatchPayload {
    pub key: String,
    pub ttl_seconds: Option<i64>,
    pub hash_set: Option<BTreeMap<String, String>>,
    pub hash_del: Option<Vec<String>>,
    pub set_add: Option<Vec<String>>,
    pub set_rem: Option<Vec<String>>,
    pub zset_add: Option<Vec<RedisZSetMember>>,
    pub zset_rem: Option<Vec<String>>,
    pub list_rpush: Option<Vec<String>>,
    pub list_lpush: Option<Vec<String>>,
    pub list_set: Option<Vec<RedisListSetItem>>,
    pub list_rem: Option<Vec<String>>,
    pub list_lpop: Option<usize>,
    pub list_rpop: Option<usize>,
}

fn parse_database(database: Option<&str>) -> Result<i64, String> {
    let Some(raw) = database else {
        return Ok(0);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    let normalized = trimmed.strip_prefix("db").unwrap_or(trimmed);
    let db = normalized
        .parse::<i64>()
        .map_err(|_| "[VALIDATION_ERROR] Redis database must be a numeric index".to_string())?;
    if !(0..=255).contains(&db) {
        return Err("[VALIDATION_ERROR] Redis database must be between 0 and 255".to_string());
    }
    Ok(db)
}

fn selected_database(form: &ConnectionForm, database: Option<&str>) -> Result<i64, String> {
    match database {
        Some(db) => parse_database(Some(db)),
        None => parse_database(form.database.as_deref()),
    }
}

fn is_cluster_form(form: &ConnectionForm) -> bool {
    form.host
        .as_deref()
        .map(|host| {
            host.split(',')
                .filter(|part| !part.trim().is_empty())
                .count()
                > 1
        })
        .unwrap_or(false)
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("[VALIDATION_ERROR] Redis key cannot be empty".to_string());
    }
    Ok(())
}

fn validate_value_for_write(value: &RedisValue) -> Result<(), String> {
    match value {
        RedisValue::Hash(fields) if fields.is_empty() => {
            Err("[VALIDATION_ERROR] Redis hash must contain at least one field".into())
        }
        RedisValue::List(items) if items.is_empty() => {
            Err("[VALIDATION_ERROR] Redis list must contain at least one item".into())
        }
        RedisValue::Set(items) if items.is_empty() => {
            Err("[VALIDATION_ERROR] Redis set must contain at least one member".into())
        }
        RedisValue::ZSet(items) if items.is_empty() => {
            Err("[VALIDATION_ERROR] Redis zset must contain at least one member".into())
        }
        RedisValue::None => Err("[VALIDATION_ERROR] Redis value is required".into()),
        _ => Ok(()),
    }
}

fn parse_host_port(raw: &str, fallback_port: i64) -> Result<(String, i64), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("[VALIDATION_ERROR] Redis host is required".to_string());
    }
    if trimmed.starts_with('[') {
        return Ok((trimmed.to_string(), fallback_port));
    }
    let mut parts = trimmed.rsplitn(2, ':');
    let port_part = parts.next().unwrap_or_default();
    let host_part = parts.next();
    if let Some(host) = host_part {
        if !host.is_empty() && port_part.chars().all(|c| c.is_ascii_digit()) {
            let port = port_part
                .parse::<i64>()
                .map_err(|_| "[VALIDATION_ERROR] Redis port is invalid".to_string())?;
            return Ok((host.to_string(), port));
        }
    }
    Ok((trimmed.to_string(), fallback_port))
}

fn build_connection_info_for_host(
    form: &ConnectionForm,
    host: &str,
    db: i64,
) -> Result<ConnectionInfo, String> {
    let (host, port) = parse_host_port(host, form.port.unwrap_or(DEFAULT_REDIS_PORT))?;
    if !(1..=65535).contains(&port) {
        return Err("[VALIDATION_ERROR] Redis port must be between 1 and 65535".to_string());
    }

    let addr = if form.ssl.unwrap_or(false) {
        ConnectionAddr::TcpTls {
            host,
            port: port as u16,
            insecure: false,
            tls_params: None,
        }
    } else {
        ConnectionAddr::Tcp(host, port as u16)
    };

    Ok(ConnectionInfo {
        addr,
        redis: RedisConnectionInfo {
            db,
            username: form
                .username
                .as_deref()
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            password: form
                .password
                .as_deref()
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            protocol: ProtocolVersion::RESP2,
        },
    })
}

fn build_connection_info(form: &ConnectionForm, db: i64) -> Result<ConnectionInfo, String> {
    let host = form
        .host
        .as_deref()
        .filter(|h| !h.trim().is_empty())
        .ok_or_else(|| "[VALIDATION_ERROR] Redis host is required".to_string())?;
    build_connection_info_for_host(form, host, db)
}

fn build_cluster_nodes(form: &ConnectionForm) -> Result<Vec<ConnectionInfo>, String> {
    let db = selected_database(form, None)?;
    if db != 0 {
        return Err("[VALIDATION_ERROR] Redis Cluster only supports database 0".to_string());
    }
    let host = form
        .host
        .as_deref()
        .filter(|h| !h.trim().is_empty())
        .ok_or_else(|| "[VALIDATION_ERROR] Redis host is required".to_string())?;
    let nodes: Vec<ConnectionInfo> = host
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| build_connection_info_for_host(form, part, 0))
        .collect::<Result<_, _>>()?;
    if nodes.len() < 2 {
        return Err(
            "[VALIDATION_ERROR] Redis Cluster requires at least two seed nodes".to_string(),
        );
    }
    Ok(nodes)
}

pub async fn connect(form: &ConnectionForm, database: Option<&str>) -> Result<RedisConnection, String> {
    if is_cluster_form(form) {
        if let Some(db) = database {
            if parse_database(Some(db))? != 0 {
                return Err("[VALIDATION_ERROR] Redis Cluster only supports database 0".to_string());
            }
        }
        let nodes = build_cluster_nodes(form)?;
        let client = ClusterClient::new(nodes).map_err(|e| conn_failed_error(&e))?;
        let conn = client
            .get_async_connection()
            .await
            .map_err(|e| conn_failed_error(&e))?;
        return Ok(RedisConnection::Cluster(Arc::new(TokioMutex::new(conn))));
    }

    let db = selected_database(form, database)?;
    let info = build_connection_info(form, db)?;
    let client = redis::Client::open(info).map_err(|e| conn_failed_error(&e))?;
    let conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| conn_failed_error(&e))?;
    Ok(RedisConnection::Standalone(conn))
}

async fn query_on<T: FromRedisValue, C: ConnectionLike + Send + Sync>(
    conn: &mut C,
    cmd: Cmd,
) -> Result<T, String> {
    cmd.query_async::<T>(conn)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))
}

pub async fn ping(conn: &mut RedisConnection) -> Result<(), String> {
    conn.query::<String>(redis::cmd("PING"))
        .await
        .map(|_| ())
        .map_err(|e| conn_failed_error(&e))
}

pub fn list_databases(form: &ConnectionForm, db_count: i64) -> Result<Vec<RedisDatabaseInfo>, String> {
    if is_cluster_form(form) {
        build_cluster_nodes(form)?;
        return Ok(vec![RedisDatabaseInfo {
            index: 0,
            name: "db0".to_string(),
            selected: true,
        }]);
    }

    let selected = selected_database(form, None)?;
    let db_count = db_count.clamp(1, 256);
    Ok((0..db_count)
        .map(|index| RedisDatabaseInfo {
            index,
            name: format!("db{index}"),
            selected: index == selected,
        })
        .collect())
}

fn encode_cluster_scan_state(cursors: &HashMap<String, u64>) -> Result<String, String> {
    let json = serde_json::to_string(cursors)
        .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
    Ok(base64::prelude::BASE64_STANDARD.encode(json.as_bytes()))
}

fn decode_cluster_scan_state(s: &str) -> Result<HashMap<String, u64>, String> {
    let bytes = base64::prelude::BASE64_STANDARD
        .decode(s)
        .map_err(|e| format!("[REDIS_SCAN_ERROR] Invalid cursor: {e}"))?;
    let json = String::from_utf8(bytes)
        .map_err(|e| format!("[REDIS_SCAN_ERROR] Invalid cursor: {e}"))?;
    serde_json::from_str(&json)
        .map_err(|e| format!("[REDIS_SCAN_ERROR] Invalid cursor: {e}"))
}

async fn get_cluster_master_nodes(
    conn: &mut RedisConnection,
) -> Result<Vec<(String, u16)>, String> {
    let mut cmd = redis::cmd("CLUSTER");
    cmd.arg("SLOTS");
    let value: Value = conn.query(cmd).await?;

    let slots = match value {
        Value::Array(arr) => arr,
        _ => {
            return Err(
                "[REDIS_SCAN_ERROR] Unexpected CLUSTER SLOTS response".to_string(),
            )
        }
    };

    let mut masters = Vec::new();
    for slot in slots {
        let slot_arr = match slot {
            Value::Array(arr) => arr,
            _ => continue,
        };
        if slot_arr.len() < 3 {
            continue;
        }
        let master_info = match &slot_arr[2] {
            Value::Array(info) => info,
            _ => continue,
        };
        if master_info.len() < 2 {
            continue;
        }
        let host = from_redis_value::<String>(&master_info[0])
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        let port = from_redis_value::<u16>(&master_info[1])
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        masters.push((host, port));
    }

    masters.sort();
    masters.dedup();
    Ok(masters)
}

fn parse_node_addr(addr: &str) -> Result<(&str, u16), String> {
    let mut parts = addr.rsplitn(2, ':');
    let port_part = parts.next().ok_or_else(|| "[REDIS_SCAN_ERROR] Invalid node addr".to_string())?;
    let host_part = parts.next().ok_or_else(|| "[REDIS_SCAN_ERROR] Invalid node addr".to_string())?;
    let port = port_part
        .parse::<u16>()
        .map_err(|_| "[REDIS_SCAN_ERROR] Invalid node port".to_string())?;
    Ok((host_part, port))
}

fn is_dangerous_wildcard(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return true;
    }
    !trimmed.chars().any(|c| c.is_alphanumeric())
}

async fn scan_cluster_keys(
    conn: &mut RedisConnection,
    state: Option<&str>,
    pattern: &str,
    count: u32,
) -> Result<(Vec<String>, String, bool), String> {
    if is_dangerous_wildcard(pattern) {
        return Err("[VALIDATION_ERROR] Cluster scan requires a non-wildcard pattern".to_string());
    }
    let masters = get_cluster_master_nodes(conn).await?;

    let mut cursors: HashMap<String, u64> = match state {
        Some(s) => decode_cluster_scan_state(s)?,
        None => HashMap::new(),
    };

    // Seed any newly-discovered masters with cursor 0
    for (host, port) in &masters {
        cursors
            .entry(format!("{host}:{port}"))
            .or_insert(0);
    }

    let mut keys: Vec<String> = Vec::new();
    let mut addresses: Vec<String> = cursors.keys().cloned().collect();
    addresses.sort();

    // Scan every master that still has a non-zero cursor once per call.
    for addr in &addresses {
        let cursor = cursors.get(addr).copied().unwrap_or(0);
        if cursor == 0 {
            continue;
        }
        let (host, port) = parse_node_addr(addr)?;
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count);
        let (next_cursor, node_keys): (u64, Vec<String>) = conn
            .query_on_node(host, port, cmd)
            .await
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        keys.extend(node_keys);
        cursors.insert(addr.clone(), next_cursor);
    }

    // If we still haven't reached the limit and some nodes remain unfinished,
    // perform one more round to be more eager.
    if keys.len() < count as usize {
        for addr in &addresses {
            let cursor = cursors.get(addr).copied().unwrap_or(0);
            if cursor == 0 {
                continue;
            }
            let (host, port) = parse_node_addr(addr)?;
            let mut cmd = redis::cmd("SCAN");
            cmd.arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(count);
            let (next_cursor, node_keys): (u64, Vec<String>) = conn
                .query_on_node(host, port, cmd)
                .await
                .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
            keys.extend(node_keys);
            cursors.insert(addr.clone(), next_cursor);
            if keys.len() >= count as usize {
                break;
            }
        }
    }

    keys.sort();
    keys.truncate(count as usize);

    let is_partial = cursors.values().any(|c| *c != 0);
    let next_state = encode_cluster_scan_state(&cursors)?;
    Ok((keys, next_state, is_partial))
}

pub async fn scan_keys(
    conn: &mut RedisConnection,
    cursor: Option<String>,
    pattern: Option<String>,
    limit: Option<u32>,
) -> Result<RedisScanResponse, String> {
    let count = limit.unwrap_or(DEFAULT_SCAN_LIMIT).clamp(1, MAX_SCAN_LIMIT);
    let match_pattern = pattern
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or("*");

    let (next_cursor, is_partial, keys): (String, bool, Vec<String>) = if conn.is_cluster() {
        let (keys, next_state, partial) =
            scan_cluster_keys(conn, cursor.as_deref(), match_pattern, count).await?;
        (next_state, partial, keys)
    } else {
        let scan_cursor: u64 = cursor
            .as_deref()
            .unwrap_or("0")
            .parse()
            .map_err(|_| "[VALIDATION_ERROR] Invalid cursor".to_string())?;
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(scan_cursor)
            .arg("MATCH")
            .arg(match_pattern)
            .arg("COUNT")
            .arg(count);
        let (next_cursor, keys): (u64, Vec<String>) = conn
            .query(cmd)
            .await
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        let partial = next_cursor != 0;
        (next_cursor.to_string(), partial, keys)
    };

    let out = if keys.is_empty() {
        Vec::new()
    } else {
        let mut pipe = redis::pipe();
        for key in &keys {
            pipe.cmd("TYPE").arg(key);
            pipe.cmd("TTL").arg(key);
        }
        let results: Vec<Value> = conn
            .pipe_query(&mut pipe)
            .await
            .map_err(|e| format!("[REDIS_SCAN_ERROR] {e}"))?;
        keys.into_iter()
            .enumerate()
            .map(|(i, key)| {
                let key_type = from_redis_value(results.get(i * 2).unwrap_or(&Value::Nil))
                    .unwrap_or_else(|_| "unknown".to_string());
                let ttl =
                    from_redis_value(results.get(i * 2 + 1).unwrap_or(&Value::Nil)).unwrap_or(-2);
                RedisKeyInfo { key, key_type, ttl }
            })
            .collect()
    };

    Ok(RedisScanResponse {
        cursor: next_cursor,
        keys: out,
        is_partial,
    })
}

pub async fn get_key(
    conn: &mut RedisConnection,
    key: String,
) -> Result<RedisKeyValue, String> {
    validate_key(&key)?;

    let mut pipe1 = redis::pipe();
    pipe1.cmd("TYPE").arg(&key).cmd("TTL").arg(&key);
    let (key_type, ttl): (String, i64) = conn
        .pipe_query(&mut pipe1)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let page = PAGE_SIZE - 1;
    let (value, value_total_len, value_offset, is_binary): (RedisValue, Option<u64>, u64, bool) =
        match key_type.as_str() {
            "none" => (RedisValue::None, None, 0, false),
            "string" => {
                let mut cmd = redis::cmd("GET");
                cmd.arg(&key);
                let bytes: Vec<u8> = conn.query(cmd).await.unwrap_or_default();
                let (text, is_binary) = match String::from_utf8(bytes) {
                    Ok(s) => (s, false),
                    Err(e) => {
                        let encoded = base64::prelude::BASE64_STANDARD.encode(e.into_bytes());
                        (encoded, true)
                    }
                };
                (
                    RedisValue::String(text),
                    None,
                    0,
                    is_binary,
                )
            }
            "hash" => {
                let mut pipe = redis::pipe();
                pipe.cmd("HLEN")
                    .arg(&key)
                    .cmd("HSCAN")
                    .arg(&key)
                    .arg(0u64)
                    .arg("COUNT")
                    .arg(PAGE_SIZE);
                let (total, (next_cursor, fields)): (u64, (u64, BTreeMap<String, String>)) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                (RedisValue::Hash(fields), Some(total), next_cursor, false)
            }
            "list" => {
                let mut pipe = redis::pipe();
                pipe.cmd("LLEN")
                    .arg(&key)
                    .cmd("LRANGE")
                    .arg(&key)
                    .arg(0)
                    .arg(page);
                let (total, items): (u64, Vec<String>) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                let next_offset = (items.len() as u64).min(total);
                (RedisValue::List(items), Some(total), next_offset, false)
            }
            "set" => {
                let mut pipe = redis::pipe();
                pipe.cmd("SCARD")
                    .arg(&key)
                    .cmd("SSCAN")
                    .arg(&key)
                    .arg(0u64)
                    .arg("COUNT")
                    .arg(PAGE_SIZE);
                let (total, (next_cursor, members)): (u64, (u64, Vec<String>)) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                (RedisValue::Set(members), Some(total), next_cursor, false)
            }
            "zset" => {
                let mut pipe = redis::pipe();
                pipe.cmd("ZCARD")
                    .arg(&key)
                    .cmd("ZRANGE")
                    .arg(&key)
                    .arg(0)
                    .arg(page)
                    .arg("WITHSCORES");
                let (total, members): (u64, Vec<(String, f64)>) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                let next_offset = (members.len() as u64).min(total);
                (
                    RedisValue::ZSet(
                        members
                            .into_iter()
                            .map(|(member, score)| RedisZSetMember { member, score })
                            .collect(),
                    ),
                    Some(total),
                    next_offset,
                    false,
                )
            }
            other => {
                return Err(format!(
                    "[UNSUPPORTED] Redis type '{other}' is not supported"
                ))
            }
        };

    Ok(RedisKeyValue {
        key,
        key_type,
        ttl,
        value,
        value_total_len,
        value_offset,
        is_binary,
    })
}

pub async fn get_key_page(
    conn: &mut RedisConnection,
    key: String,
    offset: u64,
    limit: u32,
) -> Result<RedisKeyValue, String> {
    validate_key(&key)?;
    let limit = limit.clamp(1, MAX_SCAN_LIMIT);

    let mut pipe1 = redis::pipe();
    pipe1.cmd("TYPE").arg(&key).cmd("TTL").arg(&key);
    let (key_type, ttl): (String, i64) = conn
        .pipe_query(&mut pipe1)
        .await
        .map_err(|e| format!("[REDIS_ERROR] {e}"))?;

    let end = offset.saturating_add(limit as u64).saturating_sub(1);

    let (value, value_total_len, value_offset): (RedisValue, Option<u64>, u64) =
        match key_type.as_str() {
            "list" => {
                let mut pipe = redis::pipe();
                pipe.cmd("LLEN")
                    .arg(&key)
                    .cmd("LRANGE")
                    .arg(&key)
                    .arg(offset)
                    .arg(end);
                let (total, items): (u64, Vec<String>) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                let next_offset = offset.saturating_add(items.len() as u64).min(total);
                (RedisValue::List(items), Some(total), next_offset)
            }
            "zset" => {
                let mut pipe = redis::pipe();
                pipe.cmd("ZCARD")
                    .arg(&key)
                    .cmd("ZRANGE")
                    .arg(&key)
                    .arg(offset)
                    .arg(end)
                    .arg("WITHSCORES");
                let (total, members): (u64, Vec<(String, f64)>) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                let next_offset = offset.saturating_add(members.len() as u64).min(total);
                (
                    RedisValue::ZSet(
                        members
                            .into_iter()
                            .map(|(member, score)| RedisZSetMember { member, score })
                            .collect(),
                    ),
                    Some(total),
                    next_offset,
                )
            }
            "hash" => {
                let mut pipe = redis::pipe();
                pipe.cmd("HLEN")
                    .arg(&key)
                    .cmd("HSCAN")
                    .arg(&key)
                    .arg(offset)
                    .arg("COUNT")
                    .arg(limit);
                let (total, (next_cursor, fields)): (u64, (u64, BTreeMap<String, String>)) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                (RedisValue::Hash(fields), Some(total), next_cursor)
            }
            "set" => {
                let mut pipe = redis::pipe();
                pipe.cmd("SCARD")
                    .arg(&key)
                    .cmd("SSCAN")
                    .arg(&key)
                    .arg(offset)
                    .arg("COUNT")
                    .arg(limit);
                let (total, (next_cursor, members)): (u64, (u64, Vec<String>)) =
                    conn.pipe_query(&mut pipe).await.unwrap_or_default();
                (RedisValue::Set(members), Some(total), next_cursor)
            }
            "string" | "none" => {
                return get_key(conn, key).await;
            }
            other => {
                return Err(format!(
                    "[UNSUPPORTED] Redis type '{other}' is not supported"
                ))
            }
        };

    Ok(RedisKeyValue {
        key,
        key_type,
        ttl,
        value,
        value_total_len,
        value_offset,
        is_binary: false,
    })
}

pub async fn set_key(
    conn: &mut RedisConnection,
    payload: RedisSetKeyPayload,
) -> Result<RedisMutationResult, String> {
    validate_key(&payload.key)?;
    validate_value_for_write(&payload.value)?;
    let mut del_cmd = redis::cmd("DEL");
    del_cmd.arg(&payload.key);
    let _: i64 = conn.query(del_cmd).await.unwrap_or(0);

    match payload.value {
        RedisValue::String(value) => {
            let mut cmd = redis::cmd("SET");
            cmd.arg(&payload.key).arg(value);
            conn.query::<()>(cmd).await?;
        }
        RedisValue::Hash(fields) => {
            let mut cmd = redis::cmd("HSET");
            cmd.arg(&payload.key);
            for (field, value) in fields {
                cmd.arg(field).arg(value);
            }
            conn.query::<i64>(cmd).await?;
        }
        RedisValue::List(items) => {
            let mut cmd = redis::cmd("RPUSH");
            cmd.arg(&payload.key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
        RedisValue::Set(items) => {
            let mut cmd = redis::cmd("SADD");
            cmd.arg(&payload.key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
        RedisValue::ZSet(items) => {
            for item in items {
                let mut cmd = redis::cmd("ZADD");
                cmd.arg(&payload.key).arg(item.score).arg(item.member);
                conn.query::<i64>(cmd).await?;
            }
        }
        RedisValue::None => unreachable!("validated above"),
    }

    if let Some(ttl) = payload.ttl_seconds {
        if ttl > 0 {
            let mut cmd = redis::cmd("EXPIRE");
            cmd.arg(&payload.key).arg(ttl);
            conn.query::<bool>(cmd).await?;
        }
    }

    Ok(RedisMutationResult {
        success: true,
        affected: 1,
    })
}

pub async fn delete_key(
    conn: &mut RedisConnection,
    key: String,
) -> Result<RedisMutationResult, String> {
    validate_key(&key)?;
    let mut cmd = redis::cmd("DEL");
    cmd.arg(key);
    let affected: i64 = conn.query(cmd).await?;
    Ok(RedisMutationResult {
        success: true,
        affected,
    })
}

pub async fn patch_key(
    conn: &mut RedisConnection,
    payload: RedisKeyPatchPayload,
) -> Result<RedisMutationResult, String> {
    validate_key(&payload.key)?;
    let key = &payload.key;

    if let Some(fields) = payload.hash_set {
        if !fields.is_empty() {
            let mut cmd = redis::cmd("HSET");
            cmd.arg(key);
            for (f, v) in fields {
                cmd.arg(f).arg(v);
            }
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(fields) = payload.hash_del {
        if !fields.is_empty() {
            let mut cmd = redis::cmd("HDEL");
            cmd.arg(key);
            for f in fields {
                cmd.arg(f);
            }
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.set_add {
        if !members.is_empty() {
            let mut cmd = redis::cmd("SADD");
            cmd.arg(key).arg(members);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.set_rem {
        if !members.is_empty() {
            let mut cmd = redis::cmd("SREM");
            cmd.arg(key).arg(members);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.zset_add {
        if !members.is_empty() {
            let mut cmd = redis::cmd("ZADD");
            cmd.arg(key);
            for m in members {
                cmd.arg(m.score).arg(m.member);
            }
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(members) = payload.zset_rem {
        if !members.is_empty() {
            let mut cmd = redis::cmd("ZREM");
            cmd.arg(key).arg(members);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(items) = payload.list_rpush {
        if !items.is_empty() {
            let mut cmd = redis::cmd("RPUSH");
            cmd.arg(key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(items) = payload.list_lpush {
        if !items.is_empty() {
            let mut cmd = redis::cmd("LPUSH");
            cmd.arg(key).arg(items);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(items) = payload.list_set {
        for item in items {
            let mut cmd = redis::cmd("LSET");
            cmd.arg(key).arg(item.index).arg(item.value);
            conn.query::<()>(cmd).await?;
        }
    }
    if let Some(values) = payload.list_rem {
        for value in values {
            let mut cmd = redis::cmd("LREM");
            cmd.arg(key).arg(0).arg(value);
            conn.query::<i64>(cmd).await?;
        }
    }
    if let Some(count) = payload.list_lpop {
        if count > 0 {
            let mut cmd = redis::cmd("LPOP");
            cmd.arg(key).arg(count);
            conn.query::<()>(cmd).await?;
        }
    }
    if let Some(count) = payload.list_rpop {
        if count > 0 {
            let mut cmd = redis::cmd("RPOP");
            cmd.arg(key).arg(count);
            conn.query::<()>(cmd).await?;
        }
    }

    match payload.ttl_seconds {
        Some(ttl) if ttl > 0 => {
            let mut cmd = redis::cmd("EXPIRE");
            cmd.arg(key).arg(ttl);
            conn.query::<bool>(cmd).await?;
        }
        Some(_) => {
            // Caller sends 0 or negative to explicitly remove TTL.
            let mut cmd = redis::cmd("PERSIST");
            cmd.arg(key);
            conn.query::<bool>(cmd).await?;
        }
        None => {
            // None means "leave TTL unchanged" — no action.
        }
    }

    Ok(RedisMutationResult {
        success: true,
        affected: 1,
    })
}

pub async fn rename_key(
    conn: &mut RedisConnection,
    old_key: String,
    new_key: String,
    force: bool,
) -> Result<RedisMutationResult, String> {
    validate_key(&old_key)?;
    validate_key(&new_key)?;
    let cmd_name = if force { "RENAME" } else { "RENAMENX" };
    let mut cmd = redis::cmd(cmd_name);
    cmd.arg(&old_key).arg(&new_key);
    let renamed: i64 = conn.query(cmd).await?;
    if renamed == 0 && !force {
        return Err(format!(
            "[REDIS_ERROR] Key '{}' already exists. RENAMENX refused to overwrite.",
            new_key
        ));
    }
    Ok(RedisMutationResult {
        success: true,
        affected: 1,
    })
}

pub async fn set_ttl(
    conn: &mut RedisConnection,
    key: String,
    ttl_seconds: Option<i64>,
) -> Result<RedisMutationResult, String> {
    validate_key(&key)?;
    let changed: bool = match ttl_seconds {
        Some(ttl) if ttl > 0 => {
            let mut cmd = redis::cmd("EXPIRE");
            cmd.arg(key).arg(ttl);
            conn.query(cmd).await?
        }
        _ => {
            let mut cmd = redis::cmd("PERSIST");
            cmd.arg(key);
            conn.query(cmd).await?
        }
    };
    Ok(RedisMutationResult {
        success: true,
        affected: if changed { 1 } else { 0 },
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisRawResult {
    pub output: String,
}

fn tokenize_command(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace()) {
            chars.next();
        }
        match chars.peek() {
            None => break,
            Some('"') => {
                chars.next();
                let mut tok = String::new();
                loop {
                    match chars.next() {
                        None => return Err("Unterminated double quote in command".to_string()),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            None => return Err("Unexpected end after backslash".to_string()),
                            Some(c) => tok.push(c),
                        },
                        Some(c) => tok.push(c),
                    }
                }
                tokens.push(tok);
            }
            Some('\'') => {
                chars.next();
                let mut tok = String::new();
                loop {
                    match chars.next() {
                        None => return Err("Unterminated single quote in command".to_string()),
                        Some('\'') => break,
                        Some(c) => tok.push(c),
                    }
                }
                tokens.push(tok);
            }
            Some(_) => {
                let mut tok = String::new();
                while chars.peek().map_or(false, |c| !c.is_whitespace()) {
                    tok.push(chars.next().unwrap());
                }
                tokens.push(tok);
            }
        }
    }
    Ok(tokens)
}

fn format_redis_value(value: Value) -> String {
    match value {
        Value::Nil => "(nil)".to_string(),
        Value::Okay => "OK".to_string(),
        Value::Int(n) => format!("(integer) {n}"),
        Value::BulkString(bytes) => match String::from_utf8(bytes) {
            Ok(s) => format!("\"{s}\""),
            Err(e) => format!("(binary {} bytes)", e.into_bytes().len()),
        },
        Value::SimpleString(s) => s,
        Value::Array(items) => {
            if items.is_empty() {
                return "(empty array)".to_string();
            }
            items
                .into_iter()
                .enumerate()
                .map(|(i, v)| format!("{}) {}", i + 1, format_redis_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Map(pairs) => {
            if pairs.is_empty() {
                return "(empty map)".to_string();
            }
            pairs
                .into_iter()
                .enumerate()
                .flat_map(|(i, (k, v))| {
                    [
                        format!("{}) {}", i * 2 + 1, format_redis_value(k)),
                        format!("{}) {}", i * 2 + 2, format_redis_value(v)),
                    ]
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Set(items) => {
            if items.is_empty() {
                return "(empty set)".to_string();
            }
            items
                .into_iter()
                .enumerate()
                .map(|(i, v)| format!("{}) {}", i + 1, format_redis_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Double(f) => format!("(double) {f}"),
        Value::Boolean(b) => format!("(boolean) {b}"),
        Value::VerbatimString { text, .. } => format!("\"{text}\""),
        Value::Attribute { data, .. } => format_redis_value(*data),
        Value::Push { data, .. } => {
            if data.is_empty() {
                return "(empty push)".to_string();
            }
            data.into_iter()
                .enumerate()
                .map(|(i, v)| format!("{}) {}", i + 1, format_redis_value(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::BigNumber(n) => format!("(big number) {n}"),
        Value::ServerError(e) => format!("(error) {:?}", e),
    }
}

pub async fn execute_raw(
    conn: &mut RedisConnection,
    command: String,
) -> Result<RedisRawResult, String> {
    let tokens = tokenize_command(&command)?;
    if tokens.is_empty() {
        return Err("[VALIDATION_ERROR] Command cannot be empty".to_string());
    }
    let mut cmd = redis::cmd(&tokens[0]);
    for arg in &tokens[1..] {
        cmd.arg(arg.as_str());
    }
    let value: Value = conn.query(cmd).await?;
    Ok(RedisRawResult {
        output: format_redis_value(value),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_cluster_nodes, build_connection_info, is_cluster_form, list_databases,
        parse_database, validate_value_for_write, RedisValue,
    };
    use crate::models::ConnectionForm;
    use redis::ConnectionAddr;

    #[test]
    fn parse_database_accepts_db_prefix() {
        assert_eq!(parse_database(Some("db3")).unwrap(), 3);
        assert_eq!(parse_database(Some(" 4 ")).unwrap(), 4);
        assert_eq!(parse_database(None).unwrap(), 0);
    }

    #[test]
    fn parse_database_rejects_invalid_index() {
        assert!(parse_database(Some("abc")).is_err());
        assert!(parse_database(Some("256")).is_err());
    }

    #[test]
    fn redis_connection_info_preserves_acl_credentials() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("localhost".to_string()),
            port: Some(6379),
            username: Some("app".to_string()),
            password: Some("secret".to_string()),
            ..ConnectionForm::default()
        };
        let info = build_connection_info(&form, 2).unwrap();
        assert_eq!(info.redis.db, 2);
        assert_eq!(info.redis.username.as_deref(), Some("app"));
        assert_eq!(info.redis.password.as_deref(), Some("secret"));
        assert!(matches!(info.addr, ConnectionAddr::Tcp(_, 6379)));
    }

    #[test]
    fn list_databases_marks_selected_index() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            database: Some("db5".to_string()),
            ..ConnectionForm::default()
        };
        let dbs = list_databases(&form, 16).unwrap();
        assert_eq!(dbs.len(), 16);
        assert!(dbs[5].selected);
    }

    #[test]
    fn comma_separated_hosts_enable_cluster_mode() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("10.0.0.1:6379,10.0.0.2:6380".to_string()),
            ..ConnectionForm::default()
        };
        assert!(is_cluster_form(&form));
        let nodes = build_cluster_nodes(&form).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn cluster_mode_rejects_non_zero_database() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("10.0.0.1:6379,10.0.0.2:6380".to_string()),
            database: Some("db1".to_string()),
            ..ConnectionForm::default()
        };
        assert!(build_cluster_nodes(&form).is_err());
    }

    #[test]
    fn password_is_optional_for_connection_info() {
        let form = ConnectionForm {
            driver: "redis".to_string(),
            host: Some("localhost".to_string()),
            port: Some(6379),
            ..ConnectionForm::default()
        };
        let info = build_connection_info(&form, 0).unwrap();
        assert!(info.redis.username.is_none());
        assert!(info.redis.password.is_none());
    }

    #[test]
    fn empty_collection_values_are_rejected_before_write() {
        assert!(validate_value_for_write(&RedisValue::Hash(Default::default())).is_err());
        assert!(validate_value_for_write(&RedisValue::List(vec![])).is_err());
        assert!(validate_value_for_write(&RedisValue::Set(vec![])).is_err());
        assert!(validate_value_for_write(&RedisValue::ZSet(vec![])).is_err());
        assert!(validate_value_for_write(&RedisValue::String(String::new())).is_ok());
    }

    use super::{format_redis_value, tokenize_command};
    use redis::Value;

    // tokenize_command

    #[test]
    fn tokenize_simple_command() {
        assert_eq!(tokenize_command("GET mykey").unwrap(), vec!["GET", "mykey"]);
    }

    #[test]
    fn tokenize_trims_extra_whitespace() {
        assert_eq!(
            tokenize_command("  SET  foo  bar  ").unwrap(),
            vec!["SET", "foo", "bar"]
        );
    }

    #[test]
    fn tokenize_double_quoted_value_with_spaces() {
        assert_eq!(
            tokenize_command(r#"SET key "hello world""#).unwrap(),
            vec!["SET", "key", "hello world"]
        );
    }

    #[test]
    fn tokenize_single_quoted_value() {
        assert_eq!(
            tokenize_command("SET key 'hello world'").unwrap(),
            vec!["SET", "key", "hello world"]
        );
    }

    #[test]
    fn tokenize_backslash_escape_in_double_quotes() {
        assert_eq!(
            tokenize_command(r#"SET key "say \"hi\"""#).unwrap(),
            vec!["SET", "key", r#"say "hi""#]
        );
    }

    #[test]
    fn tokenize_empty_string_returns_empty_vec() {
        assert_eq!(tokenize_command("").unwrap(), Vec::<String>::new());
        assert_eq!(tokenize_command("   ").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn tokenize_unterminated_double_quote_is_error() {
        assert!(tokenize_command(r#"SET key "unclosed"#).is_err());
    }

    #[test]
    fn tokenize_unterminated_single_quote_is_error() {
        assert!(tokenize_command("SET key 'unclosed").is_err());
    }

    // format_redis_value

    #[test]
    fn format_nil() {
        assert_eq!(format_redis_value(Value::Nil), "(nil)");
    }

    #[test]
    fn format_okay() {
        assert_eq!(format_redis_value(Value::Okay), "OK");
    }

    #[test]
    fn format_integer() {
        assert_eq!(format_redis_value(Value::Int(42)), "(integer) 42");
        assert_eq!(format_redis_value(Value::Int(-1)), "(integer) -1");
    }

    #[test]
    fn format_bulk_string_utf8() {
        assert_eq!(
            format_redis_value(Value::BulkString(b"hello".to_vec())),
            "\"hello\""
        );
    }

    #[test]
    fn format_bulk_string_binary() {
        let bytes = vec![0xc3, 0x28]; // invalid UTF-8
        let out = format_redis_value(Value::BulkString(bytes));
        assert!(out.starts_with("(binary "));
        assert!(out.ends_with(" bytes)"));
    }

    #[test]
    fn format_simple_string() {
        assert_eq!(
            format_redis_value(Value::SimpleString("PONG".to_string())),
            "PONG"
        );
    }

    #[test]
    fn format_empty_array() {
        assert_eq!(format_redis_value(Value::Array(vec![])), "(empty array)");
    }

    #[test]
    fn format_array_with_items() {
        let items = vec![
            Value::BulkString(b"a".to_vec()),
            Value::BulkString(b"b".to_vec()),
        ];
        let out = format_redis_value(Value::Array(items));
        assert_eq!(out, "1) \"a\"\n2) \"b\"");
    }

    #[test]
    fn format_nested_array() {
        let inner = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let outer = Value::Array(vec![inner, Value::Nil]);
        let out = format_redis_value(outer);
        assert_eq!(out, "1) 1) (integer) 1\n2) (integer) 2\n2) (nil)");
    }
}
