//! Quarantine service.
//!
//! Moves user-selected files into the application-managed quarantine
//! area instead of deleting them. Every operation is recorded in the
//! operation history and in per-batch manifest files so restores are
//! always possible.
//!
//! Transfer safety (cross-volume moves):
//!   rename → on failure copy to destination → verify destination
//!   exists and matches expected size (+ hash when known) → only then
//!   remove the original. If any verification fails the original is
//!   kept and the failure is reported.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cancel::CancelToken;
use crate::hasher;
use crate::logutil;
use crate::models::*;
use crate::ops;
use crate::store::Store;

#[derive(Debug, Clone, Serialize)]
pub struct QuarantineBatchResult {
    pub report: BatchReport,
    pub quarantined_item_ids: Vec<i64>,
}

/// Sanitize a filename for safe reuse inside the quarantine directory.
pub fn sanitize_filename(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect();
    // Windows reserved device names.
    let upper = cleaned.to_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    if matches!(
        stem,
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    ) {
        cleaned = format!("_{cleaned}");
    }
    while cleaned.ends_with('.') || cleaned.ends_with(' ') {
        cleaned.pop();
    }
    if cleaned.is_empty() {
        cleaned = "_unnamed".to_string();
    }
    cleaned
}

/// Move the given file records into quarantine as one batch operation.
///
/// Files that fail keep their original location untouched and are
/// reported individually.
pub fn quarantine_files(
    store: &Store,
    file_ids: &[i64],
    quarantine_root: &Path,
    cancel: &CancelToken,
) -> Result<QuarantineBatchResult, String> {
    if file_ids.is_empty() {
        return Ok(QuarantineBatchResult {
            report: BatchReport { operation_id: -1, batch_id: String::new(), succeeded: 0, failed: 0, failures: vec![] },
            quarantined_item_ids: vec![],
        });
    }

    let batch_id = ops::new_batch_id("quarantine");
    logutil::info("quarantine_started", &[("batch", &batch_id), ("count", &file_ids.len().to_string())]);

    let op_id = store.insert_operation(&batch_id, OperationKind::Quarantine)?;
    let batch_dir = quarantine_root.join(&batch_id);
    fs::create_dir_all(&batch_dir)
        .map_err(|e| format!("unable to create quarantine directory {}: {e}", batch_dir.display()))?;

    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<FailedItem> = Vec::new();
    let mut item_ids: Vec<i64> = Vec::new();
    let mut manifest_items: Vec<ManifestEntry> = Vec::new();

    for (seq, file_id) in file_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            failed += file_ids.len() - seq;
            break;
        }
        let record = match store.get_file(*file_id) {
            Ok(r) => r,
            Err(e) => {
                failed += 1;
                failures.push(FailedItem { path: format!("#{file_id}"), error: e });
                continue;
            }
        };
        let item_row = store.insert_operation_item(
            op_id,
            Some(&record.path),
            None,
            Some(record.id),
            OpItemStatus::InProgress,
            None,
        )?;

        let src = PathBuf::from(&record.path);
        let safe_name = sanitize_filename(&record.file_name);
        let dst = batch_dir.join(format!("{:06}_{safe_name}", seq + 1));

        match transfer(&src, &dst, record.size_bytes, record.hash.as_deref()) {
            Ok(()) => {
                let qpath = path_to_string(&dst);
                let qid = store.insert_quarantine_item(
                    op_id,
                    &record.path,
                    &qpath,
                    &record.file_name,
                    record.size_bytes,
                    record.hash.as_deref(),
                )?;
                store.update_operation_item(item_row, OpItemStatus::Completed, Some(&qpath), None)?;
                manifest_items.push(ManifestEntry {
                    original_path: record.path.clone(),
                    quarantine_path: qpath,
                    size_bytes: record.size_bytes,
                    hash: record.hash.clone(),
                });
                succeeded += 1;
                item_ids.push(qid);
                logutil::info("file_quarantined", &[("from", &record.path)]);
            }
            Err(err) => {
                failed += 1;
                failures.push(FailedItem { path: record.path.clone(), error: err.clone() });
                store.update_operation_item(item_row, OpItemStatus::Failed, None, Some(&err))?;
                logutil::warn("quarantine_failed", &[("path", &record.path), ("error", &err)]);
            }
        }
    }

    let status = if cancel.is_cancelled() {
        if succeeded > 0 {
            OperationStatus::PartiallyCompleted
        } else {
            OperationStatus::Cancelled
        }
    } else if failed == 0 {
        OperationStatus::Completed
    } else if succeeded > 0 {
        OperationStatus::PartiallyCompleted
    } else {
        OperationStatus::Failed
    };
    store.finish_operation(op_id, status, Some(&serde_detail(succeeded, failed)))?;

    write_manifest(&batch_dir, &batch_id, &manifest_items);

    logutil::info("quarantine_finished", &[("succeeded", &succeeded.to_string()), ("failed", &failed.to_string())]);
    Ok(QuarantineBatchResult {
        report: BatchReport { operation_id: op_id, batch_id, succeeded, failed, failures },
        quarantined_item_ids: item_ids,
    })
}

