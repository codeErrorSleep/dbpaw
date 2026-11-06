pub mod ai;
pub mod config;
pub mod connection;
pub mod elasticsearch;
pub mod mcp;
pub mod metadata;
pub mod mongodb;
pub mod query;
pub mod redis;
pub mod storage;
pub mod system;
pub mod transfer;

use crate::db::drivers::DatabaseDriver;
use crate::error::AppError;
use crate::models::ConnectionForm;
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

pub async fn get_connection_form_by_id(
    state: &State<'_, AppState>,
    id: i64,
) -> Result<ConnectionForm, AppError> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    let db = local_db.ok_or_else(|| AppError::internal("Local DB not initialized"))?;
    db.get_connection_form_by_id(id).await
}

pub async fn get_connection_form_by_id_with_driver_check(
    state: &State<'_, AppState>,
    id: i64,
    expected_driver: &str,
) -> Result<ConnectionForm, AppError> {
    let form = get_connection_form_by_id(state, id).await?;
    if form.driver != expected_driver {
        return Err(AppError::unsupported(format!(
            "Connection {} is not a {} connection",
            id, expected_driver
        )));
    }
    Ok(form)
}

pub async fn ensure_connection_with_db(
    state: &State<'_, AppState>,
    id: i64,
    database: Option<String>,
) -> Result<Arc<dyn DatabaseDriver>, AppError> {
    ensure_connection_with_db_inner(state.inner(), id, database).await
}

pub async fn ensure_connection_with_db_from_app_state(
    state: &AppState,
    id: i64,
    database: Option<String>,
) -> Result<Arc<dyn DatabaseDriver>, AppError> {
    ensure_connection_with_db_inner(state, id, database).await
}

async fn ensure_connection_with_db_inner(
    state: &AppState,
    id: i64,
    database: Option<String>,
) -> Result<Arc<dyn DatabaseDriver>, AppError> {
    let key = connection_pool_key(id, &database);

    if let Some(driver) = state.pool_manager.get_connection(&key).await {
        let local_db = {
            let lock = state.local_db.lock().await;
            lock.clone()
        };

        if let Some(db) = local_db {
            if db.get_connection_by_id(id).await.is_err() {
                state.pool_manager.remove_by_prefix(&id.to_string()).await;
                return Err(AppError::internal(format!(
                    "Connection with ID {} no longer exists",
                    id
                )));
            }
        }
        return Ok(driver);
    }

    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };

    let db = local_db.ok_or_else(|| AppError::internal("Local DB not initialized"))?;
    let mut form = db.get_connection_form_by_id(id).await?;

    if let Some(db_name) = database {
        if !db_name.is_empty() {
            form.database = Some(db_name);
        }
    }

    state.pool_manager.connect(&key, &form).await
}

async fn execute_with_retry_core<T, E, Ensure, EnsureFut, Remove, RemoveFut, Task, TaskFut>(
    mut ensure: Ensure,
    mut remove: Remove,
    task: Task,
) -> Result<T, AppError>
where
    Ensure: FnMut() -> EnsureFut,
    EnsureFut: std::future::Future<Output = Result<Arc<dyn DatabaseDriver>, AppError>>,
    Remove: FnMut() -> RemoveFut,
    RemoveFut: std::future::Future<Output = ()>,
    Task: Fn(Arc<dyn DatabaseDriver>) -> TaskFut,
    TaskFut: std::future::Future<Output = Result<T, E>>,
    E: Into<AppError>,
{
    let driver = ensure().await?;
    match task(driver.clone()).await {
        Ok(res) => Ok(res),
        Err(e) => {
            let err = e.into();
            if is_connection_error(&err.to_string()) {
                tracing::warn!("Connection error detected, retrying");
                remove().await;
                let driver = ensure().await?;
                task(driver).await.map_err(|e| {
                    let err = e.into();
                    tracing::error!(error = %err, "Retry failed");
                    err
                })
            } else {
                tracing::error!(error = %err, "Operation failed");
                Err(err)
            }
        }
    }
}

