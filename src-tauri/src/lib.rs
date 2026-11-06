#![recursion_limit = "256"]

use crate::db::local::LocalDb;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn set_log_level(state: tauri::State<'_, AppState>, level: String) -> Result<(), String> {
    let handle = state.log_reload_handle.lock().await;
    if let Some(ref h) = *handle {
        let filter = tracing_subscriber::EnvFilter::try_new(&level)
            .map_err(|e| format!("Invalid log level: {}", e))?;
        h.reload(filter)
            .map_err(|e| format!("Failed to set log level: {}", e))?;
    }
    Ok(())
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

            // Initialize structured logging
            let (log_handle, log_guard) = crate::log::init_logging(&handle);
            {
                let state = handle.state::<AppState>();
                let mut h = state.log_reload_handle.blocking_lock();
                *h = Some(log_handle);
                let mut g = state._log_guard.blocking_lock();
                *g = Some(log_guard);
            }

            // Initialize Oracle Instant Client library path
            // This must be done before any Oracle connections are made
            init_oracle_lib_path(&handle);

            // Explicitly restore window state on Windows as a workaround for upstream timing issues
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.restore_state(StateFlags::all());
            }

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
                    tracing::error!(error = %e, "Error setting up menu");
                }
            }

            // Initialize local database (blocking to avoid race conditions)
            let handle_for_cleanup = handle.clone();
            tauri::async_runtime::block_on(async move {
                let state = handle.state::<AppState>();
                match LocalDb::init(&handle).await {
                    Ok(db) => {
                        let mut lock = state.local_db.lock().await;
                        *lock = Some(Arc::new(db));
                        tracing::info!("Local DB initialized successfully");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to initialize local DB");
                        // Make the error visible in the frontend if possible, or at least easier to debug
                    }
                }
            });

            // Start connection pool cleanup task
            tauri::async_runtime::spawn(async move {
                let state = handle_for_cleanup.state::<AppState>();
                state.pool_manager.start_cleanup_task().await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            set_log_level,
            commands::connection::get_connections,
            commands::connection::create_connection,
            commands::connection::update_connection,
            commands::connection::delete_connection,
            commands::connection::import_connections,
            commands::connection::test_connection_ephemeral,
            commands::connection::test_connection_saved_edit,
            commands::connection::list_databases,
            commands::connection::list_databases_by_id,
            commands::metadata::list_schemas,
            commands::connection::create_database_by_id,
            commands::connection::get_mysql_charsets_by_id,
            commands::connection::get_mysql_collations_by_id,
            commands::metadata::list_tables,
            commands::metadata::list_routines,
            commands::metadata::list_events,
            commands::metadata::list_sequences,
            commands::metadata::list_types,
            commands::metadata::list_synonyms,
            commands::metadata::list_packages,
            commands::metadata::get_table_structure,
            commands::metadata::get_table_ddl,
            commands::metadata::get_routine_ddl,
            commands::metadata::get_table_metadata,
            commands::metadata::get_schema_overview,
            commands::metadata::get_schema_foreign_keys,
            commands::metadata::get_driver_capabilities,
            commands::metadata::list_tables_by_conn,
            commands::query::analyze_sql_risk,
            commands::query::execute_query,
            commands::query::get_table_data,
            commands::query::cancel_query,
            commands::query::get_table_data_by_conn,
            commands::query::execute_by_conn,
            commands::query::list_sql_execution_logs,
            commands::storage::save_query,
            commands::storage::get_saved_queries,
            commands::storage::update_saved_query,
            commands::storage::delete_saved_query,
            commands::ai::providers::ai_list_providers,
            commands::ai::providers::ai_create_provider,
            commands::ai::providers::ai_update_provider,
            commands::ai::providers::ai_delete_provider,
            commands::ai::providers::ai_set_default_provider,
            commands::ai::providers::ai_clear_provider_api_key,
            commands::ai::chat::ai_chat_start,
            commands::ai::chat::ai_chat_continue,
            commands::ai::conversations::ai_list_conversations,
            commands::ai::conversations::ai_get_conversation,
            commands::ai::conversations::ai_delete_conversation,
            commands::transfer::export_table_data,
            commands::transfer::export_database_sql,
            commands::transfer::export_query_result,
            commands::transfer::import_sql_file,
            commands::redis::redis_list_databases,
            commands::redis::redis_scan_keys,
            commands::redis::redis_get_key,
            commands::redis::redis_set_key,
            commands::redis::redis_update_key,
            commands::redis::redis_delete_key,
            commands::redis::redis_rename_key,
            commands::redis::redis_set_ttl,
            commands::redis::redis_get_key_page,
            commands::redis::redis_patch_key,
            commands::redis::redis_get_stream_range,
            commands::redis::redis_get_stream_view,
            commands::redis::redis_execute_raw,
            commands::redis::redis_server_info,
            commands::redis::redis_server_config,
            commands::redis::redis_slowlog_get,
            commands::redis::list_redis_command_logs,
            commands::redis::redis_bitmap_get_bit,
            commands::redis::redis_bitmap_count,
            commands::redis::redis_bitmap_pos,
            commands::redis::redis_hll_pfadd,
            commands::redis::redis_geo_add,
            commands::redis::redis_geo_pos,
            commands::redis::redis_geo_dist,
            commands::redis::redis_geo_search,
            commands::redis::redis_zrangebyscore,
            commands::redis::redis_zrank,
            commands::redis::redis_zscore,
            commands::redis::redis_zmscore,
            commands::redis::redis_zrangebylex,
            commands::redis::redis_zlexcount,
            commands::redis::redis_zpopmin,
            commands::redis::redis_zpopmax,
            commands::redis::redis_set_operation,
            commands::redis::redis_sismember,
            commands::redis::redis_smove,
            commands::redis::redis_batch_key_ops,
            commands::redis::redis_mget,
            commands::redis::redis_mset,
            commands::redis::redis_lindex,
            commands::redis::redis_lpos,
            commands::redis::redis_ltrim,
            commands::redis::redis_linsert,
            commands::redis::redis_lmove,
            commands::redis::redis_xgroup_create,
            commands::redis::redis_xgroup_del,
            commands::redis::redis_xgroup_setid,
            commands::redis::redis_xack,
            commands::redis::redis_xpending,
            commands::redis::redis_xclaim,
            commands::redis::redis_xtrim,
            commands::redis::redis_xreadgroup,
            commands::redis::redis_cluster_info,
            commands::elasticsearch::elasticsearch_test_connection,
            commands::elasticsearch::elasticsearch_test_connection_ephemeral,
            commands::elasticsearch::elasticsearch_list_indices,
            commands::elasticsearch::elasticsearch_get_index_mapping,
            commands::elasticsearch::elasticsearch_create_index,
            commands::elasticsearch::elasticsearch_delete_index,
            commands::elasticsearch::elasticsearch_refresh_index,
            commands::elasticsearch::elasticsearch_open_index,
            commands::elasticsearch::elasticsearch_close_index,
            commands::elasticsearch::elasticsearch_search_documents,
            commands::elasticsearch::elasticsearch_get_document,
            commands::elasticsearch::elasticsearch_upsert_document,
            commands::elasticsearch::elasticsearch_delete_document,
            commands::elasticsearch::elasticsearch_export_documents,
            commands::elasticsearch::elasticsearch_import_documents,
            commands::elasticsearch::elasticsearch_execute_raw,
            commands::mongodb::mongodb_test_connection,
            commands::mongodb::mongodb_test_connection_ephemeral,
            commands::mongodb::mongodb_list_databases,
            commands::mongodb::mongodb_list_collections,
            commands::mongodb::mongodb_find_documents,
            commands::mongodb::mongodb_get_document,
            commands::mongodb::mongodb_insert_document,
            commands::mongodb::mongodb_update_document,
            commands::mongodb::mongodb_delete_document,
            commands::system::list_system_fonts,
            commands::mcp::mcp_status,
            commands::mcp::mcp_start,
            commands::mcp::mcp_stop,
            commands::mcp::mcp_get_tools,
            commands::mcp::mcp_detect_clients,
            commands::mcp::mcp_configure_client,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::Exit => {
            let _ = app_handle.save_window_state(StateFlags::all());
            let state = app_handle.state::<AppState>();
            tauri::async_runtime::block_on(async {
                // Kill MCP server process if running
                let mut lock = state.mcp_process.lock().await;
                if let Some(mut child) = lock.take() {
                    let _ = child.kill().await;
                }
                state.pool_manager.close_all().await;
            });
        }
        _ => {}
    });
}

