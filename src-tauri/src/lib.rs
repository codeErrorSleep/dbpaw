use crate::db::local::LocalDb;
use crate::state::AppState;
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize local database
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                match LocalDb::init(&handle).await {
                    Ok(db) => {
                        *state.local_db.lock().await = Some(db);
                        println!("Local DB initialized successfully");
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize local DB: {}", e);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::connection::get_connections,
            commands::connection::create_connection,
            commands::connection::update_connection,
            commands::connection::delete_connection,
            commands::metadata::list_tables,
            commands::metadata::get_table_structure,
            commands::metadata::get_table_ddl,
            commands::metadata::get_schema_overview,
            commands::query::execute_query,
            commands::query::get_table_data,
            commands::query::cancel_query,
            commands::connection::test_connection_ephemeral,
            commands::metadata::list_tables_by_conn,
            commands::query::get_table_data_by_conn,
            commands::query::execute_by_conn,
            commands::connection::list_databases,
            commands::connection::list_databases_by_id,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub mod commands;
pub mod db;
pub mod error;
pub mod events;
pub mod models;
pub mod state;
pub mod utils;
