#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use std::sync::Mutex;

use state::AppState;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("file-organizer.db");
            let quarantine_root = app_data_dir.join("Quarantine");
            std::fs::create_dir_all(&quarantine_root)?;

            organizer_core::logutil::init_file_logging(&app_data_dir);
            let store = organizer_core::store::Store::open(&db_path)
                .map_err(std::io::Error::other)?;
            // Remember quarantine root for deletion safety checks.
            let _ = store.set_setting(
                "quarantine_root",
                &quarantine_root.to_string_lossy(),
            );

            app.manage(AppState {
                store: Mutex::new(store),
                scans: Mutex::new(std::collections::HashMap::new()),
                quarantine_root,
                app_data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_scan,
            commands::cancel_scan,
            commands::get_recent_sessions,
            commands::get_scan_summary,
            commands::list_files,
            commands::get_problem_files,
            commands::get_duplicate_groups,
            commands::rebuild_analysis,
            commands::quarantine_files,
            commands::get_quarantine_items,
            commands::restore_preview,
            commands::restore_execute,
            commands::delete_permanently,
            commands::get_operations,
            commands::get_operation_items,
            commands::list_exclusions,
            commands::add_exclusion,
            commands::remove_exclusion,
            commands::get_settings,
            commands::save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running file organizer");
}

