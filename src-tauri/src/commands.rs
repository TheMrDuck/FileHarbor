//! Tauri command layer: thin wrappers that translate UI requests into
//! organizer-core service calls and emit progress events to the window.

use std::path::{Path, PathBuf};
use std::thread;

use organizer_core::{
    cancel::CancelToken,
    deleter,
    models::*,
    quarantine,
    restore::{self, RestoreResolution, RestorePreview},
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
pub fn restore_preview(state: State<AppState>, item_ids: Vec<i64>) -> Result<Vec<RestorePreview>, String> {
    let store = state.store.lock().map_err(|_| "lock error")?;
    restore::preview_restore(&store, &item_ids)
}

#[derive(Serialize, Clone)]
pub struct RestoreResult {
    pub report: BatchReport,
    pub kept_existing: Vec<String>,
}

#[tauri::command]
pub fn restore_execute(
    state: State<AppState>,
    item_ids: Vec<i64>,
    resolution: Option<String>,
) -> Result<RestoreResult, String> {
    let resolution = resolution.map(|r| match r.as_str() {
        "keep_existing" => RestoreResolution::KeepExisting,
        "new_name" => RestoreResolution::RestoreWithNewName,
        _ => RestoreResolution::KeepExisting,
    });
    let store = state.store.lock().map_err(|_| "lock error")?;
    let result = restore::restore_items(&store, &item_ids, resolution)?;
    Ok(RestoreResult { report: result.report, kept_existing: result.kept_existing })
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
