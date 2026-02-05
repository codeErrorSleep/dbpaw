use crate::db::drivers::get_driver;
use crate::models::{ConnectionForm, TableInfo, TableStructure};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_tables_by_conn(form: ConnectionForm) -> Result<Vec<TableInfo>, String> {
    let driver = get_driver(&form)?;
    driver.list_tables(form.schema).await
}

#[tauri::command]
pub async fn list_tables(
    state: State<'_, AppState>,
    id: i64,
    database: Option<String>,
    schema: Option<String>,
) -> Result<Vec<TableInfo>, String> {
    let local_db = state.local_db.lock().await;
    let db = local_db.as_ref().ok_or("Local DB not initialized")?;

    let mut form = db.get_connection_form_by_id(id).await?;

    // 如果指定了 database 或 schema，覆盖保存的配置，以便连接到指定的库/模式
    if let Some(db_name) = database {
        form.database = Some(db_name);
    }
    if let Some(sch) = schema {
        form.schema = Some(sch);
    }

    let driver = get_driver(&form)?;
    driver.list_tables(form.schema).await
}

#[tauri::command]
pub async fn get_table_structure(
    state: State<'_, AppState>,
    id: i64,
    schema: String,
    table: String,
) -> Result<TableStructure, String> {
    let local_db = state.local_db.lock().await;
    let db = local_db.as_ref().ok_or("Local DB not initialized")?;

    let form = db.get_connection_form_by_id(id).await?;
    let driver = get_driver(&form)?;
    driver.get_table_structure(schema, table).await
}
