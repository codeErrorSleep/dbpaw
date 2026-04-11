use crate::db::local::LocalDb;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .on_menu_event(|app, event| {
            if event.id() == "settings" {
                let _ = app.emit("open-settings", ());
            } else if event.id() == "debug_reload" {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.reload();
                }
            } else if event.id() == "debug_toggle_devtools" {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_devtools_open() {
                        window.close_devtools();
                    } else {
                        window.open_devtools();
                    }
                }
            }
        })
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle().clone();

            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
                // Use a closure to handle potential errors gracefully
                if let Err(e) = (|| -> tauri::Result<()> {
                    let app_menu = Submenu::new(&handle, "App", true)?;
                    let edit_menu = Submenu::new(&handle, "Edit", true)?;
                    let developer_menu = Submenu::new(&handle, "Developer", true)?;

                    let about = PredefinedMenuItem::about(&handle, None, None)?;
                    let settings = MenuItem::with_id(
                        &handle,
                        "settings",
                        "Settings...",
                        true,
                        Some("CmdOrCtrl+,"),
                    )?;
                    let separator = PredefinedMenuItem::separator(&handle)?;
                    let services = PredefinedMenuItem::services(&handle, None)?;
                    let hide = PredefinedMenuItem::hide(&handle, None)?;
                    let hide_others = PredefinedMenuItem::hide_others(&handle, None)?;
                    let show_all = PredefinedMenuItem::show_all(&handle, None)?;
                    let quit = PredefinedMenuItem::quit(&handle, None)?;

                    app_menu.append(&about)?;
                    app_menu.append(&separator)?;
                    app_menu.append(&settings)?;
                    app_menu.append(&separator)?;
                    app_menu.append(&services)?;
                    app_menu.append(&separator)?;
                    app_menu.append(&hide)?;
                    app_menu.append(&hide_others)?;
                    app_menu.append(&show_all)?;
                    app_menu.append(&separator)?;
                    app_menu.append(&quit)?;

                    let undo = PredefinedMenuItem::undo(&handle, None)?;
                    let redo = PredefinedMenuItem::redo(&handle, None)?;
                    let cut = PredefinedMenuItem::cut(&handle, None)?;
                    let copy = PredefinedMenuItem::copy(&handle, None)?;
                    let paste = PredefinedMenuItem::paste(&handle, None)?;
                    let select_all = PredefinedMenuItem::select_all(&handle, None)?;
                    let reload = MenuItem::with_id(
                        &handle,
                        "debug_reload",
                        "Reload",
                        true,
                        Some("CmdOrCtrl+R"),
                    )?;
                    let toggle_devtools = MenuItem::with_id(
                        &handle,
                        "debug_toggle_devtools",
                        "Toggle DevTools",
                        true,
                        Some("Alt+CmdOrCtrl+I"),
                    )?;

                    edit_menu.append(&undo)?;
                    edit_menu.append(&redo)?;
                    edit_menu.append(&separator)?;
                    edit_menu.append(&cut)?;
                    edit_menu.append(&copy)?;
                    edit_menu.append(&paste)?;
                    edit_menu.append(&select_all)?;

                    developer_menu.append(&reload)?;
                    developer_menu.append(&toggle_devtools)?;

                    let menu =
                        Menu::with_items(&handle, &[&app_menu, &edit_menu, &developer_menu])?;
                    app.set_menu(menu)?;
                    Ok(())
                })() {
                    eprintln!("Error setting up menu: {}", e);
                }
            }

            // Initialize local database (blocking to avoid race conditions)
            tauri::async_runtime::block_on(async move {
                let state = handle.state::<AppState>();
                match LocalDb::init(&handle).await {
                    Ok(db) => {
                        let mut lock = state.local_db.lock().await;
                        *lock = Some(Arc::new(db));
                        println!("Local DB initialized successfully");
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize local DB: {}", e);
                        // Make the error visible in the frontend if possible, or at least easier to debug
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
            commands::metadata::get_table_metadata,
            commands::metadata::get_schema_overview,
            commands::query::execute_query,
            commands::query::get_table_data,
            commands::query::cancel_query,
            commands::connection::test_connection_ephemeral,
            commands::metadata::list_tables_by_conn,
            commands::query::get_table_data_by_conn,
            commands::query::execute_by_conn,
            commands::query::list_sql_execution_logs,
            commands::connection::list_databases,
            commands::connection::list_databases_by_id,
            commands::connection::create_database_by_id,
            commands::connection::get_mysql_charsets_by_id,
            commands::connection::get_mysql_collations_by_id,
            commands::storage::save_query,
            commands::storage::get_saved_queries,
            commands::storage::update_saved_query,
            commands::storage::delete_saved_query,
            commands::ai::ai_list_providers,
            commands::ai::ai_create_provider,
            commands::ai::ai_update_provider,
            commands::ai::ai_delete_provider,
            commands::ai::ai_set_default_provider,
            commands::ai::ai_clear_provider_api_key,
            commands::ai::ai_chat_start,
            commands::ai::ai_chat_continue,
            commands::ai::ai_list_conversations,
            commands::ai::ai_get_conversation,
            commands::ai::ai_delete_conversation,
            commands::transfer::export_table_data,
            commands::transfer::export_query_result,
            commands::transfer::import_sql_file,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::Exit => {
            let state = app_handle.state::<AppState>();
            tauri::async_runtime::block_on(async {
                state.pool_manager.close_all().await;
            });
        }
        _ => {}
    });
}

pub mod ai;
pub mod commands;
pub mod connection_input;
pub mod db;
pub mod error;
pub mod events;
pub mod models;
pub mod ssh;
pub mod state;
pub mod utils;
