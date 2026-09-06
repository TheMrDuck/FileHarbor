//! Tauri command layer: thin wrappers that translate UI requests into
//! organizer-core service calls and emit progress events to the window.

use std::path::{Path, PathBuf};
use std::thread;

use organizer_core::{
    backup::{self, BackupError, BackupSet, BackupRun, BackupSnapshot, BackupDestination, BackupSchedule, RetentionPolicy, VerificationMode, BackupPlan, BackupProgress, BackupHealth, BackupRunSummary, RestoreConflictPolicy, DestinationValidation, BackupRunStatus, RestoreResult as BackupRestoreResult},
    cancel::CancelToken,
    deleter,
    models::*,
    quarantine,
    restore::{self, RestoreResolution},
    scanner::{self, ScanProgress, ScannerConfig},
    settings::AppSettings,
    store::{FileFilter, Store},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::{AppState, ScanEntry};

fn with_store<R>(
    state: &State<AppState>,
    f: impl FnOnce(&Store) -> Result<R, String>,
) -> Result<R, String> {
    let guard = state.store.lock().map_err(|_| "internal lock error")?;
    f(&guard)
}

/// Convert BackupError to String for Tauri commands
fn backup_err(e: BackupError) -> String {
    e.user_message()
}

/// Helper for backup operations that return BackupError
fn with_store_backup<R>(
    state: &State<AppState>,
    f: impl FnOnce(&Store) -> Result<R, BackupError>,
) -> Result<R, String> {
    let guard = state.store.lock().map_err(|_| "internal lock error")?;
    f(&guard).map_err(backup_err)
}

// ----------------------------------------------------------------------
// Scanning
// ----------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ScanRequest {
    pub roots: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct ScanStarted {
    pub session_id: i64,
}

#[tauri::command]
pub fn start_scan(
    app: AppHandle,
    state: State<AppState>,
    request: ScanRequest,
) -> Result<ScanStarted, String> {
    let roots: Vec<PathBuf> = request
        .roots
        .iter()
        .filter_map(|r| {
            let p = PathBuf::from(r);
            if p.is_absolute() && p.exists() {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    if roots.is_empty() {
        return Err("please select at least one existing folder".into());
    }

    let root_strings: Vec<String> = roots.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let session_id = with_store(&state, |s| s.insert_session(&root_strings))?;

    let settings = AppSettings::load(&state.store.lock().unwrap());
    let exclusions: Vec<PathBuf> = with_store(&state, |s| s.list_exclusions())?
        .into_iter()
        .map(|(_, path)| PathBuf::from(path))
        .collect();

    let cfg = ScannerConfig {
        roots: roots.clone(),
        exclusions,
        quarantine_root: Some(state.quarantine_root.clone()),
        max_hash_concurrency: settings.max_hash_concurrency,
    };

    let cancel = CancelToken::new();
    state
        .scans
        .lock()
        .unwrap()
        .insert(session_id, ScanEntry { cancel: cancel.clone() });

    let app_handle = app.clone();
    let scan_cancel = cancel.clone();
    thread::spawn(move || {
        // The background thread opens its own SQLite connection.
        let app_data_dir = app_handle.path().app_data_dir().expect("app data dir");
        let db_path = app_data_dir.join("file-organizer.db");
        let store = match Store::open(&db_path) {
            Ok(s) => s,
            Err(e) => {
                log_scan_error(&app_handle, session_id, &e);
                return;
            }
        };
        let emit_progress = |p: &ScanProgress| {
            let _ = app_handle.emit("scan-progress", p);
        };
        let status = scanner::run_scan(&store, &cfg, session_id, &scan_cancel, &emit_progress);
        if let Ok(summary) = store.scan_summary(session_id) {
            let _ = app_handle.emit("scan-finished", serde_json::json!({
                "session_id": session_id,
                "status": status.as_str(),
                "summary": summary,
            }));
        }
    });

    Ok(ScanStarted { session_id })
}

fn log_scan_error(app: &AppHandle, session_id: i64, e: &str) {
    let _ = app.emit(
        "scan-finished",
        serde_json::json!({ "session_id": session_id, "status": "failed", "error": e }),
    );
}

#[tauri::command]
pub fn cancel_scan(state: State<AppState>, session_id: i64) -> Result<(), String> {
    if let Some(entry) = state.scans.lock().unwrap().get(&session_id) {
        entry.cancel.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn get_recent_sessions(state: State<AppState>) -> Result<Vec<ScanSession>, String> {
    with_store(&state, |s| s.list_sessions(20))
}

#[tauri::command]
pub fn get_scan_summary(state: State<AppState>, session_id: i64) -> Result<ScanSummary, String> {
    with_store(&state, |s| s.scan_summary(session_id))
}

#[derive(Deserialize)]
pub struct ListFilesRequest {
    pub session_id: i64,
    pub search: Option<String>,
    pub extension: Option<String>,
    #[serde(default)]
    pub problem_only: bool,
    #[serde(default)]
    pub empty_only: bool,
    pub sort: Option<String>,
    pub ascending: Option<bool>,
    pub page: Option<u64>,
    pub page_size: Option<usize>,
}

#[tauri::command]
pub fn list_files(state: State<AppState>, request: ListFilesRequest) -> Result<serde_json::Value, String> {
    let page_size = request.page_size.unwrap_or(100).clamp(10, 500);
    let offset = request.page.unwrap_or(0).saturating_mul(page_size as u64) as usize;

    if request.problem_only {
        let fp = with_store(&state, |s| s.problem_files(request.session_id, page_size, offset))?;
        return Ok(serde_json::json!({ "items": fp.items, "total": fp.total }));
    }

    let filter = FileFilter {
        search: request.search,
        extension: request.extension,
        scan_status: None,
        empty_only: request.empty_only,
        min_size: None,
        max_size: None,
        sort: request.sort,
        ascending: request.ascending.unwrap_or(true),
        limit: page_size,
        offset,
    };
    let fp = with_store(&state, |s| s.list_files(request.session_id, &filter))?;
    Ok(serde_json::json!({ "items": fp.items, "total": fp.total }))
}

#[derive(Deserialize)]
pub struct GetFileBackupStatusRequest {
    pub session_id: i64,
    pub file_ids: Vec<i64>,
}

#[tauri::command]
pub fn get_file_backup_status(
    state: State<AppState>,
    request: GetFileBackupStatusRequest,
) -> Result<Vec<organizer_core::backup::FileBackupStatus>, String> {
    with_store_backup(&state, |s| {
        backup::compute_file_backup_status(s, request.session_id, &request.file_ids)
    })
}

#[tauri::command]
pub fn get_problem_files(state: State<AppState>, session_id: i64, page: Option<u64>) -> Result<serde_json::Value, String> {
    let page_size = 100;
    let offset = page.unwrap_or(0).saturating_mul(page_size as u64) as usize;
    let fp = with_store(&state, |s| s.problem_files(session_id, page_size, offset))?;
    Ok(serde_json::json!({ "items": fp.items, "total": fp.total }))
}

#[tauri::command]
pub fn get_duplicate_groups(
    state: State<AppState>,
    session_id: i64,
    page: Option<u64>,
) -> Result<serde_json::Value, String> {
    let page_size = 25;
    let offset = page.unwrap_or(0).saturating_mul(page_size as u64) as usize;
    let (groups, total) =
        with_store(&state, |s| s.duplicate_groups_page(session_id, page_size, offset))?;
    Ok(serde_json::json!({ "items": groups, "total": total }))
}

/// Re-run candidate finding + hashing + grouping without a filesystem
/// walk. Used by "Retry analysis" after failures or manual fixes.
#[tauri::command]
pub fn rebuild_analysis(
    state: State<AppState>,
    session_id: i64,
) -> Result<(), String> {
    let cfg = ScannerConfig::default();
    let cancel = CancelToken::new();
    let store = state.store.lock().map_err(|_| "lock error")?;
    scanner::run_analysis(&store, &cfg, session_id, &cancel, &|_| {})
        .map_err(|e| match e {
            scanner::AnalysisError::Cancelled => "cancelled".to_string(),
            scanner::AnalysisError::Fatal(msg) => msg,
        })
}

// ----------------------------------------------------------------------
// Quarantine / restore / delete
// ----------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct QuarantineResult {
    pub report: BatchReport,
}

#[tauri::command]
pub fn quarantine_files(
    state: State<AppState>,
    file_ids: Vec<i64>,
) -> Result<QuarantineResult, String> {
    let cancel = CancelToken::new();
    let result = {
        let store = state.store.lock().map_err(|_| "lock error")?;
        quarantine::quarantine_files(&store, &file_ids, &state.quarantine_root, &cancel)?
    };
    Ok(QuarantineResult { report: result.report })
}

#[tauri::command]
pub fn get_quarantine_items(state: State<AppState>) -> Result<Vec<QuarantineItem>, String> {
    with_store(&state, |s| s.get_quarantine_items(false))
}

#[tauri::command]
pub fn restore_preview(state: State<AppState>, item_ids: Vec<i64>) -> Result<Vec<organizer_core::restore::RestorePreview>, String> {
    let store = state.store.lock().map_err(|_| "lock error")?;
    restore::preview_restore(&store, &item_ids)
}

#[derive(Serialize, Clone)]
pub struct RestoreResultLocal {
    pub report: BatchReport,
    pub kept_existing: Vec<String>,
}

#[tauri::command]
pub fn restore_execute(
    state: State<AppState>,
    item_ids: Vec<i64>,
    resolution: Option<String>,
) -> Result<RestoreResultLocal, String> {
    let resolution = resolution.map(|r| match r.as_str() {
        "keep_existing" => RestoreResolution::KeepExisting,
        "new_name" => RestoreResolution::RestoreWithNewName,
        _ => RestoreResolution::KeepExisting,
    });
    let store = state.store.lock().map_err(|_| "lock error")?;
    let result = restore::restore_items(&store, &item_ids, resolution)?;
    Ok(RestoreResultLocal { report: result.report, kept_existing: result.kept_existing })
}

#[tauri::command]
pub fn delete_permanently(state: State<AppState>, item_ids: Vec<i64>) -> Result<BatchReport, String> {
    let store = state.store.lock().map_err(|_| "lock error")?;
    deleter::delete_permanently(&store, &item_ids)
}

// ----------------------------------------------------------------------
// Operations history
// ----------------------------------------------------------------------

#[tauri::command]
pub fn get_operations(state: State<AppState>, page: Option<u64>) -> Result<Vec<FileOperation>, String> {
    let offset = page.unwrap_or(0).saturating_mul(50) as usize;
    with_store(&state, |s| s.list_operations(50, offset))
}

#[tauri::command]
pub fn get_operation_items(state: State<AppState>, operation_id: i64) -> Result<Vec<OperationItem>, String> {
    with_store(&state, |s| s.operation_items(operation_id))
}

// ----------------------------------------------------------------------
// Exclusions + settings
// ----------------------------------------------------------------------

#[tauri::command]
pub fn list_exclusions(state: State<AppState>) -> Result<Vec<(i64, String)>, String> {
    with_store(&state, |s| s.list_exclusions())
}

#[tauri::command]
pub fn add_exclusion(state: State<AppState>, path: String) -> Result<i64, String> {
    let normalized = organizer_core::fs_layer::normalize_exclusion(Path::new(&path));
    if normalized.is_empty() {
        return Err("empty path".into());
    }
    with_store(&state, |s| s.add_exclusion(&normalized))
}

#[tauri::command]
pub fn remove_exclusion(state: State<AppState>, id: i64) -> Result<(), String> {
    with_store(&state, |s| s.remove_exclusion(id))
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let store = state.store.lock().map_err(|_| "lock error")?;
    Ok(AppSettings::load(&store))
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: AppSettings) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "lock error")?;
    settings.save(&store)
}

// ----------------------------------------------------------------------
// Backup
// ----------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateBackupSetRequest {
    pub name: String,
    pub source_path: String,
    pub destination_path: String,
    pub schedule: Option<String>,
    pub retention_policy: Option<String>,
    pub verification_mode: Option<String>,
}

#[tauri::command]
pub fn create_backup_set(
    state: State<AppState>,
    request: CreateBackupSetRequest,
) -> Result<BackupSet, String> {
    let schedule = request.schedule.as_deref().unwrap_or("manual").parse().unwrap_or(BackupSchedule::Manual);
    let retention = request.retention_policy.as_deref().unwrap_or("keep_all").parse().unwrap_or(RetentionPolicy::KeepAll);
    let verification = request.verification_mode.as_deref().unwrap_or("standard").parse().unwrap_or(VerificationMode::Standard);

    with_store_backup(&state, |s| {
        backup::create_backup_set(s, &request.name, &request.source_path, &request.destination_path, schedule, retention, verification)
    })
}

#[tauri::command]
pub fn list_backup_sets(state: State<AppState>) -> Result<Vec<BackupSet>, String> {
    with_store_backup(&state, |s| backup::list_backup_sets(s))
}

#[tauri::command]
pub fn get_backup_set(state: State<AppState>, set_id: i64) -> Result<Option<BackupSet>, String> {
    with_store_backup(&state, |s| backup::get_backup_set(s, set_id))
}

#[derive(Deserialize)]
pub struct UpdateBackupSetRequest {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub retention_policy: Option<String>,
    pub verification_mode: Option<String>,
}

#[tauri::command]
pub fn update_backup_set(
    state: State<AppState>,
    set_id: i64,
    request: UpdateBackupSetRequest,
) -> Result<BackupSet, String> {
    let schedule = request.schedule.map(|s| s.parse().unwrap_or(BackupSchedule::Manual));
    let retention = request.retention_policy.map(|s| s.parse().unwrap_or(RetentionPolicy::KeepAll));
    let verification = request.verification_mode.map(|s| s.parse().unwrap_or(VerificationMode::Standard));

    with_store_backup(&state, |s| backup::update_backup_set(s, set_id, request.name.as_deref(), schedule, retention, verification))
}

#[derive(Deserialize)]
pub struct DeleteBackupSetRequest {
    pub delete_data: Option<bool>,
}

#[tauri::command]
pub fn delete_backup_set(
    state: State<AppState>,
    set_id: i64,
    request: DeleteBackupSetRequest,
) -> Result<(), String> {
    with_store_backup(&state, |s| backup::delete_backup_set(s, set_id, request.delete_data.unwrap_or(false)))
}

#[tauri::command]
pub fn get_backup_runs(state: State<AppState>, backup_set_id: i64, limit: Option<usize>) -> Result<Vec<BackupRun>, String> {
    with_store_backup(&state, |s| backup::get_backup_runs(s, backup_set_id, limit.unwrap_or(20)))
}

#[tauri::command]
pub fn get_backup_snapshots(state: State<AppState>, backup_set_id: i64) -> Result<Vec<BackupSnapshot>, String> {
    with_store_backup(&state, |s| backup::get_backup_snapshots(s, backup_set_id))
}

#[tauri::command]
pub fn validate_backup_destination(
    _state: State<AppState>,
    source_path: String,
    destination_path: String,
) -> Result<DestinationValidation, String> {
    use std::path::Path;
    let validation = backup::destination::validate_destination(Path::new(&source_path), Path::new(&destination_path));
    Ok(validation)
}

#[derive(Deserialize)]
pub struct PlanBackupRequest {
    pub backup_set_id: i64,
}

#[tauri::command]
pub fn plan_backup(state: State<AppState>, request: PlanBackupRequest) -> Result<BackupPlan, String> {
    with_store_backup(&state, |s| {
        let _backup_set = backup::get_backup_set(s, request.backup_set_id)?.ok_or("Backup set not found")?;
        // For planning, we need to get the latest scan session for the source path
        // This is a simplified version - in production you'd query the latest session for the source
        let session_id = s.list_sessions(1).ok().and_then(|sessions| sessions.first().map(|s| s.id));
        if let Some(sid) = session_id {
            let source_files = s.list_files(sid, &FileFilter { limit: 100000, ..Default::default() })?;
            backup::planner::compute_backup_plan(s, request.backup_set_id, &source_files.items)
        } else {
            Err("No scan session found for planning".into())
        }
    })
}

#[tauri::command]
pub fn start_backup(
    app: AppHandle,
    state: State<AppState>,
    backup_set_id: i64,
) -> Result<i64, String> {
    let (cancel, backup_set, destination) = with_store_backup(&state, |s| {
        let bs = backup::get_backup_set(s, backup_set_id)?.ok_or("Backup set not found")?;
        let dest = backup::destination::get_destination_by_id_string(s, &bs.destination_id.to_string())?.ok_or("Destination not found")?;
        Ok((CancelToken::new(), bs, dest))
    })?;

    // Create the run record first so we can return the run_id immediately
    let run_id = with_store_backup(&state, |s| {
        s.create_backup_run(backup_set.id)
    })?;

    let app_handle = app.clone();
    let app_handle2 = app.clone();
    let cancel_token = cancel.clone();

    thread::spawn(move || {
        let app_data_dir = app_handle.path().app_data_dir().expect("app data dir");
        let db_path = app_data_dir.join("file-organizer.db");
        let store = match Store::open(&db_path) {
            Ok(s) => s,
            Err(e) => {
                let _ = app_handle.emit("backup-progress", serde_json::json!({
                    "run_id": run_id,
                    "status": "failed",
                    "error": e,
                }));
                return;
            }
        };

        // Create engine with progress callback
        let engine = backup::engine::BackupEngine::new(&store)
            .with_cancel_token(cancel_token)
            .with_progress(move |progress| {
                let _ = app_handle2.emit("backup-progress", progress);
            });

        let result = engine.run_backup_with_existing_run(run_id, &backup_set, Path::new(&destination.path));
        
        if let Ok(summary) = result {
            let _ = app_handle.emit("backup-finished", serde_json::json!({
                "run_id": summary.run_id,
                "status": summary.status.as_str(),
                "summary": summary,
            }));
        } else if let Err(e) = result {
            let _ = app_handle.emit("backup-finished", serde_json::json!({
                "run_id": run_id,
                "status": "failed",
                "error": e.to_string(),
            }));
        }
    });

    Ok(run_id)
}

#[tauri::command]
pub fn cancel_backup(_state: State<AppState>, _run_id: i64) -> Result<(), String> {
    // Cancellation is handled via the CancelToken in the engine
    // This would need a map of run_id -> CancelToken in AppState
    Ok(())
}

#[tauri::command]
pub fn get_backup_progress(state: State<AppState>, run_id: i64) -> Result<Option<BackupProgress>, String> {
    // Progress is emitted via events; this could query DB for current state
    with_store(&state, |s| {
        // Get run info from database
        let run_info = s.get_backup_run_status(run_id)?;
        
        Ok(run_info.map(|run_info| {
            BackupProgress {
                run_id,
                status: BackupRunStatus::parse(&run_info.0),
                current_file: None,
                files_processed: 0,
                files_total: 0,
                bytes_transferred: 0,
                bytes_total: 0,
                bytes_verified: 0,
                files_verified: 0,
                files_failed: 0,
                speed_bytes_per_sec: 0.0,
                eta_seconds: None,
            }
        }))
    })
}

#[tauri::command]
pub fn verify_backup(
    state: State<AppState>,
    run_id: i64,
) -> Result<backup::verifier::VerificationResult, String> {
    with_store_backup(&state, |s| {
        // Get destination for the backup set
        let backup_set_id = s.get_backup_set_id_from_run(run_id)?.ok_or("Run not found")?;
        let bs = backup::get_backup_set(s, backup_set_id)?.ok_or("Backup set not found")?;
        let dest = backup::destination::get_destination_by_id_string(s, &bs.destination_id.to_string())?.ok_or("Destination not found")?;
        
        backup::verifier::verify_backup_run(s, run_id, Path::new(&dest.path))
    })
}

#[tauri::command]
pub fn get_backup_health(state: State<AppState>) -> Result<BackupHealth, String> {
    with_store(&state, |s| backup::get_backup_health(s).map_err(backup_err))
}

#[derive(Deserialize)]
pub struct RestoreFromBackupRequest {
    pub snapshot_id: i64,
    pub destination_root: String,
    pub conflicts: Vec<(String, String)>, // (relative_path, policy)
}

#[tauri::command]
pub fn restore_from_backup(
    state: State<AppState>,
    request: RestoreFromBackupRequest,
) -> Result<BackupRestoreResult, String> {
    use std::path::Path;
    with_store(&state, |s| {
        let conflicts: Vec<(String, RestoreConflictPolicy)> = request.conflicts
            .into_iter()
            .map(|(path, policy)| (path, policy.parse().unwrap_or(RestoreConflictPolicy::Cancel)))
            .collect();
        
        backup::restore::restore_from_backup(s, request.snapshot_id, Path::new(&request.destination_root), &conflicts).map_err(backup_err)
    })
}

#[tauri::command]
pub fn get_restore_preview(
    state: State<AppState>,
    snapshot_id: i64,
    destination_root: String,
) -> Result<backup::restore::RestorePreview, String> {
    use std::path::Path;
    with_store(&state, |s| {
        backup::restore::get_restore_preview(s, snapshot_id, Path::new(&destination_root)).map_err(backup_err)
    })
}

#[tauri::command]
pub fn retry_failed_backup_items(
    state: State<AppState>,
    _run_id: i64,
) -> Result<(), String> {
    // This would re-run failed items from a previous run
    // Implementation would query failed items and re-run them
    with_store(&state, |_s| {
        // For now, just return OK - full implementation would be more complex
        Ok(())
    })
}

#[tauri::command]
pub fn list_backup_destinations(state: State<AppState>) -> Result<Vec<BackupDestination>, String> {
    with_store(&state, |s| backup::destination::list_destinations(s).map_err(backup_err))
}

#[tauri::command]
pub fn check_backup_destination_safety(
    path: String,
) -> Result<Option<String>, String> {
    use std::path::Path;
    Ok(backup::destination::recognize_backup_destination(Path::new(&path)))
}

#[tauri::command]
pub fn check_file_backup_status(
    state: State<AppState>,
    file_path: String,
) -> Result<Option<(String, String)>, String> {
    with_store_backup(&state, |s| backup::check_file_backup_status(s, &file_path))
}

#[tauri::command]
pub fn detect_interrupted_backups(
    state: State<AppState>,
) -> Result<Vec<BackupRun>, String> {
    with_store_backup(&state, |s| backup::detect_interrupted_backups(s))
}

#[tauri::command]
pub fn resume_backup_run(
    state: State<AppState>,
    run_id: i64,
) -> Result<BackupRunSummary, String> {
    use std::path::Path;
    with_store_backup(&state, |s| {
        // First, get the run to find its backup set
        let run = backup::get_backup_run(s, run_id)?.ok_or("Backup run not found")?;
        let backup_set = backup::get_backup_set(s, run.backup_set_id)?.ok_or("Backup set not found")?;
        let dest_path = backup::destination::get_destination_path(s, backup_set.destination_id)?;
        
        backup::resume_backup_run(s, run_id, Path::new(&dest_path))
    })
}
