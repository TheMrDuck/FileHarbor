//! Backup restore: restores files from backup with conflict resolution.

use std::path::{Path, PathBuf};
use std::fs;

use serde::{Deserialize, Serialize};
use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;
use crate::hasher;

/// Restore preview showing what would happen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub total_files: u64,
    pub existing_files: u64,
    pub missing_files: u64,
    pub conflicts: Vec<RestoreConflict>,
}

/// A file that would have a conflict on restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreConflict {
    pub relative_path: String,
    pub existing_size: u64,
    pub existing_hash: Option<String>,
    pub backup_size: u64,
    pub backup_hash: String,
}

/// Restores files from a backup snapshot.
pub fn restore_from_backup(
    store: &Store,
    snapshot_id: i64,
    destination_root: &Path,
    conflicts: &[(String, RestoreConflictPolicy)],
) -> Result<RestoreResult, BackupError> {
    let snapshot = get_snapshot(store, snapshot_id)?;
    
    // Get backup set to find the backup destination
    let backup_set = get_backup_set(store, snapshot.backup_set_id)?;
    let backup_destination = get_backup_destination(store, backup_set.destination_id)?;
    let backup_destination_root = Path::new(&backup_destination.path);
    
    let files = get_snapshot_files(store, snapshot.id)?;

    let mut result = RestoreResult {
        requested: files.len() as u64,
        restored: 0,
        verified: 0,
        failed: 0,
        skipped: 0,
        conflicts_resolved: 0,
        errors: Vec::new(),
    };

    for file in files {
        let conflict_policy = conflicts.iter()
            .find(|(p, _)| p == &file.relative_path)
            .map(|(_, policy)| *policy);

        let dest_path = destination_root.join(&file.relative_path);
        
        match restore_single_file(store, &file, &dest_path, conflict_policy, backup_destination_root) {
            Ok(restored) => {
                if restored {
                    result.restored += 1;
                    // Verify restored file
                    if verify_restored_file(&dest_path, &file) {
                        result.verified += 1;
                    } else {
                        result.failed += 1;
                        result.errors.push(RestoreError {
                            relative_path: file.relative_path.clone(),
                            error: "Restored file hash mismatch".to_string(),
                        });
                    }
                } else {
                    result.skipped += 1;
                }
            }
            Err(e) => {
                result.failed += 1;
                result.errors.push(RestoreError {
                    relative_path: file.relative_path.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(result)
}

/// Restores a single file.
fn restore_single_file(
    store: &Store,
    file: &BackupFile,
    dest_path: &Path,
    conflict_policy: Option<RestoreConflictPolicy>,
    backup_destination_root: &Path,
) -> Result<bool, BackupError> {
    // Check if destination exists
    if dest_path.exists() {
        let policy = conflict_policy.unwrap_or(RestoreConflictPolicy::Cancel);
        
        match policy {
            RestoreConflictPolicy::Cancel => {
                return Err(BackupError::InvalidConfiguration(
                    format!("File exists and policy is cancel: {}", file.relative_path)
                ));
            }
            RestoreConflictPolicy::KeepExisting => {
                return Ok(false); // Skip
            }
            RestoreConflictPolicy::RestoreWithNewName => {
                // Generate new name
                let new_path = generate_unique_name(dest_path)?;
                return restore_to_path(store, file, &new_path, backup_destination_root);
            }
        }
    }

    restore_to_path(store, file, dest_path, backup_destination_root)
}

/// Restores file to a specific path with pre-verification and atomic staging.
fn restore_to_path(
    _store: &Store,
    file: &BackupFile,
    dest_path: &Path,
    backup_destination_root: &Path,
) -> Result<bool, BackupError> {
    // 1. Locate pool object
    let rel_pool = crate::backup::manifest::pool_rel_path(&file.sha256)?;
    let pool_file = backup_destination_root.join(".fileharbor").join("pool").join(&rel_pool);

    // Fallback for legacy flat files if pool doesn't exist
    let source_path = if pool_file.exists() {
        pool_file
    } else {
        let legacy = backup_destination_root.join(&file.relative_path);
        if legacy.exists() {
            legacy
        } else {
            return Err(BackupError::ContentNotFound(format!(
                "Backup content object missing: {}",
                pool_file.display()
            )));
        }
    };

    // 2. Pre-verify pool object before restoration
    let pool_hash = hasher::hash_file(&source_path)
        .map_err(|e| BackupError::TransferFailed(format!("Failed to hash pool object: {}", e)))?;
    if pool_hash != file.sha256 {
        return Err(BackupError::VerificationFailed(format!(
            "Pool object corrupted for {}: expected {}, got {}",
            file.relative_path, file.sha256, pool_hash
        )));
    }

    // 3. Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| BackupError::TransferFailed(format!("Failed to create dir: {}", e)))?;
    }

    // 4. Staged restore via .fileharbor-restore
    let staging_path = dest_path.with_extension("fileharbor-restore");
    fs::copy(&source_path, &staging_path)
        .map_err(|e| BackupError::TransferFailed(format!("Failed to copy file to stage: {}", e)))?;

    // 5. Verify staged file
    let staged_hash = hasher::hash_file(&staging_path)
        .map_err(|e| BackupError::TransferFailed(format!("Failed to hash staged file: {}", e)))?;
    if staged_hash != file.sha256 {
        let _ = fs::remove_file(&staging_path);
        return Err(BackupError::VerificationFailed(format!(
            "Staged restore verification failed: expected {}, got {}",
            file.sha256, staged_hash
        )));
    }

    // 6. Atomic promote into destination path
    fs::rename(&staging_path, dest_path)
        .map_err(|e| BackupError::TransferFailed(format!("Failed to promote restored file: {}", e)))?;

    Ok(true)
}

/// Verifies a restored file matches expected hash.
fn verify_restored_file(dest_path: &Path, file: &BackupFile) -> bool {
    if !dest_path.exists() {
        return false;
    }

    let meta = match fs::metadata(dest_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    if meta.len() != file.size_bytes {
        return false;
    }

    let computed_hash = match hasher::hash_file(dest_path) {
        Ok(h) => h,
        Err(_) => return false,
    };

    computed_hash == file.sha256
}

/// Generates a unique filename by appending a number.
fn generate_unique_name(path: &Path) -> Result<PathBuf, BackupError> {
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = path.extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    for i in 1..10000 {
        let new_name = format!("{}-{}{}", stem, i, ext);
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return Ok(new_path);
        }
    }

    Err(BackupError::Internal("Could not generate unique name".to_string()))
}

/// Gets a restore preview.
pub fn get_restore_preview(
    store: &Store,
    snapshot_id: i64,
    destination_root: &Path,
) -> Result<RestorePreview, BackupError> {
    let files = get_snapshot_files(store, snapshot_id)?;
    
    let mut preview = RestorePreview {
        total_files: files.len() as u64,
        existing_files: 0,
        missing_files: 0,
        conflicts: Vec::new(),
    };

    for file in files {
        let dest_path = destination_root.join(&file.relative_path);
        if dest_path.exists() {
            preview.existing_files += 1;
            
            let existing_meta = fs::metadata(&dest_path)
                .map_err(|e| BackupError::SourceUnreadable(e.to_string()))?;
            
            let existing_hash = if existing_meta.len() == file.size_bytes {
                hasher::hash_file(&dest_path).ok()
            } else {
                None
            };

            preview.conflicts.push(RestoreConflict {
                relative_path: file.relative_path.clone(),
                existing_size: existing_meta.len(),
                existing_hash,
                backup_size: file.size_bytes,
                backup_hash: file.sha256.clone(),
            });
        } else {
            preview.missing_files += 1;
        }
    }

    Ok(preview)
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

/// Gets a backup set by ID.
fn get_backup_set(
    store: &Store,
    backup_set_id: i64,
) -> Result<BackupSet, BackupError> {
    store.conn.query_row(
        "SELECT id, name, source_path, destination_id, schedule, retention_policy, verification_mode, created_at_ms, updated_at_ms
         FROM backup_sets WHERE id = ?1",
        rusqlite::params![backup_set_id],
        |row| {
            let schedule_json: String = row.get(4)?;
            let retention_json: String = row.get(5)?;
            let verification_str: String = row.get(6)?;
            Ok(BackupSet {
                id: row.get(0)?,
                name: row.get(1)?,
                source_path: row.get(2)?,
                destination_id: row.get(3)?,
                schedule: serde_json::from_str(&schedule_json).unwrap_or_default(),
                retention_policy: serde_json::from_str(&retention_json).unwrap_or_default(),
                verification_mode: VerificationMode::parse(&verification_str),
                created_at_ms: row.get(7)?,
                updated_at_ms: row.get(8)?,
            })
        },
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Gets a backup destination by ID.
fn get_backup_destination(
    store: &Store,
    destination_id: i64,
) -> Result<BackupDestination, BackupError> {
    store.conn.query_row(
        "SELECT id, destination_id, path, label, filesystem_id, created_at_ms, last_seen_at_ms
         FROM backup_destinations WHERE id = ?1",
        rusqlite::params![destination_id],
        |row| {
            Ok(BackupDestination {
                id: row.get(0)?,
                destination_id: row.get(1)?,
                path: row.get(2)?,
                label: row.get(3)?,
                filesystem_id: row.get(4)?,
                created_at_ms: row.get(5)?,
                last_seen_at_ms: row.get(6)?,
            })
        },
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))
}