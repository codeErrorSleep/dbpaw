use crate::db::drivers::{self, DatabaseDriver};
use crate::error::AppError;
use crate::models::ConnectionForm;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, RwLock};

/// Configuration for the connection pool
#[derive(Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    pub max_connections: usize,
    /// Timeout in seconds for idle connections
    pub idle_timeout_secs: u64,
    /// Timeout in seconds for establishing new connections
    pub connection_timeout_secs: u64,
    /// Interval in seconds between health checks
    pub health_check_interval_secs: u64,
    /// Maximum number of retry attempts for failed connections
    pub max_retries: u32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            idle_timeout_secs: 300,
            connection_timeout_secs: 30,
            health_check_interval_secs: 60,
            max_retries: 3,
        }
    }
}

pub struct PoolEntry {
    pub driver: Arc<dyn DatabaseDriver>,
    // Use std::sync::Mutex for interior mutability to update timestamp on read access
    pub last_used: Mutex<std::time::Instant>,
    pub created_at: std::time::Instant,
    pub is_healthy: AtomicBool,
    pub use_count: AtomicU64,
}

pub struct PoolManager {
    // Store active connections, Key is connection UUID
    pools: Arc<RwLock<HashMap<String, PoolEntry>>>,
    // Connection lock to prevent concurrent connection attempts for the same UUID
    connect_locks: RwLock<HashMap<String, Arc<AsyncMutex<()>>>>,
    // Pool configuration
    config: PoolConfig,
}

