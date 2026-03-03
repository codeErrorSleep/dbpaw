pub mod ai;
pub mod config;
pub mod connection;
pub mod metadata;
pub mod query;
pub mod storage;
pub mod transfer;

use crate::db::drivers::DatabaseDriver;
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

fn connection_pool_key(id: i64, database: &Option<String>) -> String {
    if let Some(db) = database {
        if !db.is_empty() {
            return format!("{}:{}", id, db);
        }
    }
    id.to_string()
}

pub async fn ensure_connection(
    state: &State<'_, AppState>,
    id: i64,
) -> Result<Arc<dyn DatabaseDriver>, String> {
    ensure_connection_with_db(state, id, None).await
}

pub async fn ensure_connection_with_db(
    state: &State<'_, AppState>,
    id: i64,
    database: Option<String>,
) -> Result<Arc<dyn DatabaseDriver>, String> {
    let key = connection_pool_key(id, &database);

    if let Some(driver) = state.pool_manager.get_connection(&key).await {
        // Harden: Check if connection still exists in LocalDb
        let local_db = {
            let lock = state.local_db.lock().await;
            lock.clone()
        };

        if let Some(db) = local_db {
            if db.get_connection_by_id(id).await.is_err() {
                state.pool_manager.remove_by_prefix(&id.to_string()).await;
                return Err(format!("Connection with ID {} no longer exists", id));
            }
        }
        return Ok(driver);
    }

    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };

    let db = local_db.ok_or("Local DB not initialized")?;
    let mut form = db.get_connection_form_by_id(id).await?;

    if let Some(db_name) = database {
        if !db_name.is_empty() {
            form.database = Some(db_name);
        }
    }

    state.pool_manager.connect(&key, &form).await
}

async fn execute_with_retry_core<T, Ensure, EnsureFut, Remove, RemoveFut, Task, TaskFut>(
    mut ensure: Ensure,
    mut remove: Remove,
    task: Task,
) -> Result<T, String>
where
    Ensure: FnMut() -> EnsureFut,
    EnsureFut: std::future::Future<Output = Result<Arc<dyn DatabaseDriver>, String>>,
    Remove: FnMut() -> RemoveFut,
    RemoveFut: std::future::Future<Output = ()>,
    Task: Fn(Arc<dyn DatabaseDriver>) -> TaskFut,
    TaskFut: std::future::Future<Output = Result<T, String>>,
{
    let driver = ensure().await?;
    match task(driver.clone()).await {
        Ok(res) => Ok(res),
        Err(e) => {
            if is_connection_error(&e) {
                println!("[Pool] Connection error detected, retrying...");
                remove().await;
                let driver = ensure().await?;
                task(driver).await.map_err(|e| {
                    println!("[Pool] Retry failed: {}", e);
                    e
                })
            } else {
                println!("[Pool] Operation failed: {}", e);
                Err(e)
            }
        }
    }
}

pub async fn execute_with_retry<F, Fut, T>(
    state: &State<'_, AppState>,
    id: i64,
    database: Option<String>,
    task: F,
) -> Result<T, String>
where
    F: Fn(Arc<dyn DatabaseDriver>) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let key = connection_pool_key(id, &database);
    execute_with_retry_core(
        || ensure_connection_with_db(state, id, database.clone()),
        || state.pool_manager.remove(&key),
        task,
    )
    .await
}

fn is_connection_error(e: &str) -> bool {
    let lower = e.to_lowercase();
    lower.contains("pool closed")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("timeout")
        || lower.contains("network unreachable")
        || lower.contains("closed")
        || lower.contains("eof")
}

#[cfg(test)]
mod tests {
    use super::execute_with_retry_core;
    use crate::db::drivers::DatabaseDriver;
    use crate::models::{
        QueryResult, SchemaOverview, TableDataResponse, TableInfo, TableMetadata, TableStructure,
    };
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockDriver;

