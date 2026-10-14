//! Staged, cancellable scan pipeline.
//!
//! Phases:
//!   1. Discovering + reading metadata (single streaming traversal)
//!   2. Finding duplicate candidates (group by size in the database)
//!   3. Hashing candidates (parallel streaming SHA-256)
//!   4. Building duplicate groups
//!
//! Safety rules enforced here:
//! - Only regular files are processed; symlinks are recorded as skipped.
//! - The quarantine area itself is always excluded from scanning.
//! - A failure on one file never aborts the scan.
//! - Cancellation leaves partial results persisted and usable.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::cancel::CancelToken;
use crate::dupdetect;
use crate::fs_layer::{self, EntryKind};
use crate::hasher;
use crate::logutil;
use crate::models::*;
use crate::store::Store;

pub const DISCOVERY_BATCH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    Discovering,
    FindingCandidates,
    Hashing,
    BuildingResults,
    Done,
}

impl ScanPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanPhase::Discovering => "discovering",
            ScanPhase::FindingCandidates => "finding_candidates",
            ScanPhase::Hashing => "hashing",
            ScanPhase::BuildingResults => "building_results",
            ScanPhase::Done => "done",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanProgress {
    pub session_id: i64,
    pub phase: ScanPhase,
    pub files_discovered: u64,
    pub files_processed: u64,
    pub candidates_total: u64,
    pub candidates_hashed: u64,
    pub errors: u64,
    pub current_path: String,
}

pub struct ScannerConfig {
    pub roots: Vec<PathBuf>,
    /// User-configured exclusions (absolute paths).
    pub exclusions: Vec<PathBuf>,
    /// Application quarantine root; always excluded automatically.
    pub quarantine_root: Option<PathBuf>,
    pub max_hash_concurrency: usize,
    /// Automatically shield sensitive credential directories (.ssh, .aws, etc.)
    pub privacy_shield_enabled: bool,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            exclusions: Vec::new(),
            quarantine_root: None,
            max_hash_concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            privacy_shield_enabled: true,
        }
    }
}

/// Run a full scan for an already-created session id.
///
/// Returns the final [`SessionStatus`] (Completed or Cancelled). This
/// function is designed to run on a background thread; progress is
/// reported through `progress` and cancellation is cooperative via
/// `cancel`.
pub fn run_scan(
    store: &Store,
    cfg: &ScannerConfig,
    session_id: i64,
    cancel: &CancelToken,
    progress: &(dyn Fn(&ScanProgress) + Send + Sync),
) -> SessionStatus {
    logutil::info(
        "scan_started",
        &[("session", &session_id.to_string()), ("roots", &format!("{:?}", cfg.roots))],
    );

    let mut excl: Vec<String> = cfg.exclusions.iter().map(|p| fs_layer::normalize_exclusion(p)).collect();
    if let Some(q) = &cfg.quarantine_root {
        excl.push(fs_layer::normalize_exclusion(q));
    }

    let counters = Arc::new(ProgressCounters::default());
    let status = phase_discovery(store, cfg, &excl, session_id, cancel, progress, &counters);

    let final_status = if cancel.is_cancelled() {
        SessionStatus::Cancelled
    } else if matches!(status, SessionStatus::Failed) {
        SessionStatus::Failed
    } else {
        // Analysis phases only run when discovery completed uncancelled.
        match run_analysis(store, cfg, session_id, cancel, progress, Some(&counters)) {
            Ok(_) => SessionStatus::Completed,
            Err(AnalysisError::Cancelled) => SessionStatus::Cancelled,
            Err(AnalysisError::Fatal(e)) => {
                logutil::error("analysis_failed", &[("error", &e)]);
                SessionStatus::Failed
            }
        }
    };

    let _ = store.finish_session(session_id, final_status);
    progress.emit_phase(session_id, ScanPhase::Done, "", &counters, 0, 0);
    logutil::info(
        "scan_finished",
        &[("session", &session_id.to_string()), ("status", final_status.as_str())],
    );
    final_status
}

trait EmitProgress {
    fn emit_phase(
        &self,
        session_id: i64,
        phase: ScanPhase,
        current_path: &str,
        counters: &ProgressCounters,
        candidates_total: u64,
        candidates_hashed: u64,
    );
}

