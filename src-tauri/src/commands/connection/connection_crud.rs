use crate::db::drivers::DatabaseDriver;
use crate::error::AppError;
use crate::models::{Connection, ConnectionForm, TestConnectionResult};
use crate::state::AppState;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn list_databases(form: ConnectionForm) -> Result<Vec<String>, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    let driver = crate::db::drivers::connect(&form).await?;
    driver.list_databases().await
}

#[tauri::command]
pub async fn list_databases_by_id(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<String>, AppError> {
    crate::commands::execute_with_retry(&state, id, None, |driver| async move {
        driver.list_databases().await
    })
    .await
}

pub async fn list_databases_by_id_direct(
    state: &AppState,
    id: i64,
) -> Result<Vec<String>, AppError> {
    crate::commands::execute_with_retry_from_app_state(state, id, None, |driver| async move {
        driver.list_databases().await
    })
    .await
}

#[tauri::command]
pub async fn test_connection_ephemeral(
    form: ConnectionForm,
) -> Result<TestConnectionResult, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    test_connection_form(&form).await
}

#[tauri::command]
pub async fn test_connection_saved_edit(
    state: State<'_, AppState>,
    id: i64,
    form: ConnectionForm,
) -> Result<TestConnectionResult, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    let saved = crate::commands::get_connection_form_by_id(&state, id).await?;
    let form = merge_saved_secrets_for_edit_test(form, &saved);
    test_connection_form(&form).await
}

async fn test_connection_form(form: &ConnectionForm) -> Result<TestConnectionResult, AppError> {
    let start = Instant::now();
    if form.driver == "redis" {
        let mut conn = crate::datasources::redis::connect(form, None).await?;
        crate::datasources::redis::ping(&mut conn).await?;
    } else if form.driver == "elasticsearch" {
        let client = crate::datasources::elasticsearch::ElasticsearchClient::connect(form)?;
        client.test_connection().await?;
    } else if form.driver == "mongodb" {
        let driver = crate::db::drivers::mongodb::MongoDBDriver::connect(form).await?;
        driver.test_connection().await?;
    } else {
        let driver = crate::db::drivers::connect(form).await?;
        driver.test_connection().await?;
    }

    let elapsed = start.elapsed().as_millis() as i64;
    Ok(TestConnectionResult {
        success: true,
        message: "Connection successful".to_string(),
        latency_ms: Some(elapsed),
    })
}

fn merge_saved_secrets_for_edit_test(
    mut form: ConnectionForm,
    saved: &ConnectionForm,
) -> ConnectionForm {
    crate::network::merge_proxy_password(&mut form, saved);
    fill_empty_secret(&mut form.password, &saved.password);
    fill_empty_secret(&mut form.ssh_password, &saved.ssh_password);
    fill_empty_secret(&mut form.sentinel_password, &saved.sentinel_password);
    fill_empty_secret(&mut form.api_key_secret, &saved.api_key_secret);
    fill_empty_secret(&mut form.api_key_encoded, &saved.api_key_encoded);
    form
}

fn fill_empty_secret(current: &mut Option<String>, saved: &Option<String>) {
    if !is_empty_secret(current) {
        return;
    }
    if let Some(saved_secret) = saved.as_deref().filter(|value| !value.is_empty()) {
        *current = Some(saved_secret.to_string());
    }
}

fn is_empty_secret(secret: &Option<String>) -> bool {
    match secret {
        Some(value) => value.is_empty(),
        None => true,
    }
}

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, AppError> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        db.list_connections().await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

pub async fn get_connections_direct(state: &AppState) -> Result<Vec<Connection>, AppError> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        db.list_connections().await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

#[tauri::command]
pub async fn create_connection(
    state: State<'_, AppState>,
    form: ConnectionForm,
) -> Result<Connection, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        db.create_connection(form).await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

