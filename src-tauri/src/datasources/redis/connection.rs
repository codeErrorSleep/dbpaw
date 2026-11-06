use crate::db::drivers::conn_failed_error;
use crate::models::ConnectionForm;
use redis::aio::{ConnectionLike, MultiplexedConnection};
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use redis::cluster_routing::{
    MultipleNodeRoutingInfo, ResponsePolicy, RoutingInfo, SingleNodeRoutingInfo,
};
use redis::sentinel::{Sentinel, SentinelNodeConnectionInfo};
use redis::{
    AsyncConnectionConfig, Cmd, ConnectionAddr, ConnectionInfo, FromRedisValue, ProtocolVersion,
    RedisConnectionInfo, TlsMode,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;

const DEFAULT_REDIS_PORT: i64 = 6379;
const DEFAULT_CONNECT_TIMEOUT_MS: i64 = 5000;

/// Shareable Redis connection handle.
/// Standalone uses MultiplexedConnection (Clone, shared underlying TCP).
/// Cluster wraps ClusterConnection in Arc<Mutex> so it can be shared across commands.
#[derive(Clone)]
pub enum RedisConnection {
    Standalone(MultiplexedConnection, Option<crate::network::NetworkTunnel>),
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

    pub async fn query<T: FromRedisValue>(&mut self, cmd: Cmd) -> error::RedisResult<T> {
        match self {
            RedisConnection::Standalone(inner, _) => query_on(inner, cmd).await,
            RedisConnection::Cluster(arc) => {
                let mut conn = arc.lock().await;
                query_on(&mut *conn, cmd).await
            }
        }
    }

    pub async fn route_all_masters_combine_arrays<T: FromRedisValue>(
        &mut self,
        cmd: &Cmd,
    ) -> error::RedisResult<T> {
        let RedisConnection::Cluster(arc) = self else {
            return Err(error::command("all-master routing requires Redis Cluster"));
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
            .map_err(|e| error::to_command_error(e))?;
        from_redis_value(&value).map_err(|e| error::to_command_error(e))
    }

    pub async fn pipe_query<T: FromRedisValue>(
        &mut self,
        pipe: &mut redis::Pipeline,
    ) -> error::RedisResult<T> {
        match self {
            RedisConnection::Standalone(inner, _) => pipe
                .query_async(inner)
                .await
                .map_err(|e| error::to_command_error(e)),
            RedisConnection::Cluster(arc) => {
                let mut conn = arc.lock().await;
                pipe.query_async(&mut *conn)
                    .await
                    .map_err(|e| error::to_command_error(e))
            }
        }
    }

    pub async fn query_on_node<T: FromRedisValue>(
        &mut self,
        host: &str,
        port: u16,
        cmd: Cmd,
    ) -> error::RedisResult<T> {
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
            .map_err(|e| error::to_command_error(e))?;
        from_redis_value(&value).map_err(|e| error::to_command_error(e))
    }
}

fn parse_database(database: Option<&str>) -> error::RedisResult<i64> {
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
        .map_err(|_| error::validation("Redis database must be a numeric index"))?;
    if !(0..=255).contains(&db) {
        return Err(error::validation(
            "Redis database must be between 0 and 255",
        ));
    }
    Ok(db)
}

fn selected_database(form: &ConnectionForm, database: Option<&str>) -> error::RedisResult<i64> {
    match database {
        Some(db) => parse_database(Some(db)),
        None => parse_database(form.database.as_deref()),
    }
}

fn redis_mode(form: &ConnectionForm) -> &str {
    match form.mode.as_deref() {
        Some("standalone") => "standalone",
        Some("cluster") => "cluster",
        Some("sentinel") => "sentinel",
        _ if form
            .host
            .as_deref()
            .map(|host| {
                host.split(',')
                    .filter(|part| !part.trim().is_empty())
                    .count()
                    > 1
            })
            .unwrap_or(false) =>
        {
            "cluster"
        }
        _ => "standalone",
    }
}

fn is_cluster_form(form: &ConnectionForm) -> bool {
    redis_mode(form) == "cluster"
}

fn is_sentinel_form(form: &ConnectionForm) -> bool {
    redis_mode(form) == "sentinel"
}

fn connect_timeout(form: &ConnectionForm) -> Duration {
    Duration::from_millis(
        form.connect_timeout_ms
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS) as u64,
    )
}

fn parse_host_port(raw: &str, fallback_port: i64) -> error::RedisResult<(String, i64)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(error::validation("Redis host is required"));
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
                .map_err(|_| error::validation("Redis port is invalid"))?;
            return Ok((host.to_string(), port));
        }
    }
    Ok((trimmed.to_string(), fallback_port))
}

