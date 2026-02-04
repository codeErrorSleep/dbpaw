use crate::models::{ConnectionForm, TableInfo, TableStructure};
use crate::db::drivers::get_driver;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_tables_by_conn(form: ConnectionForm) -> Result<Vec<TableInfo>, String> {
    let driver = get_driver(&form)?;
    driver.list_tables(form.schema).await
}

#[tauri::command]
pub async fn list_tables(state: State<'_, AppState>, id: i64) -> Result<Vec<TableInfo>, String> {
    let local_db = state.local_db.lock().await;
    let db = local_db.as_ref().ok_or("Local DB not initialized")?;
    
    let form = db.get_connection_form_by_id(id).await?;
    let driver = get_driver(&form)?;
    driver.list_tables(form.schema).await
}

#[tauri::command]
pub async fn get_table_structure(state: State<'_, AppState>, id: i64, schema: String, table: String) -> Result<TableStructure, String> {
    let local_db = state.local_db.lock().await;
    let db = local_db.as_ref().ok_or("Local DB not initialized")?;
    
    let form = db.get_connection_form_by_id(id).await?;
    let driver = get_driver(&form)?;
    driver.get_table_structure(schema, table).await
}
