//! Backup retention: manages snapshot lifecycle.

use serde::{Deserialize, Serialize};
use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;

/// Applies retention policy to a backup set.
pub fn apply_retention(
    store: &Store,
    backup_set_id: i64,
    policy: &RetentionPolicy,
) -> Result<RetentionResult, BackupError> {
    let snapshots = get_snapshots_for_set(store, backup_set_id)?;

    match policy {
        RetentionPolicy::KeepAll => {
            Ok(RetentionResult {
                deleted_snapshots: 0,
                freed_bytes: 0,
            })
        }
        RetentionPolicy::KeepLastN(n) => {
            if snapshots.len() <= *n as usize {
                return Ok(RetentionResult {
                    deleted_snapshots: 0,
                    freed_bytes: 0,
                });
            }

            let to_delete = &snapshots[*n as usize..];
            let mut freed_bytes = 0u64;
            let mut deleted = 0u64;

            for snap in to_delete {
                let size = delete_snapshot(store, snap.id)?;
                freed_bytes += size;
                deleted += 1;
            }

            Ok(RetentionResult {
                deleted_snapshots: deleted,
                freed_bytes,
            })
        }
    }
}

/// Result of retention operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionResult {
    pub deleted_snapshots: u64,
    pub freed_bytes: u64,
}

/// Deletes a snapshot and its files (with content deduplication).
pub fn delete_snapshot(
    store: &Store,
    snapshot_id: i64,
) -> Result<u64, BackupError> {
    let tx = store.conn.unchecked_transaction()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    // Get files in snapshot - use a block to ensure stmt is dropped before tx is used again
    let files: Vec<(i64, u64, String, Option<i64>)> = {
        let mut stmt = tx.prepare(
            "SELECT id, size_bytes, sha256, content_id FROM backup_files WHERE snapshot_id = ?1"
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        let rows = stmt.query_map(
            rusqlite::params![snapshot_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| BackupError::DatabaseError(e.to_string()))?
    }; // stmt dropped here

    let mut freed_bytes = 0u64;

    // For each file, decrement content reference count
    for (_file_id, size_bytes, _sha256, content_id) in files {
        if let Some(cid) = content_id {
            // Decrement reference count
            let remaining: i64 = tx.query_row(
                "UPDATE backup_contents SET reference_count = reference_count - 1 WHERE id = ?1 RETURNING reference_count",
                rusqlite::params![cid],
                |r| r.get(0),
            ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

            // If no more references, we could delete the content object
            // But we keep it for safety (garbage collection can be separate)
            if remaining == 0 {
                freed_bytes += size_bytes;
            }
        } else {
            freed_bytes += size_bytes;
        }
    }

    // Delete snapshot files
    tx.execute(
        "DELETE FROM backup_files WHERE snapshot_id = ?1",
        rusqlite::params![snapshot_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    // Delete snapshot
    tx.execute(
        "DELETE FROM backup_snapshots WHERE id = ?1",
        rusqlite::params![snapshot_id],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    tx.commit().map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    Ok(freed_bytes)
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

/// Garbage collects unreferenced content objects.
pub fn garbage_collect_contents(
    store: &Store,
) -> Result<GcResult, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, size_bytes FROM backup_contents WHERE reference_count = 0"
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