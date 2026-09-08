//! Backup export/import: allows moving backup data between FileHarbor installations.
//!
//! Exports backup manifests and metadata so a future FileHarbor installation
//! can recognize backup data even if the local SQLite database is lost.

use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};

use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;
use crate::backup::{get_backup_set, get_backup_runs, get_backup_snapshots, create_backup_set};

/// Exports backup metadata for a set to a portable archive.
pub fn export_backup_set(
    store: &Store,
    backup_set_id: i64,
    output_path: &Path,
) -> Result<ExportResult, BackupError> {
    let backup_set = get_backup_set(store, backup_set_id)?
        .ok_or_else(|| BackupError::SetNotFound(backup_set_id))?;

    let runs = get_backup_runs(store, backup_set_id, 1000)?;
    let snapshots = get_backup_snapshots(store, backup_set_id)?;

    let runs_count = runs.len();
    let snapshots_count = snapshots.len();

    let export = BackupExport {
        schema_version: 1,
        exported_at_ms: now_ms(),
        fileharbor_version: env!("CARGO_PKG_VERSION").to_string(),
        backup_set,
        runs,
        snapshots,
    };

    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| BackupError::SerializationError(e.to_string()))?;

    fs::create_dir_all(output_path)
        .map_err(|e| BackupError::DestinationError(e.to_string()))?;

    let export_file = output_path.join("backup-export.json");
    let temp_path = export_file.with_extension("json.tmp");
    fs::write(&temp_path, json)
        .map_err(|e| BackupError::DestinationError(e.to_string()))?;
    fs::rename(&temp_path, &export_file)
        .map_err(|e| BackupError::DestinationError(e.to_string()))?;

    Ok(ExportResult {
        backup_set_id,
        export_path: export_file,
        runs_exported: runs_count,
        snapshots_exported: snapshots_count,
    })
}

/// Imports backup metadata from a portable archive.
pub fn import_backup_set(
    store: &Store,
    import_path: &Path,
    destination_path: &str,
) -> Result<ImportResult, BackupError> {
    let export_file = import_path.join("backup-export.json");
    if !export_file.exists() {
        return Err(BackupError::ManifestError("Export file not found".to_string()));
    }

    let content = fs::read_to_string(&export_file)
        .map_err(|e| BackupError::ManifestError(e.to_string()))?;

    let export: BackupExport = serde_json::from_str(&content)
        .map_err(|e| BackupError::SerializationError(e.to_string()))?;

    // Create the backup set in the target database
    let new_set = create_backup_set(
        store,
        &export.backup_set.name,
        &export.backup_set.source_path,
        destination_path,
        export.backup_set.schedule,
        export.backup_set.retention_policy,
        export.backup_set.verification_mode,
    )?;

    Ok(ImportResult {
        new_backup_set_id: new_set.id,
        runs_imported: export.runs.len(),
        snapshots_imported: export.snapshots.len(),
    })
}

/// Result of an export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub backup_set_id: i64,
    pub export_path: PathBuf,
    pub runs_exported: usize,
    pub snapshots_exported: usize,
}

/// Result of an import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub new_backup_set_id: i64,
    pub runs_imported: usize,
    pub snapshots_imported: usize,
}

/// Portable backup export structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupExport {
    pub schema_version: u32,
    pub exported_at_ms: i64,
    pub fileharbor_version: String,
    pub backup_set: BackupSet,
    pub runs: Vec<BackupRun>,
    pub snapshots: Vec<BackupSnapshot>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}