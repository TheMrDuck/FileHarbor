use std::path::Path;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::backup::models::*;
use crate::backup::errors::*;
use crate::backup::planner::compute_backup_plan;
use crate::backup::manifest::{write_manifest, BackupManifest, ManifestSource, ManifestFile};
use crate::backup::destination::validate_destination;
use crate::store::Store;
use crate::hasher;
use crate::cancel::CancelToken;
use crate::models::{FileRecord, HashStatus, ScanStatus};
use chrono::{TimeZone, Utc};

pub struct BackupEngine<'a> {
    store: &'a Store,
    cancel_token: CancelToken,
    progress_callback: Option<Box<dyn Fn(BackupProgress) + Send + Sync + 'static>>,
}

impl<'a> BackupEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self {
            store,
            cancel_token: CancelToken::new(),
            progress_callback: None,
        }
    }

    pub fn with_store(store: &'a Store) -> Self {
        Self::new(store)
    }

    pub fn with_cancel_token(mut self, token: CancelToken) -> Self {
        self.cancel_token = token;
        self
    }

    pub fn with_progress<F: Fn(BackupProgress) + Send + Sync + 'static>(mut self, callback: F) -> Self {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    pub fn run_backup(&self, backup_set: &BackupSet, destination_root: &Path) -> Result<BackupRunSummary, BackupError> {
        let run_id = self.store.create_backup_run(backup_set.id)?;
        self.run_backup_with_existing_run(run_id, backup_set, destination_root)
    }

    pub fn run_backup_with_existing_run(&self, run_id: i64, backup_set: &BackupSet, destination_root: &Path) -> Result<BackupRunSummary, BackupError> {
        let start_time = now_ms();

        self.cancel_token.check()?;

        let validation = validate_destination(
            Path::new(&backup_set.source_path),
            destination_root,
        );

        if !validation.valid {
            return Err(BackupError::InvalidConfiguration(
                validation.errors.join(", ")
            ));
        }

        let source_files = self.get_source_files(backup_set)?;

        let plan = compute_backup_plan(self.store, backup_set.id, &source_files)?;

        let estimated_bytes: u64 = plan.new_files.iter().chain(plan.changed_files.iter()).map(|f| f.size_bytes).sum();

        if let Some(available) = validation.available_space_bytes {
            if available < estimated_bytes {
                return Err(BackupError::InsufficientSpace {
                    required: estimated_bytes,
                    available,
                });
            }
        }

        self.store.conn.execute(
            "UPDATE backup_runs SET files_total = ?1, files_new = ?2, files_changed = ?3 WHERE id = ?4",
            rusqlite::params![
                plan.total_files,
                plan.new_files.len() as u64,
                plan.changed_files.len() as u64,
                run_id
            ],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        let mut files_processed = 0u64;
        let mut bytes_transferred = 0u64;
        let mut files_verified = 0u64;
        let mut files_failed = 0u64;
        let mut copied_files: Vec<BackupFile> = Vec::new();

        let files_to_copy: Vec<&PlannedFile> = plan.new_files.iter().chain(plan.changed_files.iter()).collect();

        for planned in &files_to_copy {
            self.cancel_token.check()?;

            let rel_path = self.relative_path(backup_set, &planned.source_path);
            let dest_path = destination_root.join(&rel_path);
            let partial_path = dest_path.with_extension("fileharbor-partial");

            if let Some(parent) = dest_path.parent() {
                if let Err(_) = fs::create_dir_all(parent) {
                    files_failed += 1;
                    continue;
                }
            }

            match self.copy_file_atomic(Path::new(&planned.source_path), &partial_path) {
                Ok(copied_bytes) => {
                    match hasher::hash_file(&partial_path) {
                        Ok(computed_hash) => {
                            let expected_hash = planned.sha256.as_deref().unwrap_or("");
                            if computed_hash == expected_hash {
                                if let Err(_) = fs::rename(&partial_path, &dest_path) {
                                    let _ = fs::remove_file(&partial_path);
                                    files_failed += 1;
                                } else {
                                    bytes_transferred += copied_bytes;
                                    copied_files.push(BackupFile {
                                        id: 0,
                                        snapshot_id: 0,
                                        relative_path: rel_path,
                                        size_bytes: copied_bytes,
                                        sha256: computed_hash,
                                        status: BackupFileStatus::Pending,
                                        content_id: None,
                                        error: None,
                                    });
                                }
                            } else {
                                let _ = fs::remove_file(&partial_path);
                                files_failed += 1;
                            }
                        }
                        Err(_) => {
                            let _ = fs::remove_file(&partial_path);
                            files_failed += 1;
                        }
                    }
                }
                Err(_) => {
                    files_failed += 1;
                }
            }

            files_processed += 1;
            self.emit_progress(
                run_id,
                BackupRunStatus::Running,
                files_processed,
                plan.total_files,
                bytes_transferred,
                0,
                files_verified,
                files_failed,
                Some(planned.source_path.clone()),
            );
        }

        for planned in &plan.unavailable_files {
            files_processed += 1;
            self.emit_progress(
                run_id,
                BackupRunStatus::Running,
                files_processed,
                plan.total_files,
                bytes_transferred,
                0,
                files_verified,
                files_failed,
                Some(planned.source_path.clone()),
            );
        }

        for planned in &plan.unchanged_files {
            self.cancel_token.check()?;

            let rel_path = self.relative_path(backup_set, &planned.source_path);
            let dest_path = destination_root.join(&rel_path);

            if dest_path.exists() {
                if let Ok(computed_hash) = hasher::hash_file(&dest_path) {
                    let expected_hash = planned.sha256.as_deref().unwrap_or("");
                    if computed_hash == expected_hash {
                        copied_files.push(BackupFile {
                            id: 0,
                            snapshot_id: 0,
                            relative_path: rel_path,
                            size_bytes: planned.size_bytes,
                            sha256: computed_hash,
                            status: BackupFileStatus::Verified,
                            content_id: None,
                            error: None,
                        });
                        files_verified += 1;
                    } else {
                        files_failed += 1;
                    }
                } else {
                    files_failed += 1;
                }
            } else {
                files_failed += 1;
            }

            files_processed += 1;
            if files_processed % 100 == 0 || files_processed == plan.total_files {
                self.emit_progress(
                    run_id,
                    BackupRunStatus::Running,
                    files_processed,
                    plan.total_files,
                    bytes_transferred,
                    0,
                    files_verified,
                    files_failed,
                    Some(planned.source_path.clone()),
                );
            }
        }

        for file in &mut copied_files {
            if file.status == BackupFileStatus::Pending {
                file.status = BackupFileStatus::Verified;
                files_verified += 1;
            }
        }

        let snapshot_id = match self.create_snapshot(run_id, backup_set, destination_root, &plan, &copied_files, files_verified, files_failed) {
            Ok(id) => id,
            Err(e) => {
                self.store.conn.execute(
                    "UPDATE backup_runs SET status = ?1, finished_at_ms = ?2, error_summary = ?3 WHERE id = ?4",
                    rusqlite::params!["failed", now_ms(), e.to_string(), run_id],
                ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
                return Err(e);
            }
        };

        let total_duration = now_ms() - start_time;
        let total_duration_u64 = total_duration.max(0) as u64;
        let run_status = if files_failed == 0 && plan.unavailable_files.is_empty() {
            BackupRunStatus::Completed
        } else if files_verified > 0 {
            BackupRunStatus::Partial
        } else {
            BackupRunStatus::Failed
        };

        self.store.conn.execute(
            "UPDATE backup_runs SET status = ?1, finished_at_ms = ?2, snapshot_id = ?3, files_failed = ?4, bytes_transferred = ?5, bytes_verified = ?6 WHERE id = ?7",
            rusqlite::params![
                run_status.as_str(),
                now_ms(),
                snapshot_id,
                files_failed,
                bytes_transferred,
                bytes_transferred,
                run_id
            ],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        let _ = super::record_backup_completed(
            self.store,
            backup_set,
            snapshot_id,
            &BackupRunSummary {
                run_id,
                snapshot_id,
                status: run_status,
                files_total: plan.total_files,
                files_new: plan.new_files.len() as u64,
                files_changed: plan.changed_files.len() as u64,
                files_unchanged: plan.unchanged_files.len() as u64,
                files_deleted: plan.deleted_files.len() as u64,
                files_failed,
                bytes_transferred,
                bytes_verified: bytes_transferred,
                duration_ms: total_duration_u64,
            },
        );

        Ok(BackupRunSummary {
            run_id,
            snapshot_id,
            status: run_status,
            files_total: plan.total_files,
            files_new: plan.new_files.len() as u64,
            files_changed: plan.changed_files.len() as u64,
            files_unchanged: plan.unchanged_files.len() as u64,
            files_deleted: plan.deleted_files.len() as u64,
            files_failed,
            bytes_transferred,
            bytes_verified: bytes_transferred,
            duration_ms: total_duration_u64,
        })
    }

    fn get_source_files(&self, backup_set: &BackupSet) -> Result<Vec<FileRecord>, BackupError> {
        if let Ok(Some(session_id)) = self.store.find_latest_session_for_path(&backup_set.source_path) {
            if let Ok(files) = self.store.get_session_files(session_id) {
                return Ok(files);
            }
        }

        self.scan_directory(&backup_set.source_path)
    }

    fn scan_directory(&self, source_path: &str) -> Result<Vec<FileRecord>, BackupError> {
        let mut files = Vec::new();
        let root = Path::new(source_path);

        if !root.exists() {
            return Err(BackupError::SourceMissing(source_path.to_string()));
        }

        self.scan_dir_recursive(root, root, &mut files)?;
        Ok(files)
    }

    fn scan_dir_recursive(&self, root: &Path, dir: &Path, files: &mut Vec<FileRecord>) -> Result<(), BackupError> {
        self.cancel_token.check()?;

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let metadata = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if !metadata.is_file() {
                if metadata.is_dir() && !is_symlink(&path) {
                    self.scan_dir_recursive(root, &path, files)?;
                }
                continue;
            }

            let file_name = path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let extension = path.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());

            let size_bytes = metadata.len();

            let modified_at_ms = metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);

            let created_at_ms = metadata.created()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);

            let accessed_at_ms = metadata.accessed()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);

            let is_empty = size_bytes == 0;

            let mut hash = None;
            let mut hash_status = HashStatus::NotComputed;
            let mut scan_status = ScanStatus::Completed;

            if !is_empty {
                match hasher::sha256_file(&path, &self.cancel_token, hasher::DEFAULT_CHUNK_SIZE) {
                    Ok(hasher::HashOutcome::Computed(h)) => {
                        hash = Some(h);
                        hash_status = HashStatus::Computed;
                    }
                    Ok(hasher::HashOutcome::Unstable) => {
                        hash_status = HashStatus::Unstable;
                        scan_status = ScanStatus::ChangedDuringScan;
                    }
                    Ok(hasher::HashOutcome::Failed(_)) => {
                        hash_status = HashStatus::Failed;
                        scan_status = ScanStatus::HashFailed;
                    }
                    Err(_) => {
                        hash_status = HashStatus::Failed;
                        scan_status = ScanStatus::HashFailed;
                    }
                }
            }

            let path_str = path.to_string_lossy().to_string();

            files.push(FileRecord {
                id: 0,
                session_id: 0,
                path: path_str.clone(),
                file_name,
                extension,
                size_bytes,
                created_at_ms,
                modified_at_ms,
                accessed_at_ms,
                is_empty,
                hash,
                hash_algorithm: Some(hasher::HASH_ALGORITHM.to_string()),
                hash_status,
                scan_status,
                error: None,
            });
        }

        Ok(())
    }

    fn relative_path(&self, backup_set: &BackupSet, source_path: &str) -> String {
        let source_buf = Path::new(&backup_set.source_path);
        let file_buf = Path::new(source_path);
        match file_buf.strip_prefix(source_buf) {
            Ok(rel) => rel.to_string_lossy().to_string(),
            Err(_) => file_buf.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
        }
    }

    fn copy_file_atomic(&self, src: &Path, dest: &Path) -> Result<u64, BackupError> {
        let mut src_file = File::open(src)
            .map_err(|e| BackupError::TransferFailed(e.to_string()))?;

        let mut dest_file = File::create(dest)
            .map_err(|e| BackupError::TransferFailed(e.to_string()))?;

        let mut buf = vec![0u8; 256 * 1024];
        let mut total: u64 = 0;

        loop {
            self.cancel_token.check()?;
            let n = src_file.read(&mut buf)
                .map_err(|e| BackupError::TransferFailed(e.to_string()))?;
            if n == 0 {
                break;
            }
            dest_file.write_all(&buf[..n])
                .map_err(|e| BackupError::TransferFailed(e.to_string()))?;
            total += n as u64;
        }

        dest_file.sync_all()
            .map_err(|e| BackupError::TransferFailed(e.to_string()))?;

        Ok(total)
    }

    fn create_snapshot(
        &self,
        run_id: i64,
        backup_set: &BackupSet,
        destination_root: &Path,
        plan: &BackupPlan,
        copied_files: &[BackupFile],
        files_verified: u64,
        files_failed: u64,
    ) -> Result<i64, BackupError> {
        let snapshot_label = format!("snapshot-{}", format_timestamp(now_ms()));
        let total_size: u64 = copied_files.iter().map(|f| f.size_bytes).sum();
        let now = now_ms();

        let snapshot_status = if files_failed == 0 && plan.unavailable_files.is_empty() {
            BackupSnapshotStatus::Verified
        } else if files_verified > 0 {
            BackupSnapshotStatus::Unverified
        } else {
            BackupSnapshotStatus::Corrupted
        };

        self.store.conn.execute(
            "INSERT INTO backup_snapshots (backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                backup_set.id,
                run_id,
                snapshot_label,
                snapshot_status.as_str(),
                now,
                copied_files.len() as u64,
                total_size
            ],
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        let snapshot_id = self.store.conn.last_insert_rowid();

        for file in copied_files {
            self.store.conn.execute(
                "INSERT INTO backup_files (snapshot_id, relative_path, size_bytes, sha256, status, content_id, error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    snapshot_id,
                    file.relative_path,
                    file.size_bytes,
                    file.sha256,
                    file.status.as_str(),
                    file.content_id,
                    file.error
                ],
            ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        }

        let manifest = BackupManifest {
            schema_version: 1,
            backup_set_id: backup_set.id.to_string(),
            snapshot_id: snapshot_id.to_string(),
            created_at: format_timestamp_iso(now),
            status: snapshot_status.as_str().to_string(),
            source: ManifestSource {
                label: backup_set.name.clone(),
                path: backup_set.source_path.clone(),
            },
            files: copied_files.iter().map(|f| ManifestFile {
                relative_path: f.relative_path.clone(),
                size_bytes: f.size_bytes,
                sha256: f.sha256.clone(),
                verified: matches!(f.status, BackupFileStatus::Verified),
            }).collect(),
        };

        write_manifest(&manifest, destination_root, &snapshot_label)?;

        Ok(snapshot_id)
    }

    fn emit_progress(
        &self,
        run_id: i64,
        status: BackupRunStatus,
        files_processed: u64,
        files_total: u64,
        bytes_transferred: u64,
        bytes_verified: u64,
        files_verified: u64,
        files_failed: u64,
        current_file: Option<String>,
    ) {
        if let Some(callback) = &self.progress_callback {
            let progress = BackupProgress {
                run_id,
                status,
                current_file,
                files_processed,
                files_total,
                bytes_transferred,
                bytes_total: bytes_transferred,
                bytes_verified,
                files_verified,
                files_failed,
                speed_bytes_per_sec: 0.0,
                eta_seconds: None,
            };
            callback(progress);
        }
    }
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn format_timestamp(ms: i64) -> String {
    let dt = match Utc.timestamp_millis_opt(ms) {
        chrono::LocalResult::Single(dt) => dt,
        _ => Utc.timestamp_millis_opt(0).unwrap(),
    };
    format!("{}{:03}", dt.format("%Y-%m-%dT%H%M%S"), ms % 1000)
}

fn format_timestamp_iso(ms: i64) -> String {
    let dt = match Utc.timestamp_millis_opt(ms) {
        chrono::LocalResult::Single(dt) => dt,
        _ => Utc.timestamp_millis_opt(0).unwrap(),
    };
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
