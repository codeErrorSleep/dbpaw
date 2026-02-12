use crate::models::{ConnectionForm, TableDataResponse, QueryResult};
use crate::db::drivers::get_driver;
use crate::state::AppState;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn get_table_data_by_conn(
    form: ConnectionForm,
    schema: String,
    table: String,
    page: i64,
    limit: i64,
) -> Result<TableDataResponse, String> {
    let driver = get_driver(&form)?;
    driver.get_table_data(schema, table, page, limit, None, None).await
}

#[tauri::command]
pub async fn execute_query(app_handle: tauri::AppHandle, state: State<'_, AppState>, id: i64, query: String, database: Option<String>) -> Result<QueryResult, String> {
    let query_id = format!("q-{}", id);
    let _ = app_handle.emit("query.progress", serde_json::json!({"queryId": query_id, "phase": "prepare"}));
    
    let local_db = state.local_db.lock().await;
    let db = local_db.as_ref().ok_or("Local DB not initialized")?;
    
    let mut form = db.get_connection_form_by_id(id).await?;
    
    // If a specific database is requested, override the default
    if let Some(db_name) = database {
        if !db_name.is_empty() {
             // For PostgreSQL, 'database' field is used for DB name. 
             // For MySQL, it is also 'database'.
             // We update the form to connect to this specific DB.
             form.database = Some(db_name);
        }
    }

    let driver = get_driver(&form)?;
    
    let result = driver.execute_query(query).await;
    
    if let Ok(res) = &result {
        // Stream first chunk for UX (simulated)
        if !res.data.is_empty() {
            let _ = app_handle.emit("query.chunk", serde_json::json!({
                "queryId": query_id, 
                "rows": res.data.iter().take(50).collect::<Vec<_>>()
            }));
        }
    }
    
    result
}

#[tauri::command]
pub async fn get_table_data(
    state: State<'_, AppState>,
    id: i64,
    schema: String,
    table: String,
    page: i64,
    limit: i64,
    filter: Option<String>,
    sort_column: Option<String>,
    sort_direction: Option<String>,
) -> Result<TableDataResponse, String> {
    let _ = filter; // TODO: Implement filters
    
    let local_db = state.local_db.lock().await;
    let db = local_db.as_ref().ok_or("Local DB not initialized")?;
    
    let form = db.get_connection_form_by_id(id).await?;
    let driver = get_driver(&form)?;
    driver.get_table_data(schema, table, page, limit, sort_column, sort_direction).await
}

#[tauri::command]
pub async fn cancel_query(_uuid: String, _query_id: String) -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub async fn execute_by_conn(app_handle: tauri::AppHandle, form: ConnectionForm, sql: String) -> Result<QueryResult, String> {
    let query_id = "q-conn-ephemeral";
    let _ = app_handle.emit("query.progress", serde_json::json!({"queryId": query_id, "phase": "prepare"}));
    
    let driver = get_driver(&form)?;
    let result = driver.execute_query(sql).await;
    
    if let Ok(res) = &result {
        if !res.data.is_empty() {
             let _ = app_handle.emit("query.chunk", serde_json::json!({
                "queryId": query_id, 
                "rows": res.data.iter().take(50).collect::<Vec<_>>()
            }));
        }
    }
    result
}
