//! Backup verification: verifies copied files and detects corruption.

use std::path::Path;

use serde::{Deserialize, Serialize};
use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;
use crate::hasher;

/// Verifies a backup run's transferred files.
pub fn verify_backup_run(
    store: &Store,
    run_id: i64,
    destination_root: &Path,
) -> Result<VerificationResult, BackupError> {
    let snapshot_id = get_run_snapshot_id(store, run_id)?;
    let snapshot = get_snapshot(store, snapshot_id)?;

    let files = get_snapshot_files(store, snapshot.id)?;
    let mut result = VerificationResult {
        run_id,
        snapshot_id,
        total_files: files.len() as u64,
        verified_files: 0,
        failed_files: 0,
        missing_files: 0,
        corrupted_files: 0,
        errors: Vec::new(),
    };

    for file in files {
        let dest_path = destination_root.join(&file.relative_path);
        match verify_single_file(&dest_path, &file) {
            Ok(verified) => {
                if verified {
                    result.verified_files += 1;
                    update_backup_file_status(store, file.id, BackupFileStatus::Verified)?;
                } else {
                    result.failed_files += 1;
                    update_backup_file_status(store, file.id, BackupFileStatus::Failed)?;
                    result.errors.push(FileVerificationError {
                        relative_path: file.relative_path.clone(),
                        error: "Hash mismatch".to_string(),
                        expected_hash: Some(file.sha256.clone()),
                        actual_hash: None,
                    });
                }
            }
            Err(e) => {
                if e.to_string().contains("not found") {
                    result.missing_files += 1;
                } else {
                    result.failed_files += 1;
                }
                update_backup_file_status(store, file.id, BackupFileStatus::Failed)?;
                result.errors.push(FileVerificationError {
                    relative_path: file.relative_path.clone(),
                    error: e.to_string(),
                    expected_hash: Some(file.sha256.clone()),
                    actual_hash: None,
                });
            }
        }
    }

    // Update run status
    let run_status = if result.failed_files == 0 && result.missing_files == 0 {
        BackupRunStatus::Completed
    } else if result.verified_files > 0 {
        BackupRunStatus::Partial
    } else {
        BackupRunStatus::Failed
    };
    update_run_status(store, run_id, run_status)?;

    // Update snapshot status
    let snap_status = if result.failed_files == 0 && result.missing_files == 0 {
        BackupSnapshotStatus::Verified
    } else if result.corrupted_files > 0 {
        BackupSnapshotStatus::Corrupted
    } else {
        BackupSnapshotStatus::Unverified
    };
    update_snapshot_status(store, snapshot_id, snap_status)?;

    Ok(result)
}

/// Result of backup verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub run_id: i64,
    pub snapshot_id: i64,
    pub total_files: u64,
    pub verified_files: u64,
    pub failed_files: u64,
    pub missing_files: u64,
    pub corrupted_files: u64,
    pub errors: Vec<FileVerificationError>,
}

/// Individual file verification error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVerificationError {
    pub relative_path: String,
    pub error: String,
    pub expected_hash: Option<String>,
    pub actual_hash: Option<String>,
}

/// Verifies a single file against its expected hash.
fn verify_single_file(
    dest_path: &Path,
    file: &BackupFile,
) -> Result<bool, BackupError> {
    if !dest_path.exists() {
        return Err(BackupError::SourceMissing(format!(
            "Backup file not found: {}",
            dest_path.display()
        )));
    }

    // Check size first (fast)
    let meta = std::fs::metadata(dest_path)
        .map_err(|e| BackupError::SourceUnreadable(e.to_string()))?;

    if meta.len() != file.size_bytes {
        return Err(BackupError::VerificationFailed(format!(
            "Size mismatch for {}: expected {}, got {}",
            file.relative_path, file.size_bytes, meta.len()
        )));
    }

    // Compute hash
    let computed_hash = hasher::hash_file(dest_path)
        .map_err(|e| BackupError::VerificationFailed(e.to_string()))?;

    if computed_hash != file.sha256 {
        return Ok(false);
    }

    Ok(true)
}