pub async fn create_connection_direct(
    state: &AppState,
    form: ConnectionForm,
) -> Result<Connection, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        db.create_connection(form).await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

#[tauri::command]
pub async fn update_connection(
    state: State<'_, AppState>,
    id: i64,
    form: ConnectionForm,
) -> Result<Connection, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        // If connection is updated, we should remove it from pool so next usage reconnects with new config
        state.pool_manager.remove_by_prefix(&id.to_string()).await;

        db.update_connection(id, form).await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

pub async fn update_connection_direct(
    state: &AppState,
    id: i64,
    form: ConnectionForm,
) -> Result<Connection, AppError> {
    let form = crate::connection_input::normalize_connection_form(form)?;
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        state.pool_manager.remove_by_prefix(&id.to_string()).await;
        db.update_connection(id, form).await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    delete_connection_direct(&state, id).await
}

pub async fn delete_connection_direct(state: &AppState, id: i64) -> Result<(), AppError> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    if let Some(db) = local_db {
        state.pool_manager.remove_by_prefix(&id.to_string()).await;
        state.redis_cache.lock().await.remove_by_connection_id(id);
        db.delete_connection(id).await
    } else {
        Err(AppError::internal("Local DB not initialized").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_test_connection_uses_saved_secrets_when_form_secrets_are_empty() {
        let saved = ConnectionForm {
            driver: "postgres".to_string(),
            password: Some("saved-db-password".to_string()),
            ssh_password: Some("saved-ssh-password".to_string()),
            sentinel_password: Some("saved-sentinel-password".to_string()),
            api_key_secret: Some("saved-api-secret".to_string()),
            api_key_encoded: Some("saved-api-encoded".to_string()),
            ..Default::default()
        };
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            password: Some(String::new()),
            ssh_password: Some(String::new()),
            sentinel_password: None,
            api_key_secret: Some(String::new()),
            api_key_encoded: None,
            ..Default::default()
        };

        let merged = merge_saved_secrets_for_edit_test(form, &saved);

        assert_eq!(merged.password.as_deref(), Some("saved-db-password"));
        assert_eq!(merged.ssh_password.as_deref(), Some("saved-ssh-password"));
        assert_eq!(
            merged.sentinel_password.as_deref(),
            Some("saved-sentinel-password")
        );
        assert_eq!(merged.api_key_secret.as_deref(), Some("saved-api-secret"));
        assert_eq!(merged.api_key_encoded.as_deref(), Some("saved-api-encoded"));
    }

    #[test]
    fn edit_test_connection_prefers_new_secrets_when_present() {
        let saved = ConnectionForm {
            driver: "postgres".to_string(),
            password: Some("saved-db-password".to_string()),
            ssh_password: Some("saved-ssh-password".to_string()),
            sentinel_password: Some("saved-sentinel-password".to_string()),
            api_key_secret: Some("saved-api-secret".to_string()),
            api_key_encoded: Some("saved-api-encoded".to_string()),
            ..Default::default()
        };
        let form = ConnectionForm {
            driver: "postgres".to_string(),
            password: Some("new-db-password".to_string()),
            ssh_password: Some("new-ssh-password".to_string()),
            sentinel_password: Some("new-sentinel-password".to_string()),
            api_key_secret: Some("new-api-secret".to_string()),
            api_key_encoded: Some("new-api-encoded".to_string()),
            ..Default::default()
        };

        let merged = merge_saved_secrets_for_edit_test(form, &saved);

        assert_eq!(merged.password.as_deref(), Some("new-db-password"));
        assert_eq!(merged.ssh_password.as_deref(), Some("new-ssh-password"));
        assert_eq!(
            merged.sentinel_password.as_deref(),
            Some("new-sentinel-password")
        );
        assert_eq!(merged.api_key_secret.as_deref(), Some("new-api-secret"));
        assert_eq!(merged.api_key_encoded.as_deref(), Some("new-api-encoded"));
    }
}
