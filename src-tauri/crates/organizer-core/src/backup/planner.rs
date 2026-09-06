//! Backup planning: determines what to backup (new/changed/unchanged/deleted).

use std::collections::HashMap;

use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;
use crate::models::FileRecord;
use rusqlite::OptionalExtension;
use crate::backup::get_backup_set;

/// Computes the backup plan by comparing source files with the last snapshot.
pub fn compute_backup_plan(
    store: &Store,
    backup_set_id: i64,
    source_files: &[FileRecord],
) -> Result<BackupPlan, BackupError> {
    let backup_set = get_backup_set(store, backup_set_id)?
        .ok_or_else(|| BackupError::SetNotFound(backup_set_id))?;
    let source_root = std::path::Path::new(&backup_set.source_path);

    let last_snapshot = get_last_completed_snapshot(store, backup_set_id)?;

    let mut snapshot_files = HashMap::new();
    if let Some(snap) = last_snapshot {
        let files = get_snapshot_files(store, snap.id)?;
        for f in files {
            let full_path = source_root.join(&f.relative_path).to_string_lossy().to_string();
            snapshot_files.insert(full_path, f);
        }
    }

    let mut new_files = Vec::new();
    let mut changed_files = Vec::new();
    let mut unchanged_files = Vec::new();
    let mut deleted_files = Vec::new();
    let mut unavailable_files = Vec::new();

    let mut source_paths = std::collections::HashSet::new();

    for src in source_files {
        source_paths.insert(src.path.clone());

        let status = &src.scan_status;
        if matches!(status, crate::models::ScanStatus::Inaccessible
            | crate::models::ScanStatus::HashFailed
            | crate::models::ScanStatus::ChangedDuringScan) {
            unavailable_files.push(PlannedFile {
                source_path: src.path.clone(),
                relative_path: src.path.clone(), // Will be made relative by caller
                size_bytes: src.size_bytes as u64,
                sha256: src.hash.clone(),
                modified_at_ms: src.modified_at_ms,
            });
            continue;
        }

        if let Some(snap_file) = snapshot_files.get(&src.path) {
            // File exists in snapshot - check if changed
            // We'll use size and hash for comparison
            if src.size_bytes as u64 == snap_file.size_bytes && src.hash.as_deref() == Some(&snap_file.sha256) {
                unchanged_files.push(PlannedFile {
                    source_path: src.path.clone(),
                    relative_path: src.path.clone(),
                    size_bytes: src.size_bytes as u64,
                    sha256: src.hash.clone(),
                    modified_at_ms: src.modified_at_ms,
                });
            } else {
                changed_files.push(PlannedFile {
                    source_path: src.path.clone(),
                    relative_path: src.path.clone(),
                    size_bytes: src.size_bytes as u64,
                    sha256: src.hash.clone(),
                    modified_at_ms: src.modified_at_ms,
                });
            }
        } else {
            // New file
            new_files.push(PlannedFile {
                source_path: src.path.clone(),
                relative_path: src.path.clone(),
                size_bytes: src.size_bytes as u64,
                sha256: src.hash.clone(),
                modified_at_ms: src.modified_at_ms,
            });
        }
    }

    // Find deleted files (in snapshot but not in source)
    for (path, snap_file) in snapshot_files {
        if !source_paths.contains(&path) {
            deleted_files.push(PlannedFile {
                source_path: path.clone(),
                relative_path: path,
                size_bytes: snap_file.size_bytes,
                sha256: Some(snap_file.sha256),
                modified_at_ms: None,
            });
        }
    }

    let total_files = source_files.len() as u64 + deleted_files.len() as u64;
    let total_new_size: u64 = new_files.iter().map(|f| f.size_bytes).sum();
    let total_changed_size: u64 = changed_files.iter().map(|f| f.size_bytes).sum();
    let estimated_transfer = total_new_size + total_changed_size;

    Ok(BackupPlan {
        new_files,
        changed_files,
        unchanged_files,
        deleted_files,
        unavailable_files,
        total_files,
        total_new_size,
        total_changed_size,
        estimated_transfer_bytes: estimated_transfer,
    })
}

/// Gets the last completed snapshot for a backup set.
fn get_last_completed_snapshot(
    store: &Store,
    backup_set_id: i64,
) -> Result<Option<BackupSnapshot>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes
         FROM backup_snapshots
         WHERE backup_set_id = ?1 AND status = 'verified'
         ORDER BY created_at_ms DESC LIMIT 1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let snap = stmt.query_row(rusqlite::params![backup_set_id], |row| {
        Ok(BackupSnapshot {
            id: row.get(0)?,
            backup_set_id: row.get(1)?,
            run_id: row.get(2)?,
            label: row.get(3)?,
            status: BackupSnapshotStatus::Verified,
            created_at_ms: row.get(5)?,
            files_count: row.get(6)?,
            total_size_bytes: row.get(7)?,
        })
    }).optional().map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    Ok(snap)
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

/// Gets the protection status for all files in a session.
pub fn compute_file_protection_status(
    store: &Store,
    session_id: i64,
) -> Result<Vec<(FileRecord, FileProtectionStatus)>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT {FILE_COLS} FROM file_records f WHERE f.session_id = ?1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let files = stmt.query_map(rusqlite::params![session_id], crate::store::map_file)
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let mut result = Vec::new();
    for file in files {
        let status = determine_protection_status(store, &file)?;
        result.push((file, status));
    }

    Ok(result)
}

/// Determines protection status for a single file.
fn determine_protection_status(
    store: &Store,
    file: &FileRecord,
) -> Result<FileProtectionStatus, BackupError> {
    // Check if file has any verified backup
    let mut stmt = store.conn.prepare(
        "SELECT COUNT(*) FROM backup_files bf
         JOIN backup_snapshots bs ON bs.id = bf.snapshot_id
         JOIN backup_runs br ON br.snapshot_id = bs.id
         WHERE bf.relative_path = ?1 AND bf.sha256 = ?2 AND bs.status = 'verified'"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let count: i64 = stmt.query_row(
        rusqlite::params![file.path, file.hash.as_deref().unwrap_or("")],
        |r| r.get(0)
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    if count > 0 {
        // Check if current version is the same as backed up
        if let Some(hash) = &file.hash {
            let mut stmt2 = store.conn.prepare(
                "SELECT COUNT(*) FROM backup_files bf
                 JOIN backup_snapshots bs ON bs.id = bf.snapshot_id
                 WHERE bf.relative_path = ?1 AND bf.sha256 = ?2 AND bs.status = 'verified'"
            ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

            let current_count: i64 = stmt2.query_row(
                rusqlite::params![file.path, hash],
                |r| r.get(0)
            ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

            if current_count > 0 {
                return Ok(FileProtectionStatus::Protected);
            }
        }
        return Ok(FileProtectionStatus::Outdated);
    }

    Ok(FileProtectionStatus::NeverBackedUp)
}