fn build_connection_info_for_host(
    form: &ConnectionForm,
    host: &str,
    db: i64,
) -> error::RedisResult<ConnectionInfo> {
    let (host, port) = parse_host_port(host, form.port.unwrap_or(DEFAULT_REDIS_PORT))?;
    if !(1..=65535).contains(&port) {
        return Err(error::validation("Redis port must be between 1 and 65535"));
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

fn build_connection_info(form: &ConnectionForm, db: i64) -> error::RedisResult<ConnectionInfo> {
    let host = if let Some(seed_nodes) = form.seed_nodes.as_ref() {
        seed_nodes
            .first()
            .cloned()
            .or_else(|| form.host.clone())
            .ok_or_else(|| error::validation("Redis host is required"))?
    } else {
        form.host
            .clone()
            .ok_or_else(|| error::validation("Redis host is required"))?
    };
    build_connection_info_for_host(form, &host, db)
}

fn build_cluster_nodes(form: &ConnectionForm) -> error::RedisResult<Vec<ConnectionInfo>> {
    let db = selected_database(form, None)?;
    if db != 0 {
        return Err(error::validation("Redis Cluster only supports database 0"));
    }
    let nodes: Vec<ConnectionInfo> = form
        .seed_nodes
        .clone()
        .or_else(|| {
            form.host.as_deref().map(|host| {
                host.split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default()
        .into_iter()
        .map(|part| build_connection_info_for_host(form, &part, 0))
        .collect::<Result<_, _>>()?;
    if nodes.len() < 2 {
        return Err(error::validation(
            "Redis Cluster requires at least two seed nodes",
        ));
    }
    Ok(nodes)
}

fn build_sentinel_node_info(form: &ConnectionForm, db: i64) -> SentinelNodeConnectionInfo {
    let tls_mode = if form.ssl.unwrap_or(false) {
        Some(TlsMode::Secure)
    } else {
        None
    };
    let username = form
        .username
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let password = form
        .password
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    SentinelNodeConnectionInfo {
        tls_mode,
        redis_connection_info: Some(RedisConnectionInfo {
            db,
            username,
            password,
            protocol: ProtocolVersion::RESP2,
        }),
    }
}

pub async fn connect(
    form: &ConnectionForm,
    database: Option<&str>,
) -> error::RedisResult<RedisConnection> {
    crate::network::validate_proxy(form)?;
    if crate::network::proxy_enabled(form) && (is_sentinel_form(form) || is_cluster_form(form)) {
        return Err(error::unsupported(
            "Network proxy currently supports standalone Redis only",
        ));
    }
    if is_sentinel_form(form) {
        let sentinel_nodes = form
            .sentinels
            .clone()
            .ok_or_else(|| error::validation("Sentinel nodes required"))?;
        let service_name = form
            .service_name
            .clone()
            .unwrap_or_else(|| "mymaster".to_string());
        let db = selected_database(form, database)?;
        let node_info = build_sentinel_node_info(form, db);

        // Build sentinel node URLs with optional sentinel password
        let sentinel_password = form.sentinel_password.as_deref().filter(|v| !v.is_empty());
        let sentinel_urls: Vec<String> = sentinel_nodes
            .iter()
            .map(|node| {
                if let Some(password) = sentinel_password {
                    format!("redis://:{}@{}", password, node)
                } else {
                    format!("redis://{}", node)
                }
            })
            .collect();

        let mut sentinel = Sentinel::build(sentinel_urls).map_err(|e| conn_failed_error(&e))?;

        let client = sentinel
            .async_master_for(&service_name, Some(&node_info))
            .await
            .map_err(|e| conn_failed_error(&e))?;

        let config = AsyncConnectionConfig::new().set_connection_timeout(connect_timeout(form));
        let conn = client
            .get_multiplexed_async_connection_with_config(&config)
            .await
            .map_err(|e| conn_failed_error(&e))?;
        return Ok(RedisConnection::Standalone(conn, None));
    }

    if is_cluster_form(form) {
        if let Some(db) = database {
            if parse_database(Some(db))? != 0 {
                return Err(error::validation("Redis Cluster only supports database 0"));
            }
        }
        let nodes = build_cluster_nodes(form)?;
        let client = ClusterClient::builder(nodes)
            .connection_timeout(connect_timeout(form))
            .build()
            .map_err(|e| conn_failed_error(&e))?;
        let conn = client
            .get_async_connection()
            .await
            .map_err(|e| conn_failed_error(&e))?;
        return Ok(RedisConnection::Cluster(Arc::new(TokioMutex::new(conn))));
    }

    let mut effective_form = form.clone();
    let tunnel = if crate::network::proxy_enabled(form) {
        let tunnel = crate::network::start_network_tunnel(form).await?;
        effective_form.seed_nodes = None;
        effective_form.host = Some("127.0.0.1".to_string());
        effective_form.port = Some(tunnel.local_port as i64);
        Some(tunnel)
    } else {
        None
    };
    let db = selected_database(form, database)?;
    let info = build_connection_info(&effective_form, db)?;
    let client = redis::Client::open(info).map_err(|e| conn_failed_error(&e))?;
    let config = AsyncConnectionConfig::new().set_connection_timeout(connect_timeout(form));
    let conn = client
        .get_multiplexed_async_connection_with_config(&config)
        .await
        .map_err(|e| conn_failed_error(&e))?;
    Ok(RedisConnection::Standalone(conn, tunnel))
}

async fn query_on<T: FromRedisValue, C: ConnectionLike + Send + Sync>(
    conn: &mut C,
    cmd: Cmd,
) -> error::RedisResult<T> {
    cmd.query_async::<T>(conn)
        .await
        .map_err(|e| error::to_command_error(e))
}

pub async fn ping(conn: &mut RedisConnection) -> error::RedisResult<()> {
    conn.query::<String>(redis::cmd("PING"))
        .await
        .map(|_| ())
        .map_err(|e| conn_failed_error(&e))
}

pub fn list_databases(
    form: &ConnectionForm,
    db_count: i64,
) -> error::RedisResult<Vec<RedisDatabaseInfo>> {
    if is_cluster_form(form) {
        build_cluster_nodes(form)?;
        return Ok(vec![RedisDatabaseInfo {
            index: 0,
            name: "db0".to_string(),
            selected: true,
            key_count: None,
        }]);
    }

    // Sentinel resolves to a master node, so database selection works like standalone.
    let selected = selected_database(form, None)?;
    let db_count = db_count.clamp(1, 256);
    Ok((0..db_count)
        .map(|index| RedisDatabaseInfo {
            index,
            name: format!("db{index}"),
            selected: index == selected,
            key_count: None,
        })
        .collect())
}

pub async fn server_info(conn: &mut RedisConnection) -> error::RedisResult<RedisServerInfo> {
    let info_str: String = conn
        .query(redis::cmd("INFO"))
        .await
        .map_err(|e| error::to_command_error(e))?;

    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for line in info_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            if let Some(name) = line.strip_prefix("# ") {
                current_section = name.to_string();
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            sections
                .entry(current_section.clone())
                .or_default()
                .insert(key.to_string(), value.to_string());
        }
    }

    let dbsize: u64 = conn
        .query(redis::cmd("DBSIZE"))
        .await
        .map_err(|e| error::to_command_error(e))?;

    Ok(RedisServerInfo { sections, dbsize })
}

pub async fn server_config(
    conn: &mut RedisConnection,
) -> error::RedisResult<HashMap<String, String>> {
    let mut cmd = redis::cmd("CONFIG");
    cmd.arg("GET").arg("*");
    let values: Vec<String> = conn
        .query(cmd)
        .await
        .map_err(|e| error::to_command_error(e))?;

    let mut config = HashMap::new();
    let mut iter = values.into_iter();
    while let Some(key) = iter.next() {
        if let Some(value) = iter.next() {
            config.insert(key, value);
        }
    }
    Ok(config)
}

pub async fn slowlog_get(
    conn: &mut RedisConnection,
    count: i64,
) -> error::RedisResult<Vec<RedisSlowlogEntry>> {
    let mut cmd = redis::cmd("SLOWLOG");
    cmd.arg("GET").arg(count.max(1));
    let raw: Vec<Vec<Value>> = conn
        .query(cmd)
        .await
        .map_err(|e| error::to_command_error(e))?;

    let mut entries = Vec::new();
    for item in raw {
        if item.len() < 4 {
            continue;
        }
        let id = match &item[0] {
            Value::Int(v) => *v as u64,
            _ => continue,
        };
        let timestamp = match &item[1] {
            Value::Int(v) => *v,
            _ => continue,
        };
        let duration_ms = match &item[2] {
            Value::Int(v) => *v as u64,
            _ => continue,
        };
        let command = match &item[3] {
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    Value::BulkString(b) => String::from_utf8(b.clone()).ok(),
                    Value::SimpleString(s) => Some(s.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => format!("{:?}", item[3]),
        };
        entries.push(RedisSlowlogEntry {
            id,
            timestamp,
            duration_ms,
            command,
        });
    }
    Ok(entries)
}