impl PoolManager {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            connect_locks: RwLock::new(HashMap::new()),
            config,
        }
    }

    pub fn config(&self) -> &PoolConfig {
        &self.config
    }

    /// Get existing connection, update last_used time if it exists
    pub async fn get_connection(&self, id: &str) -> Option<Arc<dyn DatabaseDriver>> {
        let pools = self.pools.read().await;
        if let Some(entry) = pools.get(id) {
            match entry.last_used.lock() {
                Ok(mut last) => *last = std::time::Instant::now(),
                Err(e) => tracing::warn!(error = %e, "Failed to lock last_used timestamp"),
            }
            entry
                .use_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Some(entry.driver.clone());
        }
        None
    }

    /// Establish new connection and cache it. Return existing one if already present.
    pub async fn connect(
        &self,
        id: &str,
        form: &ConnectionForm,
    ) -> Result<Arc<dyn DatabaseDriver>, AppError> {
        // 1. Fast path check
        if let Some(driver) = self.get_connection(id).await {
            return Ok(driver);
        }

        // 2. Check connection limit
        {
            let pools = self.pools.read().await;
            if pools.len() >= self.config.max_connections {
                return Err(AppError::conn_failed(
                    format!(
                        "Maximum connection limit reached ({})",
                        self.config.max_connections
                    ),
                    "Close unused connections and try again",
                ));
            }
        }

        // 3. Get lock for this specific ID
        let lock = {
            let mut locks = self.connect_locks.write().await;
            locks
                .entry(id.to_string())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        // Lock to ensure only one thread establishes connection for this ID at a time
        let _guard = lock.lock().await;

        // 4. Double check
        if let Some(driver) = self.get_connection(id).await {
            return Ok(driver);
        }

        // 5. Create new connection with timeout
        let driver_box = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.connection_timeout_secs),
            drivers::connect(form),
        )
        .await
        .map_err(|_| {
            AppError::conn_timeout(format!(
                "Connection timeout after {} seconds",
                self.config.connection_timeout_secs
            ))
        })??;

        let driver: Arc<dyn DatabaseDriver> = Arc::from(driver_box);

        // 6. Store in pool
        {
            let mut pools = self.pools.write().await;
            pools.insert(
                id.to_string(),
                PoolEntry {
                    driver: driver.clone(),
                    last_used: Mutex::new(std::time::Instant::now()),
                    created_at: std::time::Instant::now(),
                    is_healthy: AtomicBool::new(true),
                    use_count: AtomicU64::new(0),
                },
            );
        }

        Ok(driver)
    }

    /// Remove and close connection
    pub async fn remove(&self, id: &str) {
        let entry = {
            let mut pools = self.pools.write().await;
            pools.remove(id)
        };

        if let Some(entry) = entry {
            // Explicitly close connection, write lock is released at this point
            entry.driver.close().await;
        }
    }

    /// Remove and close all connections for a given ID (including different databases)
    pub async fn remove_by_prefix(&self, id: &str) {
        let entries_to_remove = {
            let mut pools = self.pools.write().await;
            let keys_to_remove: Vec<String> = pools
                .keys()
                .filter(|k| k == &id || k.starts_with(&format!("{}:", id)))
                .cloned()
                .collect();

            let mut entries = Vec::new();
            for key in keys_to_remove {
                if let Some(entry) = pools.remove(&key) {
                    entries.push(entry);
                }
            }
            entries
        };

        for entry in entries_to_remove {
            entry.driver.close().await;
        }
    }

    /// Close all connections (used when application exits)
    pub async fn close_all(&self) {
        let entries = {
            let mut pools = self.pools.write().await;
            pools.drain().map(|(_, e)| e).collect::<Vec<_>>()
        };

        for entry in entries {
            entry.driver.close().await;
        }
    }

    /// Start background cleanup task that periodically removes idle and unhealthy connections
    pub async fn start_cleanup_task(self: &Arc<Self>) {
        let manager = Arc::clone(self);
        let config = manager.config.clone();
        let pools = Arc::clone(&manager.pools);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                config.health_check_interval_secs,
            ));

            loop {
                interval.tick().await;
                manager.check_all_connections_health().await;
                Self::cleanup_idle_connections(&pools, &config).await;
                Self::cleanup_unhealthy_connections(&pools).await;
            }
        });
    }

    async fn cleanup_idle_connections(
        pools: &Arc<RwLock<HashMap<String, PoolEntry>>>,
        config: &PoolConfig,
    ) {
        let now = std::time::Instant::now();
        let idle_timeout = std::time::Duration::from_secs(config.idle_timeout_secs);

        let mut to_remove = Vec::new();
        {
            let pools_read = pools.read().await;
            for (key, entry) in pools_read.iter() {
                if let Ok(last) = entry.last_used.lock() {
                    if now.duration_since(*last) > idle_timeout {
                        to_remove.push(key.clone());
                    }
                }
            }
        }

        if !to_remove.is_empty() {
            let entries: Vec<PoolEntry> = {
                let mut pools_write = pools.write().await;
                to_remove
                    .iter()
                    .filter_map(|key| {
                        let entry = pools_write.remove(key)?;
                        tracing::info!(connection_id = %key, "Removing idle connection");
                        Some(entry)
                    })
                    .collect()
            };
            for entry in entries {
                entry.driver.close().await;
            }
        }
    }

    async fn cleanup_unhealthy_connections(pools: &Arc<RwLock<HashMap<String, PoolEntry>>>) {
        let mut to_remove = Vec::new();
        {
            let pools_read = pools.read().await;
            for (key, entry) in pools_read.iter() {
                if !entry.is_healthy.load(std::sync::atomic::Ordering::Relaxed) {
                    to_remove.push(key.clone());
                }
            }
        }

        if !to_remove.is_empty() {
            let entries: Vec<PoolEntry> = {
                let mut pools_write = pools.write().await;
                to_remove
                    .iter()
                    .filter_map(|key| {
                        let entry = pools_write.remove(key)?;
                        tracing::warn!(connection_id = %key, "Removing unhealthy connection");
                        Some(entry)
                    })
                    .collect()
            };
            for entry in entries {
                entry.driver.close().await;
            }
        }
    }

    /// Check health of a single connection by executing test_connection
    async fn check_health(&self, key: &str) -> bool {
        let driver = {
            let pools = self.pools.read().await;
            match pools.get(key) {
                Some(entry) => entry.driver.clone(),
                None => return false,
            }
        };

        match driver.test_connection().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Check health of all connections and update their health status
    async fn check_all_connections_health(&self) {
        let keys: Vec<String> = {
            let pools = self.pools.read().await;
            pools.keys().cloned().collect()
        };

        for key in keys {
            let is_healthy = self.check_health(&key).await;
            let pools = self.pools.read().await;
            if let Some(entry) = pools.get(&key) {
                entry
                    .is_healthy
                    .store(is_healthy, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    #[cfg(test)]
    pub async fn insert_mock_connection(&self, id: &str, driver: Arc<dyn DatabaseDriver>) {
        let mut pools = self.pools.write().await;
        pools.insert(
            id.to_string(),
            PoolEntry {
                driver,
                last_used: Mutex::new(std::time::Instant::now()),
                created_at: std::time::Instant::now(),
                is_healthy: AtomicBool::new(true),
                use_count: AtomicU64::new(0),
            },
        );
    }

    #[cfg(test)]
    pub async fn count(&self) -> usize {
        self.pools.read().await.len()
    }

    #[cfg(test)]
    pub async fn contains_key(&self, key: &str) -> bool {
        self.pools.read().await.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::drivers::{DatabaseDriver, DriverResult};
    use async_trait::async_trait;

    struct MockDriver;

    #[async_trait]
    impl DatabaseDriver for MockDriver {
        async fn close(&self) {}
        async fn test_connection(&self) -> DriverResult<()> {
            Ok(())
        }
        async fn list_databases(&self) -> DriverResult<Vec<String>> {
            Ok(vec![])
        }
        async fn list_tables(
            &self,
            _schema: Option<String>,
        ) -> DriverResult<Vec<crate::models::TableInfo>> {
            Ok(vec![])
        }
        async fn get_table_structure(
            &self,
            _schema: String,
            _table: String,
        ) -> DriverResult<crate::models::TableStructure> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_metadata(
            &self,
            _schema: String,
            _table: String,
        ) -> DriverResult<crate::models::TableMetadata> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_ddl(&self, _schema: String, _table: String) -> DriverResult<String> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_data(
            &self,
            _schema: String,
            _table: String,
            _page: i64,
            _limit: i64,
            _sort_column: Option<String>,
            _sort_direction: Option<String>,
            _filter: Option<String>,
            _order_by: Option<String>,
            _include_total: bool,
        ) -> DriverResult<crate::models::TableDataResponse> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_data_chunk(
            &self,
            _schema: String,
            _table: String,
            _page: i64,
            _limit: i64,
            _sort_column: Option<String>,
            _sort_direction: Option<String>,
            _filter: Option<String>,
            _order_by: Option<String>,
        ) -> DriverResult<crate::models::TableDataResponse> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn execute_query(&self, _sql: String) -> DriverResult<crate::models::QueryResult> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_schema_overview(
            &self,
            _schema: Option<String>,
        ) -> DriverResult<crate::models::SchemaOverview> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
    }

    #[tokio::test]
    async fn test_remove_by_prefix() {
        let manager = PoolManager::new(PoolConfig::default());
        let driver = Arc::new(MockDriver);

        manager.insert_mock_connection("1", driver.clone()).await;
        manager
            .insert_mock_connection("1:db1", driver.clone())
            .await;
        manager
            .insert_mock_connection("1:db2", driver.clone())
            .await;
        manager.insert_mock_connection("2", driver.clone()).await;
        manager.insert_mock_connection("21", driver.clone()).await; // Should NOT be removed

        assert_eq!(manager.count().await, 5);

        manager.remove_by_prefix("1").await;

        assert_eq!(manager.count().await, 2);
        assert!(!manager.contains_key("1").await);
        assert!(!manager.contains_key("1:db1").await);
        assert!(!manager.contains_key("1:db2").await);
        assert!(manager.contains_key("2").await);
        assert!(manager.contains_key("21").await);
    }

    #[tokio::test]
    async fn test_cleanup_idle_connections() {
        let config = PoolConfig {
            idle_timeout_secs: 1,
            ..Default::default()
        };
        let manager = PoolManager::new(config);
        let driver = Arc::new(MockDriver);

        manager
            .insert_mock_connection("idle1", driver.clone())
            .await;
        manager
            .insert_mock_connection("idle2", driver.clone())
            .await;
        assert_eq!(manager.count().await, 2);

        // Wait for connections to become idle
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        PoolManager::cleanup_idle_connections(&manager.pools, &manager.config).await;

        assert_eq!(manager.count().await, 0);
        assert!(!manager.contains_key("idle1").await);
        assert!(!manager.contains_key("idle2").await);
    }

    #[tokio::test]
    async fn test_cleanup_idle_connections_keeps_recent() {
        let config = PoolConfig {
            idle_timeout_secs: 300,
            ..Default::default()
        };
        let manager = PoolManager::new(config);
        let driver = Arc::new(MockDriver);

        manager
            .insert_mock_connection("recent1", driver.clone())
            .await;
        assert_eq!(manager.count().await, 1);

        PoolManager::cleanup_idle_connections(&manager.pools, &manager.config).await;

        // Should not be removed since it was just created
        assert_eq!(manager.count().await, 1);
        assert!(manager.contains_key("recent1").await);
    }

    #[tokio::test]
    async fn test_cleanup_unhealthy_connections() {
        let config = PoolConfig::default();
        let manager = PoolManager::new(config);
        let driver = Arc::new(MockDriver);

        manager
            .insert_mock_connection("healthy", driver.clone())
            .await;
        manager
            .insert_mock_connection("unhealthy", driver.clone())
            .await;
        assert_eq!(manager.count().await, 2);

        // Mark one as unhealthy
        {
            let pools = manager.pools.read().await;
            if let Some(entry) = pools.get("unhealthy") {
                entry
                    .is_healthy
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }

        PoolManager::cleanup_unhealthy_connections(&manager.pools).await;

        assert_eq!(manager.count().await, 1);
        assert!(manager.contains_key("healthy").await);
        assert!(!manager.contains_key("unhealthy").await);
    }

    #[tokio::test]
    async fn test_start_cleanup_task() {
        let config = PoolConfig {
            health_check_interval_secs: 1,
            idle_timeout_secs: 1,
            ..Default::default()
        };
        let manager = Arc::new(PoolManager::new(config));
        let driver = Arc::new(MockDriver);

        manager
            .insert_mock_connection("task_idle", driver.clone())
            .await;
        assert_eq!(manager.count().await, 1);

        manager.start_cleanup_task().await;

        // Wait for the cleanup task to run
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        assert_eq!(manager.count().await, 0);
    }

    struct UnhealthyMockDriver;

    #[async_trait]
    impl DatabaseDriver for UnhealthyMockDriver {
        async fn close(&self) {}
        async fn test_connection(&self) -> DriverResult<()> {
            Err(crate::error::AppError::conn_failed(
                "Connection failed",
                "Check connection settings",
            ))
        }
        async fn list_databases(&self) -> DriverResult<Vec<String>> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn list_tables(
            &self,
            _schema: Option<String>,
        ) -> DriverResult<Vec<crate::models::TableInfo>> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_structure(
            &self,
            _schema: String,
            _table: String,
        ) -> DriverResult<crate::models::TableStructure> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_metadata(
            &self,
            _schema: String,
            _table: String,
        ) -> DriverResult<crate::models::TableMetadata> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_ddl(&self, _schema: String, _table: String) -> DriverResult<String> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_data(
            &self,
            _schema: String,
            _table: String,
            _page: i64,
            _limit: i64,
            _sort_column: Option<String>,
            _sort_direction: Option<String>,
            _filter: Option<String>,
            _order_by: Option<String>,
            _include_total: bool,
        ) -> DriverResult<crate::models::TableDataResponse> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_data_chunk(
            &self,
            _schema: String,
            _table: String,
            _page: i64,
            _limit: i64,
            _sort_column: Option<String>,
            _sort_direction: Option<String>,
            _filter: Option<String>,
            _order_by: Option<String>,
        ) -> DriverResult<crate::models::TableDataResponse> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn execute_query(&self, _sql: String) -> DriverResult<crate::models::QueryResult> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_schema_overview(
            &self,
            _schema: Option<String>,
        ) -> DriverResult<crate::models::SchemaOverview> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
    }

    #[tokio::test]
    async fn test_check_health_healthy() {
        let config = PoolConfig::default();
        let manager = PoolManager::new(config);
        let driver = Arc::new(MockDriver);

        manager
            .insert_mock_connection("healthy", driver.clone())
            .await;

        assert!(manager.check_health("healthy").await);
    }

    #[tokio::test]
    async fn test_check_health_unhealthy() {
        let config = PoolConfig::default();
        let manager = PoolManager::new(config);
        let driver = Arc::new(UnhealthyMockDriver);

        manager
            .insert_mock_connection("unhealthy", driver.clone())
            .await;

        assert!(!manager.check_health("unhealthy").await);
    }

    #[tokio::test]
    async fn test_check_health_nonexistent() {
        let config = PoolConfig::default();
        let manager = PoolManager::new(config);

        assert!(!manager.check_health("nonexistent").await);
    }

    #[tokio::test]
    async fn test_check_all_connections_health() {
        let config = PoolConfig::default();
        let manager = PoolManager::new(config);
        let healthy_driver = Arc::new(MockDriver);
        let unhealthy_driver = Arc::new(UnhealthyMockDriver);

        manager
            .insert_mock_connection("healthy", healthy_driver.clone())
            .await;
        manager
            .insert_mock_connection("unhealthy", unhealthy_driver.clone())
            .await;

        manager.check_all_connections_health().await;

        let pools = manager.pools.read().await;
        assert!(
            pools
                .get("healthy")
                .unwrap()
                .is_healthy
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        assert!(
            !pools
                .get("unhealthy")
                .unwrap()
                .is_healthy
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }

    #[tokio::test]
    async fn test_connection_limit_enforced() {
        let config = PoolConfig {
            max_connections: 2,
            ..Default::default()
        };
        let manager = PoolManager::new(config);
        let driver = Arc::new(MockDriver);

        // Insert two connections (at limit)
        manager
            .insert_mock_connection("conn1", driver.clone())
            .await;
        manager
            .insert_mock_connection("conn2", driver.clone())
            .await;
        assert_eq!(manager.count().await, 2);

        // Try to add a third connection via connect (should fail)
        let form = ConnectionForm {
            driver: "sqlite".to_string(),
            name: Some("test".to_string()),
            host: None,
            port: None,
            database: None,
            schema: None,
            username: None,
            password: None,
            ssl: None,
            ssl_mode: None,
            ssl_ca_cert: None,
            file_path: None,
            ssh_enabled: None,
            ssh_host: None,
            ssh_port: None,
            ssh_username: None,
            ssh_password: None,
            ssh_key_path: None,
            mode: None,
            seed_nodes: None,
            sentinels: None,
            connect_timeout_ms: None,
            service_name: None,
            sentinel_password: None,
            auth_mode: None,
            api_key_id: None,
            api_key_secret: None,
            api_key_encoded: None,
            cloud_id: None,
            auth_source: None,
            proxy: None,
        };

        let result = manager.connect("conn3", &form).await;
        match result {
            Ok(_) => panic!("Expected error but got Ok"),
            Err(e) => assert!(e.to_string().contains("Maximum connection limit reached")),
        }
    }

    #[tokio::test]
    async fn test_connection_limit_not_enforced_when_below_limit() {
        let config = PoolConfig {
            max_connections: 5,
            ..Default::default()
        };
        let manager = PoolManager::new(config);
        let driver = Arc::new(MockDriver);

        // Insert one connection (below limit)
        manager
            .insert_mock_connection("conn1", driver.clone())
            .await;
        assert_eq!(manager.count().await, 1);

        // Get existing connection should work
        let result = manager.get_connection("conn1").await;
        assert!(result.is_some());
    }
}
