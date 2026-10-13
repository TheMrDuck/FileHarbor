//! Backup module: local backup and protection system.

pub mod models;
pub mod errors;
pub mod destination;
pub mod planner;
pub mod verifier;
pub mod manifest;
pub mod engine;
pub mod restore;
pub mod retention;
pub mod scheduler;
pub mod cache;
pub mod discovery;
pub mod logger;
pub mod export_import;
pub mod machine;
pub mod patrol;

pub use errors::{BackupError, ErrorClassification};
pub use models::*;
pub use restore::{RestorePreview, RestoreConflict};

use crate::store::{Store, FILE_COLS, map_file};
#[allow(unused_imports)]
use crate::backup::models::*;
#[allow(unused_imports)]
use crate::backup::errors::*;
use crate::models::OpItemStatus;
use serde::{Deserialize, Serialize};
use rusqlite::OptionalExtension;
use std::f64;
use std::path::Path;

/// Creates a new backup set.
pub fn create_backup_set(
    store: &Store,
    name: &str,
    source_path: &str,
    destination_path: &str,
    schedule: BackupSchedule,
    retention_policy: RetentionPolicy,
    verification_mode: VerificationMode,
) -> Result<BackupSet, BackupError> {
    // Validate destination
    let validation = destination::validate_destination(
        std::path::Path::new(source_path),
        std::path::Path::new(destination_path),
    );

    if !validation.valid {
        return Err(BackupError::InvalidConfiguration(
            validation.errors.join(", ")
        ));
    }

    // Ensure destination identity
    let dest = destination::ensure_destination_identity(
        store,
        std::path::Path::new(destination_path),
        Some(name.to_string()),
    )?;

    let now = now_ms();
    store.conn.execute(
        "INSERT INTO backup_sets
         (name, source_path, destination_id, schedule, retention_policy, verification_mode, created_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            name,
            source_path,
            dest.id,
            schedule.as_str(),
            retention_policy.as_str(),
            verification_mode.as_str(),
            now,
            now,
        ],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let id = store.conn.last_insert_rowid();

    Ok(BackupSet {
        id,
        name: name.to_string(),
        source_path: source_path.to_string(),
        destination_id: dest.id,
        schedule,
        retention_policy,
        verification_mode,
        created_at_ms: now,
        updated_at_ms: now,
    })
}

/// Lists all backup sets.
pub fn list_backup_sets(store: &Store) -> Result<Vec<BackupSet>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, name, source_path, destination_id, schedule, retention_policy, verification_mode, created_at_ms, updated_at_ms
         FROM backup_sets ORDER BY created_at_ms DESC"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map([], |row| {
        let schedule_str: String = row.get(4)?;
        let retention_str: String = row.get(5)?;
        let verification_str: String = row.get(6)?;
        Ok(BackupSet {
            id: row.get(0)?,
            name: row.get(1)?,
            source_path: row.get(2)?,
            destination_id: row.get(3)?,
            schedule: BackupSchedule::parse(&schedule_str),
            retention_policy: RetentionPolicy::parse(&retention_str),
            verification_mode: VerificationMode::parse(&verification_str),
            created_at_ms: row.get(7)?,
            updated_at_ms: row.get(8)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Gets a backup set by ID.
pub fn get_backup_set(store: &Store, set_id: i64) -> Result<Option<BackupSet>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, name, source_path, destination_id, schedule, retention_policy, verification_mode, created_at_ms, updated_at_ms
         FROM backup_sets WHERE id = ?1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let set = stmt.query_row(rusqlite::params![set_id], |row| {
        let schedule_str: String = row.get(4)?;
        let retention_str: String = row.get(5)?;
        let verification_str: String = row.get(6)?;
        Ok(BackupSet {
            id: row.get(0)?,
            name: row.get(1)?,
            source_path: row.get(2)?,
            destination_id: row.get(3)?,
            schedule: BackupSchedule::parse(&schedule_str),
            retention_policy: RetentionPolicy::parse(&retention_str),
            verification_mode: VerificationMode::parse(&verification_str),
            created_at_ms: row.get(7)?,
            updated_at_ms: row.get(8)?,
        })
    }).optional().map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    Ok(set)
}

/// Updates a backup set.
pub fn update_backup_set(
    store: &Store,
    set_id: i64,
    name: Option<&str>,
    schedule: Option<BackupSchedule>,
    retention_policy: Option<RetentionPolicy>,
    verification_mode: Option<VerificationMode>,
) -> Result<BackupSet, BackupError> {
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(name) = name {
        updates.push("name = ?");
        params.push(Box::new(name.to_string()));
    }
    if let Some(schedule) = schedule {
        updates.push("schedule = ?");
        params.push(Box::new(schedule.as_str().to_string()));
    }
    if let Some(retention) = retention_policy {
        updates.push("retention_policy = ?");
        params.push(Box::new(retention.as_str()));
    }
    if let Some(verification) = verification_mode {
        updates.push("verification_mode = ?");
        params.push(Box::new(verification.as_str().to_string()));
    }

    if updates.is_empty() {
        return get_backup_set(store, set_id)?.ok_or(BackupError::SetNotFound(set_id));
    }

    updates.push("updated_at_ms = ?");
    params.push(Box::new(now_ms()));

    let sql = format!(
        "UPDATE backup_sets SET {} WHERE id = ?",
        updates.join(", ")
    );
    params.push(Box::new(set_id));

    store.conn.execute(
        &sql,
        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    get_backup_set(store, set_id)?.ok_or(BackupError::SetNotFound(set_id))
}

/// Deletes a backup set (configuration only, not data).
pub fn delete_backup_set(store: &Store, set_id: i64, delete_data: bool) -> Result<(), BackupError> {
    if delete_data {
        // Delete all snapshots, runs, files for this set
        let snapshots = get_snapshots_for_set(store, set_id)?;
        for snap in snapshots {
            crate::backup::retention::delete_snapshot(store, snap.id)?;
        }
        store.conn.execute(
            "DELETE FROM backup_runs WHERE backup_set_id = ?1",
            rusqlite::params![set_id],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    }

    store.conn.execute(
        "DELETE FROM backup_sets WHERE id = ?1",
        rusqlite::params![set_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Gets snapshots for a backup set.
fn get_snapshots_for_set(
    store: &Store,
    backup_set_id: i64,
) -> Result<Vec<BackupSnapshot>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes
         FROM backup_snapshots
         WHERE backup_set_id = ?1
         ORDER BY created_at_ms DESC"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map(rusqlite::params![backup_set_id], |row| {
        let status_str: String = row.get(4)?;
        Ok(BackupSnapshot {
            id: row.get(0)?,
            backup_set_id: row.get(1)?,
            run_id: row.get(2)?,
            label: row.get(3)?,
            status: BackupSnapshotStatus::parse(&status_str),
            created_at_ms: row.get(5)?,
            files_count: row.get(6)?,
            total_size_bytes: row.get(7)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Gets backup runs for a set.
pub fn get_backup_runs(store: &Store, backup_set_id: i64, limit: usize) -> Result<Vec<BackupRun>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, backup_set_id, snapshot_id, status, started_at_ms, finished_at_ms,
                files_total, files_new, files_changed, files_unchanged, files_deleted, files_failed,
                bytes_transferred, bytes_verified, error_summary
         FROM backup_runs WHERE backup_set_id = ?1 ORDER BY started_at_ms DESC LIMIT ?2"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map(rusqlite::params![backup_set_id, limit as i64], |row| {
        let status_str: String = row.get(3)?;
        Ok(BackupRun {
            id: row.get(0)?,
            backup_set_id: row.get(1)?,
            snapshot_id: row.get(2)?,
            status: BackupRunStatus::parse(&status_str),
            started_at_ms: row.get(4)?,
            finished_at_ms: row.get(5)?,
            files_total: row.get(6)?,
            files_new: row.get(7)?,
            files_changed: row.get(8)?,
            files_unchanged: row.get(9)?,
            files_deleted: row.get(10)?,
            files_failed: row.get(11)?,
            bytes_transferred: row.get(12)?,
            bytes_verified: row.get(13)?,
            error_summary: row.get(14)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Updates the status of a backup run.
pub fn update_backup_run_status(
    store: &Store,
    run_id: i64,
    status: BackupRunStatus,
    detail: Option<String>,
) -> Result<(), BackupError> {
    store.conn.execute(
        "UPDATE backup_runs SET status = ?1, finished_at_ms = ?2, error_summary = ?3 WHERE id = ?4",
        rusqlite::params![status.as_str(), now_ms(), detail, run_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    Ok(())
}

/// Gets backup snapshots for a set.
pub fn get_backup_snapshots(store: &Store, backup_set_id: i64) -> Result<Vec<BackupSnapshot>, BackupError> {
    get_snapshots_for_set(store, backup_set_id)
}

/// Gets all backup files for a snapshot.
pub fn get_snapshot_files(store: &Store, snapshot_id: i64) -> Result<Vec<BackupFile>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, snapshot_id, relative_path, size_bytes, sha256, status, content_id, error
         FROM backup_files WHERE snapshot_id = ?1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map(rusqlite::params![snapshot_id], |row| {
        Ok(BackupFile {
            id: row.get(0)?,
            snapshot_id: row.get(1)?,
            relative_path: row.get(2)?,
            size_bytes: row.get(3)?,
            sha256: row.get(4)?,
            status: BackupFileStatus::parse(&row.get::<_, String>(5)?),
            content_id: row.get(6)?,
            error: row.get(7)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Detects interrupted backup runs that were not completed.
/// Returns a list of runs with status "running" that were not properly finished.
pub fn detect_interrupted_backups(store: &Store) -> Result<Vec<BackupRun>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, backup_set_id, snapshot_id, status, started_at_ms, finished_at_ms,
                files_total, files_new, files_changed, files_unchanged, files_deleted, files_failed,
                bytes_transferred, bytes_verified, error_summary
         FROM backup_runs
         WHERE status = 'running'
         ORDER BY started_at_ms DESC"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map([], |row| {
        Ok(BackupRun {
            id: row.get(0)?,
            backup_set_id: row.get(1)?,
            snapshot_id: row.get(2)?,
            status: BackupRunStatus::parse(&row.get::<_, String>(3)?),
            started_at_ms: row.get(4)?,
            finished_at_ms: row.get(5)?,
            files_total: row.get(6)?,
            files_new: row.get(7)?,
            files_changed: row.get(8)?,
            files_unchanged: row.get(9)?,
            files_deleted: row.get(10)?,
            files_failed: row.get(11)?,
            bytes_transferred: row.get(12)?,
            bytes_verified: row.get(13)?,
            error_summary: row.get(14)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Gets a backup run by ID.
pub fn get_backup_run(store: &Store, run_id: i64) -> Result<Option<BackupRun>, BackupError> {
    store.conn.query_row(
        "SELECT id, backup_set_id, snapshot_id, status, started_at_ms, finished_at_ms,
                files_total, files_new, files_changed, files_unchanged, files_deleted, files_failed,
                bytes_transferred, bytes_verified, error_summary
         FROM backup_runs WHERE id = ?1",
        rusqlite::params![run_id],
        |row| {
            Ok(BackupRun {
                id: row.get(0)?,
                backup_set_id: row.get(1)?,
                snapshot_id: row.get(2)?,
                status: BackupRunStatus::parse(&row.get::<_, String>(3)?),
                started_at_ms: row.get(4)?,
                finished_at_ms: row.get(5)?,
                files_total: row.get(6)?,
                files_new: row.get(7)?,
                files_changed: row.get(8)?,
                files_unchanged: row.get(9)?,
                files_deleted: row.get(10)?,
                files_failed: row.get(11)?,
                bytes_transferred: row.get(12)?,
                bytes_verified: row.get(13)?,
                error_summary: row.get(14)?,
            })
        },
    ).optional().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Resumes an interrupted backup run.
pub fn resume_backup_run(
    store: &Store,
    run_id: i64,
    destination_root: &Path,
) -> Result<BackupRunSummary, BackupError> {
    let run = get_backup_run(store, run_id)?
        .ok_or(BackupError::RunNotFound(run_id))?;
    
    if run.status != BackupRunStatus::Running {
        return Err(BackupError::InvalidConfiguration(
            "Cannot resume a run that is not in 'running' state".to_string()
        ));
    }
    
    let backup_set = get_backup_set(store, run.backup_set_id)?
        .ok_or(BackupError::SetNotFound(run.backup_set_id))?;
    
    // Create a new engine and run the backup
    let engine = crate::backup::engine::BackupEngine::with_store(store);
    
    // Mark the old run as partial (we're starting fresh)
    update_backup_run_status(store, run_id, BackupRunStatus::Partial, Some("Resumed as new run".to_string()))?;
    
    // Start a new run
    engine.run_backup(&backup_set, destination_root)
}

pub fn get_backup_health(store: &Store) -> Result<BackupHealth, BackupError> {
    let sets = list_backup_sets(store)?;
    let mut all_protected = 0u64;
    let mut all_outdated = 0u64;
    let mut all_never = 0u64;
    let mut set_healths = Vec::new();

    for set in sets {
        let (protected, outdated, never) = compute_set_protection_status(store, &set)?;
        
        let total = protected + outdated + never;
        let percentage = if total > 0 {
            (protected as f64 / total as f64) * 100.0
        } else {
            100.0 // 100% if no files
        };

        let status = if never > 0 {
            BackupSetStatus::AtRisk
        } else if outdated > 0 {
            BackupSetStatus::Outdated
        } else {
            BackupSetStatus::Active
        };

        // Get last verified snapshot time
        let last_verified = get_last_verified_snapshot_time(store, set.id)?;

        set_healths.push(BackupSetHealth {
            backup_set_id: set.id,
            name: set.name,
            status,
            last_verified_at_ms: last_verified,
            protected_files: protected,
            outdated_files: outdated,
            never_backed_up_files: never,
            protection_percentage: percentage,
        });

        all_protected += protected;
        all_outdated += outdated;
        all_never += never;
    }

    let total = all_protected + all_outdated + all_never;
    let percentage = if total > 0 {
        (all_protected as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    Ok(BackupHealth {
        total_files: total,
        protected_files: all_protected,
        outdated_files: all_outdated,
        never_backed_up_files: all_never,
        protection_percentage: percentage,
        sets: set_healths,
    })
}

/// Computes protection status for a backup set by comparing latest scan with latest backup.
fn compute_set_protection_status(
    store: &Store,
    set: &BackupSet,
) -> Result<(u64, u64, u64), BackupError> {
    // Find latest completed scan session for the source path
    let source_path = &set.source_path;
    let session_id = store.find_latest_session_for_path(source_path)
        .map_err(BackupError::DatabaseError)?;
    
    let Some(session_id) = session_id else {
        // No scan session - can't determine protection
        return Ok((0, 0, 0));
    };

    // Get all files from the scan session
    let scan_files = store.get_session_files(session_id)
        .map_err(BackupError::DatabaseError)?;

    // Get latest verified snapshot for this backup set
    let snapshot = get_latest_verified_snapshot(store, set.id)?;
    
    let Some(snapshot) = snapshot else {
        // No verified snapshot - all files are never backed up
        return Ok((0, 0, scan_files.len() as u64));
    };

    // Get backup files from the snapshot
    let backup_files = get_snapshot_files(store, snapshot.id)?;

    // Create a map of backup files by relative path for quick lookup
    let backup_map: std::collections::HashMap<_, _> = backup_files
        .into_iter()
        .map(|bf| (bf.relative_path.clone(), bf))
        .collect();

    let mut protected = 0u64;
    let mut outdated = 0u64;
    let mut never = 0u64;

    for scan_file in scan_files {
        // Get relative path from source path
        let source_path_buf = std::path::Path::new(source_path);
        let file_path_buf = std::path::Path::new(&scan_file.path);
        let relative = file_path_buf.strip_prefix(source_path_buf)
            .unwrap_or(file_path_buf)
            .to_string_lossy()
            .to_string();

        match backup_map.get(&relative) {
            Some(backup_file) => {
                // Check if file has changed (by comparing hash if available, or size/modified time)
                if let Some(scan_hash) = &scan_file.hash {
                    if scan_hash == &backup_file.sha256 {
                        protected += 1;
                    } else {
                        outdated += 1;
                    }
                } else if scan_file.size_bytes == backup_file.size_bytes {
                    // No hash available, check size
                    protected += 1;
                } else {
                    outdated += 1;
                }
            }
            None => {
                never += 1;
            }
        }
    }

    Ok((protected, outdated, never))
}

/// Gets the latest verified snapshot for a backup set.
fn get_latest_verified_snapshot(
    store: &Store,
    backup_set_id: i64,
) -> Result<Option<BackupSnapshot>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes
         FROM backup_snapshots 
         WHERE backup_set_id = ?1 AND status = 'verified'
         ORDER BY created_at_ms DESC LIMIT 1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let mut rows = stmt.query_map(rusqlite::params![backup_set_id], |row| {
        let status_str: String = row.get(4)?;
        Ok(BackupSnapshot {
            id: row.get(0)?,
            backup_set_id: row.get(1)?,
            run_id: row.get(2)?,
            label: row.get(3)?,
            status: BackupSnapshotStatus::parse(&status_str),
            created_at_ms: row.get(5)?,
            files_count: row.get(6)?,
            total_size_bytes: row.get(7)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let snapshot = rows.next().transpose()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    
    Ok(snapshot)
}

/// Gets the creation time of the latest verified snapshot.
fn get_last_verified_snapshot_time(
    store: &Store,
    backup_set_id: i64,
) -> Result<Option<i64>, BackupError> {
    let snapshot = get_latest_verified_snapshot(store, backup_set_id)?;
    Ok(snapshot.map(|s| s.created_at_ms))
}

/// Checks if a file (by path) has a verified backup in any backup set.
/// Returns the backup set name and snapshot label if found.
pub fn check_file_backup_status(
    store: &Store,
    file_path: &str,
) -> Result<Option<(String, String)>, BackupError> {
    let backup_sets = list_backup_sets(store)?;
    
    for set in backup_sets {
        if let Some(snapshot) = get_latest_verified_snapshot(store, set.id)? {
            let backup_files = get_snapshot_files(store, snapshot.id)?;
            // Check if file exists in this snapshot
            // We need to find the relative path from the backup set's source path
            let source_path_buf = std::path::Path::new(&set.source_path);
            let file_path_buf = std::path::Path::new(file_path);
            if let Ok(relative) = file_path_buf.strip_prefix(source_path_buf) {
                let relative_str = relative.to_string_lossy().to_string();
                for bf in backup_files {
                    if bf.relative_path == relative_str {
                        return Ok(Some((set.name.clone(), snapshot.label.clone())));
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Computes backup protection status for specific files in a scan session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackupStatus {
    pub file_id: i64,
    pub status: String,
    pub last_verified_at_ms: Option<i64>,
    pub backup_set_id: Option<i64>,
    pub backup_set_name: Option<String>,
    pub snapshot_id: Option<i64>,
    pub snapshot_label: Option<String>,
}

/// Returns a vector of status objects matching the input file_ids order.
pub fn compute_file_backup_status(
    store: &Store,
    session_id: i64,
    file_ids: &[i64],
) -> Result<Vec<FileBackupStatus>, BackupError> {
    // Find the latest completed scan session for the source path
    // For now, we'll use the provided session_id directly since we're checking specific files
    
pub type BackupSnapshotFileMap = (i64, String, i64, String, i64, std::collections::HashMap<String, BackupFile>);

    // Get all backup sets
    let backup_sets = list_backup_sets(store)?;
    
    // For each backup set, get the latest verified snapshot and its files
    let mut backup_file_maps: Vec<BackupSnapshotFileMap> = Vec::new();
    
    for set in &backup_sets {
        if let Some(snapshot) = get_latest_verified_snapshot(store, set.id)? {
            let backup_files = get_snapshot_files(store, snapshot.id)?;
            let file_map: std::collections::HashMap<String, BackupFile> = backup_files
                .into_iter()
                .map(|bf| (bf.relative_path.clone(), bf))
                .collect();
            backup_file_maps.push((set.id, set.name.clone(), snapshot.id, snapshot.label.clone(), snapshot.created_at_ms, file_map));
        }
    }
    
    // Get the scan files for the requested file_ids
    let placeholders = file_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT {FILE_COLS} FROM file_records WHERE id IN ({})",
        placeholders
    );
    
    let mut stmt = store.conn.prepare(&sql).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    let params: Vec<&dyn rusqlite::ToSql> = file_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter().cloned()), |row| {
        map_file(row)
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    
    let mut scan_files: std::collections::HashMap<i64, crate::models::FileRecord> = std::collections::HashMap::new();
    for row in rows {
        let file = row.map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        scan_files.insert(file.id, file);
    }
    
    // For each requested file, compute its backup status
    let mut results = Vec::new();
    
    for file_id in file_ids {
        let Some(scan_file) = scan_files.get(file_id) else {
            results.push(FileBackupStatus {
                file_id: *file_id,
                status: "error".to_string(),
                last_verified_at_ms: None,
                backup_set_id: None,
                backup_set_name: None,
                snapshot_id: None,
                snapshot_label: None,
            });
            continue;
        };
        
        // Get relative path from session roots
        let session = store.get_session(session_id).map_err(BackupError::DatabaseError)?;
        let roots: Vec<String> = session.roots;
        
        let mut relative_path = None;
        for root in &roots {
            let root_path = std::path::Path::new(root);
            let file_path = std::path::Path::new(&scan_file.path);
            if let Ok(rel) = file_path.strip_prefix(root_path) {
                relative_path = Some(rel.to_string_lossy().to_string());
                break;
            }
        }
        
        let Some(rel_path) = relative_path else {
            results.push(FileBackupStatus {
                file_id: *file_id,
                status: "error".to_string(),
                last_verified_at_ms: None,
                backup_set_id: None,
                backup_set_name: None,
                snapshot_id: None,
                snapshot_label: None,
            });
            continue;
        };
        
        // Check each backup set for this file
        let mut found = false;
        for (set_id, set_name, snap_id, snap_label, snap_created_at, file_map) in &backup_file_maps {
            if let Some(backup_file) = file_map.get(&rel_path) {
                found = true;
                // Check if file has changed since backup
                let status = if let Some(scan_hash) = &scan_file.hash {
                    if scan_hash == &backup_file.sha256 {
                        "protected"
                    } else {
                        "outdated"
                    }
                } else if scan_file.size_bytes == backup_file.size_bytes {
                    "protected"
                } else {
                    "outdated"
                };
                
                results.push(FileBackupStatus {
                    file_id: *file_id,
                    status: status.to_string(),
                    last_verified_at_ms: Some(*snap_created_at),
                    backup_set_id: Some(*set_id),
                    backup_set_name: Some(set_name.clone()),
                    snapshot_id: Some(*snap_id),
                    snapshot_label: Some(snap_label.clone()),
                });
                break;
            }
        }
        
        if !found {
            results.push(FileBackupStatus {
                file_id: *file_id,
                status: "never_backed_up".to_string(),
                last_verified_at_ms: None,
                backup_set_id: None,
                backup_set_name: None,
                snapshot_id: None,
                snapshot_label: None,
            });
        }
    }
    
    Ok(results)
}

pub type BackupOpItem = (Option<String>, Option<String>, Option<i64>, crate::models::OpItemStatus, Option<String>);

/// Records a backup operation in the operation history.
pub fn record_backup_operation(
    store: &Store,
    _backup_set: &BackupSet,
    kind: crate::models::OperationKind,
    status: crate::models::OperationStatus,
    detail: Option<String>,
    items: &[BackupOpItem],
) -> Result<i64, BackupError> {
    let batch_id = format!("backup-{}-{}", kind.as_str(), now_ms());
    
    let operation_id = store
        .insert_operation(&batch_id, kind)
        .map_err(BackupError::DatabaseError)?;
    
    for (source, destination, file_id, item_status, error) in items {
        store
            .insert_operation_item(
                operation_id,
                source.as_deref(),
                destination.as_deref(),
                *file_id,
                *item_status,
                error.as_deref(),
            )
            .map_err(BackupError::DatabaseError)?;
    }
    
    store
        .finish_operation(operation_id, status, detail.as_deref())
        .map_err(BackupError::DatabaseError)?;
    
    Ok(operation_id)
}

/// Records a backup started operation.
pub fn record_backup_started(
    store: &Store,
    backup_set: &BackupSet,
    total_files: u64,
    estimated_bytes: u64,
) -> Result<i64, BackupError> {
    let detail = format!(
        "Backup '{}' started: {} files, {} estimated",
        backup_set.name,
        total_files,
        format_bytes(estimated_bytes)
    );
    
    record_backup_operation(
        store,
        backup_set,
        crate::models::OperationKind::Backup,
        crate::models::OperationStatus::InProgress,
        Some(detail),
        &[],
    )
}

/// Records a backup completed operation.
pub fn record_backup_completed(
    store: &Store,
    backup_set: &BackupSet,
    snapshot_id: i64,
    summary: &BackupRunSummary,
) -> Result<i64, BackupError> {
    let detail = format!(
        "Backup '{}' completed: {} files ({} new, {} changed, {} unchanged), {} transferred, {} verified",
        backup_set.name,
        summary.files_total,
        summary.files_new,
        summary.files_changed,
        summary.files_unchanged,
        format_bytes(summary.bytes_transferred),
        format_bytes(summary.bytes_verified),
    );
    
    let items = vec![
        (None, Some(format!("snapshot:{}", snapshot_id)), None, OpItemStatus::Completed, None),
    ];
    
    record_backup_operation(
        store,
        backup_set,
        crate::models::OperationKind::Backup,
        crate::models::OperationStatus::Completed,
        Some(detail),
        &items,
    )
}

/// Records a backup failed operation.
pub fn record_backup_failed(
    store: &Store,
    backup_set: &BackupSet,
    error: &str,
) -> Result<i64, BackupError> {
    let detail = format!("Backup '{}' failed: {}", backup_set.name, error);
    
    record_backup_operation(
        store,
        backup_set,
        crate::models::OperationKind::Backup,
        crate::models::OperationStatus::Failed,
        Some(detail),
        &[],
    )
}

/// Records a backup cancelled operation.
pub fn record_backup_cancelled(
    store: &Store,
    backup_set: &BackupSet,
    completed_files: u64,
    verified_files: u64,
) -> Result<i64, BackupError> {
    let detail = format!(
        "Backup '{}' cancelled: {} files completed, {} verified",
        backup_set.name,
        completed_files,
        verified_files,
    );
    
    record_backup_operation(
        store,
        backup_set,
        crate::models::OperationKind::BackupCancel,
        crate::models::OperationStatus::Cancelled,
        Some(detail),
        &[],
    )
}

/// Records a backup verification operation.
pub fn record_backup_verify(
    store: &Store,
    backup_set: &BackupSet,
    _snapshot_id: i64,
    total_files: u64,
    verified_files: u64,
    failed_files: u64,
    errors: &[(String, String)], // (relative_path, error)
) -> Result<i64, BackupError> {
    let detail = format!(
        "Verification of '{}': {} checked, {} valid, {} failed",
        backup_set.name,
        total_files,
        verified_files,
        failed_files,
    );
    
    let items: Vec<_> = errors.iter().map(|(path, err)| {
        (Some(path.clone()), None, None, OpItemStatus::Failed, Some(err.clone()))
    }).collect();
    
    record_backup_operation(
        store,
        backup_set,
        crate::models::OperationKind::BackupVerify,
        if failed_files > 0 { crate::models::OperationStatus::Failed } else { crate::models::OperationStatus::Completed },
        Some(detail),
        &items.iter().map(|(s, d, f, st, e)| (s.clone(), d.clone(), *f, *st, e.clone())).collect::<Vec<_>>(),
    )
}

/// Records a backup restore operation.
pub fn record_backup_restore(
    store: &Store,
    backup_set: &BackupSet,
    snapshot_id: i64,
    result: &RestoreResult,
) -> Result<i64, BackupError> {
    let detail = format!(
        "Restore from '{}' (snapshot {}): {} requested, {} restored, {} verified, {} failed, {} skipped",
        backup_set.name,
        snapshot_id,
        result.requested,
        result.restored,
        result.verified,
        result.failed,
        result.skipped,
    );
    
    let items: Vec<_> = result.errors.iter().map(|e| {
        (Some(e.relative_path.clone()), None, None, OpItemStatus::Failed, Some(e.error.clone()))
    }).collect();
    
    let success_items = vec![
        (None, Some(format!("snapshot:{}", snapshot_id)), None, OpItemStatus::Completed, None),
    ];
    
    let all_items: Vec<_> = success_items.into_iter().chain(items).collect();
    
    record_backup_operation(
        store,
        backup_set,
        crate::models::OperationKind::BackupRestore,
        if result.failed > 0 { crate::models::OperationStatus::Failed } else { crate::models::OperationStatus::Completed },
        Some(detail),
        &all_items.iter().map(|(s, d, f, st, e)| (s.clone(), d.clone(), *f, *st, e.clone())).collect::<Vec<_>>(),
    )
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 { return "0 B".to_string(); }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 || unit == 0 {
        format!("{} {}", value.round() as u64, units[unit])
    } else {
        format!("{:.1} {}", value, units[unit])
    }
}



/// Test helper functions for creating test data.
pub mod test_helpers {
    use super::*;
    use crate::store::Store;
    use crate::backup::models::BackupRunStatus;

    /// Creates a test backup run directly in the database.
    pub fn create_test_backup_run(
        store: &Store,
        backup_set_id: i64,
        status: BackupRunStatus,
    ) -> i64 {
        let now = now_ms();
        store.conn.execute(
            "INSERT INTO backup_runs (backup_set_id, status, started_at_ms, files_total) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![backup_set_id, status.as_str(), now, 0],
        ).unwrap();
        store.conn.last_insert_rowid()
    }

    /// Creates a test backup snapshot directly in the database.
    pub fn create_test_backup_snapshot(
        store: &Store,
        backup_set_id: i64,
        run_id: i64,
        label: &str,
        status: &str,
        created_at_ms: Option<i64>,
    ) -> i64 {
        let now = created_at_ms.unwrap_or_else(now_ms);
        store.conn.execute(
            "INSERT INTO backup_snapshots (backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![backup_set_id, run_id, label, status, now, 0, 0],
        ).unwrap();
        store.conn.last_insert_rowid()
    }

    /// Creates a test backup file entry directly in the database.
    pub fn create_test_backup_file(
        store: &Store,
        snapshot_id: i64,
        relative_path: &str,
        size_bytes: u64,
        sha256: &str,
        status: &str,
    ) -> i64 {
        store.conn.execute(
            "INSERT INTO backup_files (snapshot_id, relative_path, size_bytes, sha256, status) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![snapshot_id, relative_path, size_bytes as i64, sha256, status],
        ).unwrap();
        store.conn.last_insert_rowid()
    }

    /// Retrieves status string of a snapshot by ID.
    pub fn get_snapshot_status(store: &Store, snapshot_id: i64) -> String {
        store.conn.query_row(
            "SELECT status FROM backup_snapshots WHERE id = ?1",
            rusqlite::params![snapshot_id],
            |r| r.get(0),
        ).unwrap()
    }
}