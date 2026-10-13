//! Backup retention: manages snapshot lifecycle with crash-safe reference-driven GC.

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;

/// Applies retention policy to a backup set following a strict 7-phase state machine:
/// PLAN -> VERIFY SURVIVORS -> PRE-FLIGHT VERIFY RETAINED OBJECTS -> COMMIT METADATA ->
/// REMOVE PRUNED MANIFESTS -> GC UNREFERENCED OBJECTS -> FINAL VERIFY.
pub fn apply_retention(
    store: &Store,
    backup_set_id: i64,
    policy: &RetentionPolicy,
) -> Result<RetentionResult, BackupError> {
    let snapshots = get_snapshots_for_set(store, backup_set_id)?;
    if snapshots.is_empty() {
        return Ok(RetentionResult {
            deleted_snapshots: 0,
            freed_bytes: 0,
        });
    }

    // Phase 1: PLAN
    let (retained, to_delete) = match policy {
        RetentionPolicy::KeepAll => {
            return Ok(RetentionResult {
                deleted_snapshots: 0,
                freed_bytes: 0,
            });
        }
        RetentionPolicy::KeepLastN(n) => {
            let n = *n as usize;
            if snapshots.len() <= n {
                return Ok(RetentionResult {
                    deleted_snapshots: 0,
                    freed_bytes: 0,
                });
            }
            let keep_count = n.max(1); // Never prune all snapshots down to 0
            (&snapshots[..keep_count], &snapshots[keep_count..])
        }
    };

    if to_delete.is_empty() {
        return Ok(RetentionResult {
            deleted_snapshots: 0,
            freed_bytes: 0,
        });
    }

    // Phase 2: VERIFY SURVIVORS
    if retained.is_empty() {
        return Err(BackupError::InvalidConfiguration(
            "Retention policy would prune all snapshots. At least 1 snapshot must be retained.".to_string(),
        ));
    }

    // Locate destination root
    let dest_path_str: Result<String, _> = store.conn.query_row(
        "SELECT bd.path FROM backup_destinations bd
         JOIN backup_sets bs ON bs.destination_id = bd.id
         WHERE bs.id = ?1",
        rusqlite::params![backup_set_id],
        |row| row.get(0),
    );
    let dest_root = dest_path_str.ok().map(PathBuf::from);

    // Phase 3: PRE-FLIGHT VERIFY ALL RETAINED OBJECTS
    // Collect reachability set R of all content hashes across all retained snapshots
    let mut retained_hashes = std::collections::HashSet::new();
    for snap in retained {
        let mut stmt = store.conn.prepare(
            "SELECT sha256 FROM backup_files WHERE snapshot_id = ?1",
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        let rows = stmt.query_map(rusqlite::params![snap.id], |row| row.get::<_, String>(0))
            .map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        for hash in rows.flatten() {
            retained_hashes.insert(hash);
        }
    }

    // If destination disk exists, verify every hash in retained_hashes is physically present in pool/
    if let Some(ref root) = dest_root {
        let pool_dir = root.join(".fileharbor").join("pool");
        if pool_dir.exists() {
            for hash in &retained_hashes {
                if let Ok(rel) = crate::backup::manifest::pool_rel_path(hash) {
                    let obj_path = pool_dir.join(&rel);
                    if !obj_path.exists() {
                        return Err(BackupError::VerificationFailed(format!(
                            "Retention pre-flight check failed: surviving snapshot references missing pool object {}",
                            hash
                        )));
                    }
                }
            }
        }
    }

    // Phase 4: COMMIT METADATA
    let tx = store.conn.unchecked_transaction()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    for snap in to_delete {
        tx.execute(
            "UPDATE backup_runs SET snapshot_id = NULL WHERE snapshot_id = ?1",
            rusqlite::params![snap.id],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        tx.execute(
            "DELETE FROM backup_files WHERE snapshot_id = ?1",
            rusqlite::params![snap.id],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        tx.execute(
            "DELETE FROM backup_snapshots WHERE id = ?1",
            rusqlite::params![snap.id],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    }
    tx.commit().map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    // Phase 5: REMOVE PRUNED MANIFESTS
    if let Some(ref root) = dest_root {
        let manifests_dir = root.join(".fileharbor").join("manifests");
        for snap in to_delete {
            let manifest_file = manifests_dir.join(format!("{}.json", snap.label));
            let _ = fs::remove_file(manifest_file);
        }
    }

    // Phase 6: GC UNREFERENCED OBJECTS
    let mut freed_bytes = 0u64;
    if let Some(ref root) = dest_root {
        let pool_dir = root.join(".fileharbor").join("pool");
        if pool_dir.exists() {
            if let Ok(shards) = fs::read_dir(&pool_dir) {
                for shard_entry in shards.flatten() {
                    let shard_path = shard_entry.path();
                    if shard_path.is_dir() {
                        if let Ok(objects) = fs::read_dir(&shard_path) {
                            for obj_entry in objects.flatten() {
                                let obj_path = obj_entry.path();
                                if obj_path.is_file() {
                                    let file_name = obj_path.file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("");
                                    if file_name.len() == 64 && !retained_hashes.contains(file_name) {
                                        let size = fs::metadata(&obj_path).map(|m| m.len()).unwrap_or(0);
                                        if fs::remove_file(&obj_path).is_ok() {
                                            freed_bytes += size;
                                        }
                                    }
                                }
                            }
                        }
                        // Remove shard directory if empty
                        let _ = fs::remove_dir(&shard_path);
                    }
                }
            }
        }
    }

    // Phase 7: FINAL VERIFY
    if let Some(ref root) = dest_root {
        let pool_dir = root.join(".fileharbor").join("pool");
        if pool_dir.exists() {
            for hash in &retained_hashes {
                if let Ok(rel) = crate::backup::manifest::pool_rel_path(hash) {
                    let obj_path = pool_dir.join(&rel);
                    if !obj_path.exists() {
                        return Err(BackupError::VerificationFailed(format!(
                            "Retention final verify failed: pool object missing after GC: {}",
                            hash
                        )));
                    }
                }
            }
        }
    }

    Ok(RetentionResult {
        deleted_snapshots: to_delete.len() as u64,
        freed_bytes,
    })
}

/// Result of retention operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionResult {
    pub deleted_snapshots: u64,
    pub freed_bytes: u64,
}

/// Deletes a snapshot and its database records.
pub fn delete_snapshot(
    store: &Store,
    snapshot_id: i64,
) -> Result<u64, BackupError> {
    let tx = store.conn.unchecked_transaction()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    // Sum sizes of files in snapshot
    let total_size: u64 = {
        let mut stmt = tx.prepare(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM backup_files WHERE snapshot_id = ?1",
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        stmt.query_row(rusqlite::params![snapshot_id], |row| row.get(0))
            .map_err(|e| BackupError::DatabaseError(e.to_string()))?
    };

    tx.execute(
        "UPDATE backup_runs SET snapshot_id = NULL WHERE snapshot_id = ?1",
        rusqlite::params![snapshot_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    tx.execute(
        "DELETE FROM backup_files WHERE snapshot_id = ?1",
        rusqlite::params![snapshot_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    tx.execute(
        "DELETE FROM backup_snapshots WHERE id = ?1",
        rusqlite::params![snapshot_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    tx.commit().map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    Ok(total_size)
}

/// Gets all snapshots for a backup set, ordered by creation time (newest first).
fn get_snapshots_for_set(
    store: &Store,
    backup_set_id: i64,
) -> Result<Vec<BackupSnapshot>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes
         FROM backup_snapshots
         WHERE backup_set_id = ?1
         ORDER BY created_at_ms DESC",
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

/// Garbage collects unreferenced content objects.
pub fn garbage_collect_contents(
    store: &Store,
) -> Result<GcResult, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, size_bytes FROM backup_contents WHERE reference_count = 0",
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let contents: Vec<(i64, u64)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let mut freed_bytes = 0u64;
    let mut deleted = 0u64;

    for (cid, size) in contents {
        store.conn.execute(
            "DELETE FROM backup_contents WHERE id = ?1",
            rusqlite::params![cid],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        freed_bytes += size;
        deleted += 1;
    }

    Ok(GcResult {
        deleted_contents: deleted,
        freed_bytes,
    })
}

/// Result of garbage collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcResult {
    pub deleted_contents: u64,
    pub freed_bytes: u64,
}