impl EmitProgress for &(dyn Fn(&ScanProgress) + Send + Sync) {
    fn emit_phase(
        &self,
        session_id: i64,
        phase: ScanPhase,
        current_path: &str,
        counters: &ProgressCounters,
        candidates_total: u64,
        candidates_hashed: u64,
    ) {
        self(&ScanProgress {
            session_id,
            phase,
            files_discovered: counters.discovered.load(Ordering::Relaxed) as u64,
            files_processed: counters.processed.load(Ordering::Relaxed) as u64,
            candidates_total,
            candidates_hashed,
            errors: counters.errors.load(Ordering::Relaxed) as u64,
            current_path: current_path.to_string(),
        });
    }
}

#[derive(Default)]
pub struct ProgressCounters {
    pub discovered: AtomicUsize,
    pub processed: AtomicUsize,
    pub errors: AtomicUsize,
}


fn determine_copy_role(path: &Path, backup_roots: &[String]) -> CopyRole {
    let path_str = path.to_string_lossy().to_lowercase();
    for b_root in backup_roots {
        let b_lower = b_root.trim_end_matches(['/', '\\']).to_lowercase();
        if path_str == b_lower || (path_str.starts_with(&b_lower) && matches!(path_str.as_bytes().get(b_lower.len()), Some(b'\\') | Some(b'/'))) {
            return CopyRole::Backup;
        }
    }
    let mut current = path.parent();
    while let Some(parent) = current {
        if parent.join(".fileharbor").exists() {
            return CopyRole::Backup;
        }
        current = parent.parent();
    }
    CopyRole::Working
}

fn phase_discovery(
    store: &Store,
    cfg: &ScannerConfig,
    exclusions: &[String],
    session_id: i64,
    cancel: &CancelToken,
    progress: &(dyn Fn(&ScanProgress) + Send + Sync),
    counters: &Arc<ProgressCounters>,
) -> SessionStatus {
    let mut batch: Vec<NewFileRecord> = Vec::with_capacity(DISCOVERY_BATCH);
    let mut stack: Vec<PathBuf> = cfg.roots.clone();
    let backup_destinations = store.get_all_backup_destination_paths();

    while let Some(dir) = stack.pop() {
        if cancel.is_cancelled() {
            flush_batch(store, &mut batch);
            return SessionStatus::Cancelled;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                counters.errors.fetch_add(1, Ordering::Relaxed);
                logutil::warn("read_dir_failed", &[("path", &dir.display().to_string()), ("error", &e.to_string())]);
                continue;
            }
        };

        let mut names: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        names.sort();
        // Reverse push so pop() yields alphabetical order.
        for path in names.into_iter().rev() {
            if cancel.is_cancelled() {
                flush_batch(store, &mut batch);
                return SessionStatus::Cancelled;
            }
            
            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if file_name == ".fileharbor" || file_name == ".fileharbor-partial" {
                continue;
            }

            if fs_layer::is_excluded(&path, exclusions) {
                continue;
            }

            if cfg.privacy_shield_enabled && crate::privacy::is_sensitive_path(&path) {
                continue;
            }
            let info = match fs_layer::inspect(&path) {
                Ok(i) => i,
                Err(f) => {
                    record_error(store, session_id, &path, &f.message);
                    counters.errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            };
            match info.kind {
                EntryKind::Directory => stack.push(path),
                EntryKind::Symlink => {
                    batch.push(NewFileRecord {
                        session_id,
                        path: path_str(&path),
                        file_name: file_name_of(&path),
                        extension: None,
                        size_bytes: 0,
                        created_at_ms: None,
                        modified_at_ms: None,
                        accessed_at_ms: None,
                        is_empty: false,
                        scan_status: ScanStatus::Skipped,
                        error: Some("symbolic link skipped by policy".to_string()),
                        copy_role: CopyRole::Working,
                    });
                    counters.processed.fetch_add(1, Ordering::Relaxed);
                }
                EntryKind::Other => {
                    counters.errors.fetch_add(1, Ordering::Relaxed);
                }
                EntryKind::RegularFile => {
                    counters.discovered.fetch_add(1, Ordering::Relaxed);
                    let is_empty = info.size_bytes == 0;
                    let copy_role = determine_copy_role(&info.path, &backup_destinations);
                    batch.push(NewFileRecord {
                        session_id,
                        path: path_str(&info.path),
                        file_name: file_name_of(&info.path),
                        extension: extension_of(&info.path),
                        size_bytes: info.size_bytes,
                        created_at_ms: info.created_ms,
                        modified_at_ms: info.modified_ms,
                        accessed_at_ms: info.accessed_ms,
                        is_empty,
                        scan_status: ScanStatus::Completed,
                        error: None,
                        copy_role,
                    });
                    counters.processed.fetch_add(1, Ordering::Relaxed);
                    progress(
                        &ScanProgress {
                            session_id,
                            phase: ScanPhase::Discovering,
                            files_discovered: counters.discovered.load(Ordering::Relaxed) as u64,
                            files_processed: counters.processed.load(Ordering::Relaxed) as u64,
                            candidates_total: 0,
                            candidates_hashed: 0,
                            errors: counters.errors.load(Ordering::Relaxed) as u64,
                            current_path: path_str(&path),
                        },
                    );
                    if batch.len() >= DISCOVERY_BATCH {
                        flush_batch(store, &mut batch);
                    }
                }
            }
        }
    }
    flush_batch(store, &mut batch);
    SessionStatus::Completed
}