pub async fn execute_with_retry<F, Fut, T, E>(
    state: &State<'_, AppState>,
    id: i64,
    database: Option<String>,
    task: F,
) -> Result<T, AppError>
where
    F: Fn(Arc<dyn DatabaseDriver>) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Into<AppError>,
{
    execute_with_retry_inner(state.inner(), id, database, task).await
}

pub async fn execute_with_retry_from_app_state<F, Fut, T, E>(
    state: &AppState,
    id: i64,
    database: Option<String>,
    task: F,
) -> Result<T, AppError>
where
    F: Fn(Arc<dyn DatabaseDriver>) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Into<AppError>,
{
    execute_with_retry_inner(state, id, database, task).await
}

async fn execute_with_retry_inner<F, Fut, T, E>(
    state: &AppState,
    id: i64,
    database: Option<String>,
    task: F,
) -> Result<T, AppError>
where
    F: Fn(Arc<dyn DatabaseDriver>) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: Into<AppError>,
{
    let key = connection_pool_key(id, &database);
    execute_with_retry_core(
        || ensure_connection_with_db_from_app_state(state, id, database.clone()),
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
    use super::{connection_pool_key, execute_with_retry_core, is_connection_error};
    use crate::db::drivers::{DatabaseDriver, DriverResult};
    use crate::error::AppError;
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
        async fn test_connection(&self) -> DriverResult<()> {
            Ok(())
        }
        async fn list_databases(&self) -> DriverResult<Vec<String>> {
            Ok(vec![])
        }
        async fn list_schemas(&self) -> DriverResult<Vec<String>> {
            Ok(vec![])
        }
        async fn list_tables(&self, _schema: Option<String>) -> DriverResult<Vec<TableInfo>> {
            Ok(vec![])
        }
        async fn get_table_structure(
            &self,
            _schema: String,
            _table: String,
        ) -> DriverResult<TableStructure> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_table_metadata(
            &self,
            _schema: String,
            _table: String,
        ) -> DriverResult<TableMetadata> {
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
        ) -> DriverResult<TableDataResponse> {
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
        ) -> DriverResult<TableDataResponse> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn execute_query(&self, _sql: String) -> DriverResult<QueryResult> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
        }
        async fn get_schema_overview(
            &self,
            _schema: Option<String>,
        ) -> DriverResult<SchemaOverview> {
            Err(crate::error::AppError::unsupported("Unimplemented"))
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

        let result: Result<String, AppError> = execute_with_retry_core(
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
                        Err(crate::error::AppError::query_failed(
                            "connection reset by peer",
                        ))
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

        let result: Result<String, AppError> = execute_with_retry_core(
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
                    Err::<String, crate::error::AppError>(crate::error::AppError::query_failed(
                        "pool closed",
                    ))
                }
            },
        )
        .await;

        assert_eq!(result.unwrap_err().to_string(), "[ERR-2001] pool closed");
        assert_eq!(task_calls.load(Ordering::SeqCst), 2);
        assert_eq!(ensure_calls.load(Ordering::SeqCst), 2);
        assert_eq!(remove_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn connection_pool_key_handles_none_and_empty_db() {
        assert_eq!(connection_pool_key(1, &None), "1");
        assert_eq!(connection_pool_key(1, &Some("".to_string())), "1");
        assert_eq!(connection_pool_key(1, &Some("app".to_string())), "1:app");
    }

    #[test]
    fn is_connection_error_matches_common_messages() {
        assert!(is_connection_error("connection reset by peer"));
        assert!(is_connection_error("broken pipe"));
        assert!(is_connection_error("timeout while waiting"));
        assert!(is_connection_error("EOF while reading"));
        assert!(!is_connection_error("syntax error at or near"));
    }
}