/// Performs deep verification - rehashes all files in a snapshot.
pub fn deep_verify_snapshot(
    store: &Store,
    snapshot_id: i64,
    destination_root: &Path,
) -> Result<VerificationResult, BackupError> {
    let snapshot = get_snapshot(store, snapshot_id)?;
    let files = get_snapshot_files(store, snapshot.id)?;

    let mut result = VerificationResult {
        run_id: snapshot.run_id,
        snapshot_id,
        total_files: files.len() as u64,
        verified_files: 0,
        failed_files: 0,
        missing_files: 0,
        corrupted_files: 0,
        errors: Vec::new(),
    };

    for file in files {
        let dest_path = destination_root.join(&file.relative_path);
        match verify_single_file(&dest_path, &file) {
            Ok(verified) => {
                if verified {
                    result.verified_files += 1;
                    update_backup_file_status(store, file.id, BackupFileStatus::Verified)?;
                } else {
                    result.corrupted_files += 1;
                    result.failed_files += 1;
                    update_backup_file_status(store, file.id, BackupFileStatus::Failed)?;
                    result.errors.push(FileVerificationError {
                        relative_path: file.relative_path.clone(),
                        error: "Hash mismatch (corrupted)".to_string(),
                        expected_hash: Some(file.sha256.clone()),
                        actual_hash: None, // Would need to compute again to get actual
                    });
                }
            }
            Err(e) => {
                if e.to_string().contains("not found") {
                    result.missing_files += 1;
                } else {
                    result.failed_files += 1;
                }
                update_backup_file_status(store, file.id, BackupFileStatus::Failed)?;
                result.errors.push(FileVerificationError {
                    relative_path: file.relative_path.clone(),
                    error: e.to_string(),
                    expected_hash: Some(file.sha256.clone()),
                    actual_hash: None,
                });
            }
        }
    }

    let snap_status = if result.corrupted_files > 0 {
        BackupSnapshotStatus::Corrupted
    } else if result.missing_files > 0 || result.failed_files > 0 {
        BackupSnapshotStatus::Unverified
    } else {
        BackupSnapshotStatus::Verified
    };
    update_snapshot_status(store, snapshot_id, snap_status)?;

    Ok(result)
}

/// Gets the snapshot ID for a run.
fn get_run_snapshot_id(
    store: &Store,
    run_id: i64,
) -> Result<i64, BackupError> {
    store.conn.query_row(
        "SELECT snapshot_id FROM backup_runs WHERE id = ?1",
        rusqlite::params![run_id],
        |r| r.get::<_, Option<i64>>(0),
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?
    .ok_or_else(|| BackupError::RunNotFound(run_id))
}

/// Gets a snapshot by ID.
fn get_snapshot(
    store: &Store,
    snapshot_id: i64,
) -> Result<BackupSnapshot, BackupError> {
    store.conn.query_row(
        "SELECT id, backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes
         FROM backup_snapshots WHERE id = ?1",
        rusqlite::params![snapshot_id],
        |row| {
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
        },
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Gets all files in a snapshot.
fn get_snapshot_files(
    store: &Store,
    snapshot_id: i64,
) -> Result<Vec<BackupFile>, BackupError> {
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

/// Updates a backup file's status.
fn update_backup_file_status(
    store: &Store,
    file_id: i64,
    status: BackupFileStatus,
) -> Result<(), BackupError> {
    store.conn.execute(
        "UPDATE backup_files SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.as_str(), file_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    Ok(())
}

/// Updates a run's status.
fn update_run_status(
    store: &Store,
    run_id: i64,
    status: BackupRunStatus,
) -> Result<(), BackupError> {
    let finished_at = if status.is_terminal() { Some(now_ms()) } else { None };
    store.conn.execute(
        "UPDATE backup_runs SET status = ?1, finished_at_ms = ?2 WHERE id = ?3",
        rusqlite::params![status.as_str(), finished_at, run_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    Ok(())
}

/// Updates a snapshot's status.
fn update_snapshot_status(
    store: &Store,
    snapshot_id: i64,
    status: BackupSnapshotStatus,
) -> Result<(), BackupError> {
    store.conn.execute(
        "UPDATE backup_snapshots SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.as_str(), snapshot_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}