fn record_error(store: &Store, session_id: i64, path: &Path, message: &str) {
    let _ = store.insert_file_records(&[NewFileRecord {
        session_id,
        path: path_str(path),
        file_name: file_name_of(path),
        extension: extension_of(path),
        size_bytes: 0,
        created_at_ms: None,
        modified_at_ms: None,
        accessed_at_ms: None,
        is_empty: false,
        scan_status: ScanStatus::Inaccessible,
        error: Some(message.to_string()),
        copy_role: CopyRole::Working,
    }]);
}

fn flush_batch(store: &Store, batch: &mut Vec<NewFileRecord>) {
    if !batch.is_empty() {
        let taken: Vec<NewFileRecord> = std::mem::take(batch);
        if let Err(e) = store.insert_file_records(&taken) {
            logutil::error("db_insert_failed", &[("error", &e)]);
        }
    }
}

pub enum AnalysisError {
    Cancelled,
    Fatal(String),
}

/// Re-run candidate finding, hashing and grouping using existing file
/// records (no filesystem re-walk). Also used for "retry analysis".
pub fn run_analysis(
    store: &Store,
    _cfg: &ScannerConfig,
    session_id: i64,
    cancel: &CancelToken,
    progress: &(dyn Fn(&ScanProgress) + Send + Sync),
    counters: Option<&ProgressCounters>,
) -> Result<(), AnalysisError> {
    let (total_discovered, total_processed, total_errors) = if let Some(c) = counters {
        (
            c.discovered.load(Ordering::Relaxed) as u64,
            c.processed.load(Ordering::Relaxed) as u64,
            c.errors.load(Ordering::Relaxed) as u64,
        )
    } else {
        let count = store.count_files(session_id).unwrap_or(0);
        (count, count, 0)
    };

    // Phase 2: find same-size candidates entirely in SQL.
    progress(&ScanProgress {
        session_id,
        phase: ScanPhase::FindingCandidates,
        files_discovered: total_discovered,
        files_processed: total_processed,
        candidates_total: 0,
        candidates_hashed: 0,
        errors: total_errors,
        current_path: "Analyzing duplicate file sizes...".to_string(),
    });

    let sizes = store.candidate_sizes(session_id).map_err(AnalysisError::Fatal)?;
    let candidates = store
        .files_with_sizes(session_id, &sizes)
        .map_err(AnalysisError::Fatal)?;

    let to_hash: Vec<FileRecord> = candidates
        .iter()
        .filter(|f| f.hash_status == HashStatus::NotComputed || f.hash_status == HashStatus::Pending)
        .cloned()
        .collect();

    let total_candidates = to_hash.len() as u64;
    let chunk_size = hasher::DEFAULT_CHUNK_SIZE;

    enum Work {
        Cached(String),
        Compute(FileRecord),
    }
    let work_items: Vec<(i64, Work)> = to_hash
        .iter()
        .map(|rec| {
            match store.hash_cache_get(&rec.path, rec.size_bytes, rec.modified_at_ms) {
                Some(cached) => (rec.id, Work::Cached(cached)),
                None => (rec.id, Work::Compute(rec.clone())),
            }
        })
        .collect();

    let compute_items: Vec<&(i64, Work)> =
        work_items.iter().filter(|(_, w)| matches!(w, Work::Compute(_))).collect();
    let total_compute = compute_items.len();
    let total_cached = work_items.len().saturating_sub(total_compute);

    let hashed_count = Arc::new(AtomicUsize::new(total_cached));

    // Phase 3: Hash candidate files
    progress(&ScanProgress {
        session_id,
        phase: ScanPhase::Hashing,
        files_discovered: total_discovered,
        files_processed: total_processed,
        candidates_total: total_candidates,
        candidates_hashed: total_cached as u64,
        errors: total_errors,
        current_path: if total_compute > 0 {
            "Starting parallel hash computation...".to_string()
        } else {
            "Candidate hashes retrieved from cache.".to_string()
        },
    });

    let last_progress_emit = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));

    let outcomes: Vec<(i64, hasher::HashOutcome)> = compute_items
        .par_iter()
        .map(|(file_id, work)| {
            let rec = match work {
                Work::Compute(r) => r,
                Work::Cached(_) => unreachable!(),
            };
            if cancel.is_cancelled() {
                return (*file_id, hasher::HashOutcome::Failed("__cancelled__".into()));
            }
            let outcome = match hasher::sha256_file(Path::new(&rec.path), cancel, chunk_size) {
                Ok(o) => o,
                Err(_) => hasher::HashOutcome::Failed("__cancelled__".into()),
            };

            let done = hashed_count.fetch_add(1, Ordering::Relaxed) + 1;
            let should_emit = if done % 8 == 0 || done == (total_cached + total_compute) {
                if let Ok(mut last) = last_progress_emit.lock() {
                    if last.elapsed().as_millis() >= 80 || done == (total_cached + total_compute) {
                        *last = std::time::Instant::now();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if should_emit {
                progress(&ScanProgress {
                    session_id,
                    phase: ScanPhase::Hashing,
                    files_discovered: total_discovered,
                    files_processed: total_processed,
                    candidates_total: total_candidates,
                    candidates_hashed: done as u64,
                    errors: total_errors,
                    current_path: rec.path.clone(),
                });
            }

            (*file_id, outcome)
        })
        .collect();

    let mut cancelled = false;
    for (file_id, outcome) in outcomes {
        match outcome {
            hasher::HashOutcome::Computed(hash) => {
                if store
                    .update_file_hash_result(
                        file_id,
                        Some(&hash),
                        HashStatus::Computed,
                        ScanStatus::Completed,
                        None,
                    )
                    .is_ok()
                {
                    if let Ok(rec) = store.get_file(file_id) {
                        store.hash_cache_put(&rec.path, rec.size_bytes, rec.modified_at_ms, &hash);
                    }
                }
            }
            hasher::HashOutcome::Unstable => {
                let _ = store.update_file_hash_result(
                    file_id,
                    None,
                    HashStatus::Unstable,
                    ScanStatus::ChangedDuringScan,
                    Some("the file changed while it was being read"),
                );
            }
            hasher::HashOutcome::Failed(msg) => {
                if msg == "__cancelled__" {
                    cancelled = true;
                } else {
                    let _ = store.update_file_hash_result(
                        file_id,
                        None,
                        HashStatus::Failed,
                        ScanStatus::HashFailed,
                        Some(&msg),
                    );
                }
            }
        }
    }

    // Persist cached-hash hits without touching the filesystem again.
    for (file_id, work) in &work_items {
        if let Work::Cached(hash) = work {
            let _ = store.update_file_hash_result(
                *file_id,
                Some(hash),
                HashStatus::Computed,
                ScanStatus::Completed,
                None,
            );
        }
    }

    let final_hashed = hashed_count.load(Ordering::Relaxed) as u64;
    progress(&ScanProgress {
        session_id,
        phase: ScanPhase::Hashing,
        files_discovered: total_discovered,
        files_processed: total_processed,
        candidates_total: total_candidates,
        candidates_hashed: final_hashed,
        errors: total_errors,
        current_path: String::new(),
    });

    if cancel.is_cancelled() || cancelled {
        return Err(AnalysisError::Cancelled);
    }

    // Phase 4: build groups.
    progress(&ScanProgress {
        session_id,
        phase: ScanPhase::BuildingResults,
        files_discovered: total_discovered,
        files_processed: total_processed,
        candidates_total: total_candidates,
        candidates_hashed: final_hashed,
        errors: total_errors,
        current_path: "Grouping duplicates and calculating space savings...".to_string(),
    });

    dupdetect::build_groups(store, session_id).map_err(AnalysisError::Fatal)?;
    Ok(())
}

// ----------------------------------------------------------------------
// Shared helpers
// ----------------------------------------------------------------------

pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

pub fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string_lossy().to_string())
}

pub fn extension_of(p: &Path) -> Option<String> {
    let name = p.file_name()?.to_string_lossy();
    if name.starts_with('.') {
        return None; // dotfiles like .gitkeep have no meaningful extension
    }
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
}

