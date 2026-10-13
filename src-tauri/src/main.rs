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
            let db_path = app_data_dir.join("fileharbor.db");
            let quarantine_root = app_data_dir.join("FileHarbor");
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
                backups: Mutex::new(std::collections::HashMap::new()),
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
            commands::quarantine_raw_paths,
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
            commands::save_settings,
            commands::create_backup_set,
            commands::list_backup_sets,
            commands::get_backup_set,
            commands::update_backup_set,
            commands::delete_backup_set,
            commands::get_backup_runs,
            commands::get_backup_snapshots,
            commands::validate_backup_destination,
            commands::plan_backup,
            commands::start_backup,
            commands::cancel_backup,
            commands::get_backup_progress,
            commands::verify_backup,
            commands::get_backup_health,
            commands::restore_from_backup,
            commands::get_restore_preview,
            commands::retry_failed_backup_items,
            commands::list_backup_destinations,
            commands::check_backup_destination_safety,
            commands::check_file_backup_status,
            commands::get_file_backup_status,
            commands::detect_interrupted_backups,
            commands::resume_backup_run,
            commands::check_scheduled_backups,
            commands::export_backup_set,
            commands::import_backup_set,
            commands::discover_backup_destinations,
            commands::get_volume_identity,
            commands::reassociate_backup_set,
            commands::get_backup_logs,
            commands::clear_backup_cache,
            commands::verify_backup_deep,
            commands::list_test_scenarios,
            commands::get_default_test_data_dir,
            commands::generate_test_data,
            commands::verify_test_data,
            commands::clean_test_data,
            commands::list_tracked_test_datasets,
            commands::analyze_file_health,
            commands::scan_and_compare_recovery_pairs,
            commands::scan_triage_plan,
            commands::execute_triage_moves,
            commands::get_duplicate_explanation,
            commands::scan_extension_rectifications,
            commands::execute_extension_rectifications,
            commands::scan_perceptual_duplicates,
            commands::plan_file_reconstruction,
            commands::execute_file_reconstruction,
            commands::get_privacy_audit,
            commands::scan_document_lineages,
            commands::scan_carving_candidates,
            commands::execute_carving_renames,
            commands::run_harbor_patrol,
            commands::open_file,
            commands::show_in_folder
        ])
.run(tauri::generate_context!())
        .expect("error while running FileHarbor");
}

