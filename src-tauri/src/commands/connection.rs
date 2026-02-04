use crate::models::{Connection, ConnectionForm, TestConnectionResult};
use crate::db::drivers::get_driver;
use crate::state::AppState;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn list_databases(form: ConnectionForm) -> Result<Vec<String>, String> {
    let driver = get_driver(&form)?;
    driver.list_databases().await
}


#[tauri::command]
pub async fn test_connection_ephemeral(form: ConnectionForm) -> Result<TestConnectionResult, String> {
    let start = Instant::now();
    let driver = get_driver(&form)?;
    driver.test_connection().await.map_err(|e| e.to_string())?;
    
    let elapsed = start.elapsed().as_millis() as i64;
    Ok(TestConnectionResult { success: true, message: "连接成功".to_string(), latency_ms: Some(elapsed) })
}

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let local_db = state.local_db.lock().await;
    if let Some(db) = local_db.as_ref() {
        db.list_connections().await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[tauri::command]
pub async fn create_connection(state: State<'_, AppState>, form: ConnectionForm) -> Result<Connection, String> {
    let local_db = state.local_db.lock().await;
    if let Some(db) = local_db.as_ref() {
        db.create_connection(form).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[tauri::command]
pub async fn update_connection(state: State<'_, AppState>, id: i64, form: ConnectionForm) -> Result<Connection, String> {
    let local_db = state.local_db.lock().await;
    if let Some(db) = local_db.as_ref() {
        db.update_connection(id, form).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let local_db = state.local_db.lock().await;
    if let Some(db) = local_db.as_ref() {
        db.delete_connection(id).await
    } else {
        Err("Local DB not initialized".to_string())
    }
}
