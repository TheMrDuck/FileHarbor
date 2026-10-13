//! Harbor Patrol: Background bit-rot scrubber and autonomous self-healing backup pool repair.
//!
//! Scans immutable content-addressed storage (.fileharbor/pool/xx/xxxx...),
//! validates physical block integrity against cryptographic SHA-256 hashes,
//! and heals bit-rot silent corruption on-the-fly using healthy source replicas.

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::cancel::CancelToken;
use crate::hasher;
use crate::store::Store;
use crate::backup::errors::BackupError;
use crate::backup::models::{now_ms, BackupSnapshotStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarborPatrolSummary {
    pub backup_set_id: i64,
    pub total_pool_objects: u64,
    pub scanned_objects: u64,
    pub healthy_objects: u64,
    pub bit_rot_detected: u64,
    pub self_healed_objects: u64,
    pub unrecoverable_objects: u64,
    pub healed_details: Vec<HealedObjectDetail>,
    pub unrecoverable_details: Vec<UnrecoverableDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealedObjectDetail {
    pub hash: String,
    pub source_path_used: String,
    pub pool_path: String,
    pub healed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnrecoverableDetail {
    pub hash: String,
    pub pool_path: String,
    pub affected_relative_paths: Vec<String>,
    pub affected_snapshot_ids: Vec<i64>,
}

/// Run Harbor Patrol bit-rot scrubbing and self-healing repair over a destination's pool.
pub fn run_harbor_patrol(
    store: &Store,
    backup_set_id: i64,
    destination_root: &Path,
    auto_heal: bool,
    cancel: &CancelToken,
) -> Result<HarborPatrolSummary, BackupError> {
    let pool_dir = destination_root.join(".fileharbor").join("pool");
    let partial_pool_dir = destination_root.join(".fileharbor-partial").join("pool");

    let mut summary = HarborPatrolSummary {
        backup_set_id,
        total_pool_objects: 0,
        scanned_objects: 0,
        healthy_objects: 0,
        bit_rot_detected: 0,
        self_healed_objects: 0,
        unrecoverable_objects: 0,
        healed_details: Vec::new(),
        unrecoverable_details: Vec::new(),
    };

    if !pool_dir.exists() {
        return Ok(summary);
    }

    // Collect all pool object paths
    let mut pool_objects: Vec<(String, PathBuf)> = Vec::new();

    if let Ok(prefix_entries) = fs::read_dir(&pool_dir) {
        for prefix_entry in prefix_entries.flatten() {
            let prefix_path = prefix_entry.path();
            if !prefix_path.is_dir() {
                continue;
            }
            let prefix = match prefix_path.file_name().and_then(|n| n.to_str()) {
                Some(p) if p.len() == 2 => p,
                _ => continue,
            };

            if let Ok(file_entries) = fs::read_dir(&prefix_path) {
                for file_entry in file_entries.flatten() {
                    let file_path = file_entry.path();
                    if !file_path.is_file() {
                        continue;
                    }
                    if let Some(suffix) = file_path.file_name().and_then(|n| n.to_str()) {
                        let expected_hash = format!("{prefix}{suffix}").to_lowercase();
                        if expected_hash.len() == 64 && expected_hash.chars().all(|c| c.is_ascii_hexdigit()) {
                            pool_objects.push((expected_hash, file_path));
                        }
                    }
                }
            }
        }
    }

    summary.total_pool_objects = pool_objects.len() as u64;

    for (expected_hash, pool_path) in pool_objects {
        if cancel.is_cancelled() {
            break;
        }

        summary.scanned_objects += 1;

        // Hash pool file directly
        let actual_hash = match hasher::hash_file(&pool_path) {
            Ok(h) => h.to_lowercase(),
            Err(_) => "".to_string(),
        };

        if actual_hash == expected_hash {
            summary.healthy_objects += 1;
            continue;
        }

        // Bit-rot detected!
        summary.bit_rot_detected += 1;

        let mut healed = false;

        if auto_heal {
            // Find active source files for this hash
            let candidates = find_source_candidates_for_hash(store, &expected_hash, backup_set_id);

            for (src_root, rel_path) in candidates {
                let candidate_source = Path::new(&src_root).join(&rel_path);
                if candidate_source.is_file() {
                    if let Ok(source_hash) = hasher::hash_file(&candidate_source) {
                        if source_hash.to_lowercase() == expected_hash {
                            // Valid healthy source found! Perform atomic self-healing repair.
                            let _ = fs::create_dir_all(&partial_pool_dir);
                            let partial_file = partial_pool_dir.join(format!("{}.patrol-heal", expected_hash));

                            if fs::copy(&candidate_source, &partial_file).is_ok() {
                                if let Ok(verified_partial) = hasher::hash_file(&partial_file) {
                                    if verified_partial.to_lowercase() == expected_hash {
                                        // Replace corrupt pool object atomically
                                        if fs::rename(&partial_file, &pool_path).is_ok() {
                                            summary.self_healed_objects += 1;
                                            summary.healed_details.push(HealedObjectDetail {
                                                hash: expected_hash.clone(),
                                                source_path_used: candidate_source.to_string_lossy().to_string(),
                                                pool_path: pool_path.to_string_lossy().to_string(),
                                                healed_at_ms: now_ms(),
                                            });
                                            healed = true;
                                            break;
                                        }
                                    }
                                }
                                let _ = fs::remove_file(&partial_file);
                            }
                        }
                    }
                }
            }
        }

        if !healed {
            summary.unrecoverable_objects += 1;

            // Look up affected snapshots and paths
            let (affected_paths, affected_snaps) = get_affected_files_for_hash(store, &expected_hash);

            for snap_id in &affected_snaps {
                let _ = store.conn.execute(
                    "UPDATE backup_snapshots SET status = ?2 WHERE id = ?1",
                    rusqlite::params![snap_id, BackupSnapshotStatus::Corrupted.as_str()],
                );
            }

            summary.unrecoverable_details.push(UnrecoverableDetail {
                hash: expected_hash,
                pool_path: pool_path.to_string_lossy().to_string(),
                affected_relative_paths: affected_paths,
                affected_snapshot_ids: affected_snaps,
            });
        }
    }

    Ok(summary)
}

fn find_source_candidates_for_hash(
    store: &Store,
    hash: &str,
    priority_backup_set_id: i64,
) -> Vec<(String, String)> {
    let mut candidates = Vec::new();

    // 1. Try priority backup set
    let sql_priority = "
        SELECT s.source_path, f.relative_path
        FROM backup_files f
        JOIN backup_snapshots snap ON snap.id = f.snapshot_id
        JOIN backup_sets s ON s.id = snap.backup_set_id
        WHERE f.sha256 = ?1 AND s.id = ?2
    ";

    if let Ok(mut stmt) = store.conn.prepare(sql_priority) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![hash, priority_backup_set_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        }) {
            for item in rows.flatten() {
                candidates.push(item);
            }
        }
    }

    // 2. Fallback to any backup set
    if candidates.is_empty() {
        let sql_all = "
            SELECT s.source_path, f.relative_path
            FROM backup_files f
            JOIN backup_snapshots snap ON snap.id = f.snapshot_id
            JOIN backup_sets s ON s.id = snap.backup_set_id
            WHERE f.sha256 = ?1
        ";
        if let Ok(mut stmt) = store.conn.prepare(sql_all) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![hash], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            }) {
                for item in rows.flatten() {
                    candidates.push(item);
                }
            }
        }
    }

    candidates
}

fn get_affected_files_for_hash(store: &Store, hash: &str) -> (Vec<String>, Vec<i64>) {
    let mut paths = Vec::new();
    let mut snaps = Vec::new();

    let sql = "SELECT relative_path, snapshot_id FROM backup_files WHERE sha256 = ?1";
    if let Ok(mut stmt) = store.conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![hash], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        }) {
            for (p, s) in rows.flatten() {
                paths.push(p);
                if !snaps.contains(&s) {
                    snaps.push(s);
                }
            }
        }
    }

    (paths, snaps)
}


/// Run Harbor Patrol for a configured backup set ID.
pub fn run_patrol_for_set(
    store: &Store,
    backup_set_id: i64,
    auto_heal: bool,
    cancel: &CancelToken,
) -> Result<HarborPatrolSummary, BackupError> {
    let set = crate::backup::get_backup_set(store, backup_set_id)?
        .ok_or_else(|| BackupError::InvalidConfiguration(format!("Backup set #{backup_set_id} not found")))?;
    let dest_path = crate::backup::destination::get_destination_path(store, set.destination_id)?;
    run_harbor_patrol(store, backup_set_id, Path::new(&dest_path), auto_heal, cancel)
}