pub mod ai;
pub mod commands;
pub mod connection_input;
pub mod datasources;
pub mod db;
pub mod error;
pub mod events;
pub mod import;
pub mod log;
pub mod mcp;
pub mod models;
pub mod network;
pub mod services;
pub mod sql;
pub mod ssh;
pub mod state;
pub mod utils;

/// Initialize Oracle Instant Client library path from bundled resources.
///
/// On macOS: sets DYLD_LIBRARY_PATH (works in dev mode, bypassed in hardened runtime)
/// On Linux: sets LD_LIBRARY_PATH
/// On Windows: adds to PATH
///
/// For hardened macOS builds, the library is loaded directly from the resource path.
fn init_oracle_lib_path(app_handle: &tauri::AppHandle) {
    // Try to get resource directory
    let resource_dir = match app_handle.path().resource_dir() {
        Ok(dir) => dir,
        Err(_) => return,
    };

    let oracle_dir = resource_dir.join("oracle");
    if !oracle_dir.exists() {
        // Oracle libraries not bundled, skip initialization
        return;
    }

    let oracle_path = oracle_dir.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        // For macOS, we need to set DYLD_LIBRARY_PATH
        // Note: This only works in dev mode. In production with hardened runtime,
        // the library must be loaded via absolute path or @rpath
        let current = std::env::var("DYLD_LIBRARY_PATH").unwrap_or_default();
        let new_path = if current.is_empty() {
            oracle_path.clone()
        } else {
            format!("{}:{}", oracle_path, current)
        };
        unsafe {
            std::env::set_var("DYLD_LIBRARY_PATH", &new_path);
        }
        tracing::info!(path = %oracle_path, "Oracle: Set DYLD_LIBRARY_PATH");
    }

    #[cfg(target_os = "linux")]
    {
        let current = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let new_path = if current.is_empty() {
            oracle_path.clone()
        } else {
            format!("{}:{}", oracle_path, current)
        };
        unsafe {
            std::env::set_var("LD_LIBRARY_PATH", &new_path);
        }
        tracing::info!(path = %oracle_path, "Oracle: Set LD_LIBRARY_PATH");
    }

    #[cfg(target_os = "windows")]
    {
        let current = std::env::var("PATH").unwrap_or_default();
        let new_path = if current.is_empty() {
            oracle_path.clone()
        } else {
            format!("{};{}", oracle_path, current)
        };
        unsafe {
            std::env::set_var("PATH", &new_path);
        }
        tracing::info!(path = %oracle_path, "Oracle: Added to PATH");
    }

    // Also set ORACLE_HOME if not already set
    if std::env::var("ORACLE_HOME").is_err() {
        unsafe {
            std::env::set_var("ORACLE_HOME", &oracle_path);
        }
        tracing::info!(path = %oracle_path, "Oracle: Set ORACLE_HOME");
    }
}