    #[async_trait]
    impl DatabaseDriver for MockDriver {
        async fn close(&self) {}
        async fn test_connection(&self) -> Result<(), String> {
            Ok(())
        }
        async fn list_databases(&self) -> Result<Vec<String>, String> {
            Ok(vec![])
        }
        async fn list_tables(&self, _schema: Option<String>) -> Result<Vec<TableInfo>, String> {
            Ok(vec![])
        }
        async fn get_table_structure(
            &self,
            _schema: String,
            _table: String,
        ) -> Result<TableStructure, String> {
            Err("Unimplemented".into())
        }
        async fn get_table_metadata(
            &self,
            _schema: String,
            _table: String,
        ) -> Result<TableMetadata, String> {
            Err("Unimplemented".into())
        }
        async fn get_table_ddl(&self, _schema: String, _table: String) -> Result<String, String> {
            Err("Unimplemented".into())
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
        ) -> Result<TableDataResponse, String> {
            Err("Unimplemented".into())
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
        ) -> Result<TableDataResponse, String> {
            Err("Unimplemented".into())
        }
        async fn execute_query(&self, _sql: String) -> Result<QueryResult, String> {
            Err("Unimplemented".into())
        }
        async fn get_schema_overview(
            &self,
            _schema: Option<String>,
        ) -> Result<SchemaOverview, String> {
            Err("Unimplemented".into())
        }
    }

    #[tokio::test]
    async fn execute_with_retry_retries_once_on_connection_error_and_succeeds() {
        let ensure_calls = Arc::new(AtomicUsize::new(0));
        let remove_calls = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::new(AtomicUsize::new(0));
        let driver: Arc<dyn DatabaseDriver> = Arc::new(MockDriver);

        let ensure_calls_c = ensure_calls.clone();
        let ensure_driver = driver.clone();
        let remove_calls_c = remove_calls.clone();
        let task_calls_c = task_calls.clone();

        let result: Result<String, String> = execute_with_retry_core(
            move || {
                let ensure_calls_c = ensure_calls_c.clone();
                let ensure_driver = ensure_driver.clone();
                async move {
                    ensure_calls_c.fetch_add(1, Ordering::SeqCst);
                    Ok(ensure_driver)
                }
            },
            move || {
                let remove_calls_c = remove_calls_c.clone();
                async move {
                    remove_calls_c.fetch_add(1, Ordering::SeqCst);
                }
            },
            move |_driver| {
                let task_calls_c = task_calls_c.clone();
                async move {
                    let n = task_calls_c.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        Err("[QUERY_ERROR] connection reset by peer".to_string())
                    } else {
                        Ok("ok".to_string())
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(task_calls.load(Ordering::SeqCst), 2);
        assert_eq!(ensure_calls.load(Ordering::SeqCst), 2);
        assert_eq!(remove_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn execute_with_retry_returns_retry_error_when_second_attempt_fails() {
        let ensure_calls = Arc::new(AtomicUsize::new(0));
        let remove_calls = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::new(AtomicUsize::new(0));
        let driver: Arc<dyn DatabaseDriver> = Arc::new(MockDriver);

        let ensure_calls_c = ensure_calls.clone();
        let ensure_driver = driver.clone();
        let remove_calls_c = remove_calls.clone();
        let task_calls_c = task_calls.clone();

        let result: Result<String, String> = execute_with_retry_core(
            move || {
                let ensure_calls_c = ensure_calls_c.clone();
                let ensure_driver = ensure_driver.clone();
                async move {
                    ensure_calls_c.fetch_add(1, Ordering::SeqCst);
                    Ok(ensure_driver)
                }
            },
            move || {
                let remove_calls_c = remove_calls_c.clone();
                async move {
                    remove_calls_c.fetch_add(1, Ordering::SeqCst);
                }
            },
            move |_driver| {
                let task_calls_c = task_calls_c.clone();
                async move {
                    task_calls_c.fetch_add(1, Ordering::SeqCst);
                    Err("[QUERY_ERROR] pool closed".to_string())
                }
            },
        )
        .await;

        assert_eq!(result.unwrap_err(), "[QUERY_ERROR] pool closed");
        assert_eq!(task_calls.load(Ordering::SeqCst), 2);
        assert_eq!(ensure_calls.load(Ordering::SeqCst), 2);
        assert_eq!(remove_calls.load(Ordering::SeqCst), 1);
    }
}