/// Move arbitrary filesystem paths into quarantine as one batch operation.
///
/// This does not require prior file records in the database; it queries
/// filesystem metadata, computes hashes, creates operations and quarantine items,
/// and writes the batch manifest.
pub fn quarantine_paths(
    store: &Store,
    paths: &[PathBuf],
    quarantine_root: &Path,
    cancel: &CancelToken,
) -> Result<QuarantineBatchResult, String> {
    if paths.is_empty() {
        return Ok(QuarantineBatchResult {
            report: BatchReport { operation_id: -1, batch_id: String::new(), succeeded: 0, failed: 0, failures: vec![] },
            quarantined_item_ids: vec![],
        });
    }

    let batch_id = ops::new_batch_id("quarantine");
    logutil::info("quarantine_paths_started", &[("batch", &batch_id), ("count", &paths.len().to_string())]);

    let op_id = store.insert_operation(&batch_id, OperationKind::Quarantine)?;
    let batch_dir = quarantine_root.join(&batch_id);
    fs::create_dir_all(&batch_dir)
        .map_err(|e| format!("unable to create quarantine directory {}: {e}", batch_dir.display()))?;

    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<FailedItem> = Vec::new();
    let mut item_ids: Vec<i64> = Vec::new();
    let mut manifest_items: Vec<ManifestEntry> = Vec::new();

    for (seq, src_path) in paths.iter().enumerate() {
        if cancel.is_cancelled() {
            failed += paths.len() - seq;
            break;
        }

        let path_str = path_to_string(src_path);
        if !src_path.is_file() {
            failed += 1;
            failures.push(FailedItem {
                path: path_str,
                error: "file does not exist or is not a regular file".to_string(),
            });
            continue;
        }

        let meta = match fs::metadata(src_path) {
            Ok(m) => m,
            Err(e) => {
                failed += 1;
                failures.push(FailedItem { path: path_str, error: e.to_string() });
                continue;
            }
        };
        let size_bytes = meta.len();
        let file_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown_file").to_string();

        let token = CancelToken::new();
        let hash = match hasher::sha256_file(src_path, &token, hasher::DEFAULT_CHUNK_SIZE) {
            Ok(hasher::HashOutcome::Computed(h)) => Some(h),
            _ => None,
        };

        let item_row = store.insert_operation_item(
            op_id,
            Some(&path_str),
            None,
            None,
            OpItemStatus::InProgress,
            None,
        )?;

        let safe_name = sanitize_filename(&file_name);
        let dst = batch_dir.join(format!("{:06}_{safe_name}", seq + 1));

        match transfer(src_path, &dst, size_bytes, hash.as_deref()) {
            Ok(()) => {
                let qpath = path_to_string(&dst);
                let qid = store.insert_quarantine_item(
                    op_id,
                    &path_str,
                    &qpath,
                    &file_name,
                    size_bytes,
                    hash.as_deref(),
                )?;
                store.update_operation_item(item_row, OpItemStatus::Completed, Some(&qpath), None)?;
                manifest_items.push(ManifestEntry {
                    original_path: path_str.clone(),
                    quarantine_path: qpath,
                    size_bytes,
                    hash: hash.clone(),
                });
                succeeded += 1;
                item_ids.push(qid);
                logutil::info("file_quarantined", &[("from", &path_str)]);
            }
            Err(err) => {
                failed += 1;
                failures.push(FailedItem { path: path_str.clone(), error: err.clone() });
                store.update_operation_item(item_row, OpItemStatus::Failed, None, Some(&err))?;
                logutil::warn("quarantine_failed", &[("path", &path_str), ("error", &err)]);
            }
        }
    }

    let status = if cancel.is_cancelled() {
        if succeeded > 0 {
            OperationStatus::PartiallyCompleted
        } else {
            OperationStatus::Cancelled
        }
    } else if failed == 0 {
        OperationStatus::Completed
    } else if succeeded > 0 {
        OperationStatus::PartiallyCompleted
    } else {
        OperationStatus::Failed
    };
    store.finish_operation(op_id, status, Some(&serde_detail(succeeded, failed)))?;

    write_manifest(&batch_dir, &batch_id, &manifest_items);

    logutil::info("quarantine_paths_finished", &[("succeeded", &succeeded.to_string()), ("failed", &failed.to_string())]);
    Ok(QuarantineBatchResult {
        report: BatchReport { operation_id: op_id, batch_id, succeeded, failed, failures },
        quarantined_item_ids: item_ids,
    })
}

fn serde_detail(succeeded: usize, failed: usize) -> String {
    format!(r#"{{"succeeded":{succeeded},"failed":{failed}}}"#)
}

// ----------------------------------------------------------------------
// Verified transfer primitive (shared by quarantine and restore)
// ----------------------------------------------------------------------

/// Move `src` to `dst`, verifying integrity before removing the source.
/// On any verification failure the source is left untouched.
pub fn transfer(
    src: &Path,
    dst: &Path,
    expected_size: u64,
    expected_hash: Option<&str>,
) -> Result<(), String> {
    if !src.is_file() {
        return Err("the source file no longer exists at its original location".to_string());
    }
    if dst.exists() {
        return Err("destination already exists".to_string());
    }

    // Fast path: same-volume rename.
    if fs::rename(src, dst).is_ok() {
        return verify_and_finish(src, dst, expected_size, expected_hash);
    }

    // Cross-volume fallback: copy → verify → delete original.
    if let Err(e) = fs::copy(src, dst) {
        let _ = fs::remove_file(dst);
        return Err(format!("unable to copy this file: {e}"));
    }

    // Verify destination before touching the original.
    match verify_copy(dst, expected_size, expected_hash) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_file(dst); // remove unverified partial copy
            return Err(err);
        }
    }

    if let Err(e) = fs::remove_file(src) {
        let _ = fs::remove_file(dst);
        return Err(format!(
            "the copy succeeded but the original could not be removed, so it was kept: {e}"
        ));
    }
    Ok(())
}

fn verify_and_finish(src: &Path, dst: &Path, expected_size: u64, expected_hash: Option<&str>) -> Result<(), String> {
    match verify_copy(dst, expected_size, expected_hash) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Roll back: put things back the way they were.
            let _ = fs::rename(dst, src);
            Err(err)
        }
    }
}

fn verify_copy(dst: &Path, expected_size: u64, expected_hash: Option<&str>) -> Result<(), String> {
    let meta = fs::metadata(dst).map_err(|e| format!("verification failed: {e}"))?;
    if meta.len() != expected_size {
        return Err(format!(
            "verification failed: copied size {} does not match expected {expected_size}",
            meta.len()
        ));
    }
    if let Some(hash) = expected_hash {
        let token = CancelToken::new();
        match hasher::sha256_file(dst, &token, hasher::DEFAULT_CHUNK_SIZE) {
            Ok(hasher::HashOutcome::Computed(actual)) if actual != hash => {
                return Err("verification failed: content hash of the copy does not match the original".to_string());
            }
            Ok(_) => {}
            Err(_) => return Err("verification interrupted".to_string()),
        }
    }
    Ok(())
}

pub fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

#[derive(Serialize)]
struct ManifestEntry {
    original_path: String,
    quarantine_path: String,
    size_bytes: u64,
    hash: Option<String>,
}

#[derive(Serialize)]
struct Manifest<'a> {
    batch_id: &'a str,
    created_at_ms: i64,
    items: &'a [ManifestEntry],
}

fn write_manifest(batch_dir: &Path, batch_id: &str, items: &[ManifestEntry]) {
    let manifest = Manifest {
        batch_id,
        created_at_ms: crate::store::now_ms(),
        items,
    };
    let path = batch_dir.join("manifest.json");
    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = fs::write(path, json);
    }
}

