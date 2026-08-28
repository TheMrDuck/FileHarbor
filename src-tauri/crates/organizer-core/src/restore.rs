//! Restore service: returns quarantined files to their original locations.
//!
//! Conflict policy: an existing file at the original path is NEVER
//! overwritten. The caller resolves conflicts explicitly via
//! [`RestoreResolution`].

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::logutil;
use crate::models::*;
use crate::ops;
use crate::quarantine::{path_to_string, sanitize_filename, transfer};
use crate::store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreResolution {
    /// Do nothing; keep the existing file at the original location.
    KeepExisting,
    /// Restore next to the original under a non-conflicting name.
    RestoreWithNewName,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestorePreview {
    pub item_id: i64,
    pub original_path: String,
    pub quarantine_path: String,
    pub file_exists_at_original: bool,
}

/// Inspect which restores would conflict without touching anything.
pub fn preview_restore(store: &Store, item_ids: &[i64]) -> Result<Vec<RestorePreview>, String> {
    let mut out = Vec::with_capacity(item_ids.len());
    for id in item_ids {
        let item = store.get_quarantine_item(*id)?;
        out.push(RestorePreview {
            item_id: item.id,
            original_path: item.original_path.clone(),
            quarantine_path: item.quarantine_path.clone(),
            file_exists_at_original: Path::new(&item.original_path).exists(),
        });
    }
    Ok(out)
}

pub struct RestoreBatchResult {
    pub report: BatchReport,
    pub kept_existing: Vec<String>,
}

/// Restore quarantined items. Items whose original location is occupied
/// are handled according to `default_resolution`; `None` treats them as
/// failures (conflict) and leaves everything untouched.
pub fn restore_items(
    store: &Store,
    item_ids: &[i64],
    default_resolution: Option<RestoreResolution>,
) -> Result<RestoreBatchResult, String> {
    let batch_id = ops::new_batch_id("restore");
    logutil::info("restore_started", &[("batch", &batch_id), ("count", &item_ids.len().to_string())]);

    let op_id = store.insert_operation(&batch_id, OperationKind::Restore)?;
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<FailedItem> = Vec::new();
    let mut kept_existing: Vec<String> = Vec::new();

    for id in item_ids {
        let item = match store.get_quarantine_item(*id) {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                failures.push(FailedItem { path: format!("#{id}"), error: e });
                continue;
            }
        };
        let item_row = store.insert_operation_item(
            op_id,
            Some(&item.quarantine_path),
            None,
            None,
            OpItemStatus::InProgress,
            None,
        )?;

        let src = PathBuf::from(&item.quarantine_path);
        let original = PathBuf::from(&item.original_path);

        // Recreate missing parent directories (safe: directories only).
        if let Some(parent) = original.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                let err = format!("the original folder could not be recreated: {e}");
                failed += 1;
                failures.push(FailedItem { path: item.original_path.clone(), error: err.clone() });
                store.update_operation_item(item_row, OpItemStatus::Failed, None, Some(&err))?;
                continue;
            }
        }

        let destination: PathBuf;
        if original.exists() {
            match default_resolution {
                Some(RestoreResolution::KeepExisting) => {
                    kept_existing.push(item.original_path.clone());
                    store.update_operation_item(
                        item_row,
                        OpItemStatus::Skipped,
                        None,
                        Some("existing file at original location was kept"),
                    )?;
                    continue;
                }
                Some(RestoreResolution::RestoreWithNewName) => {
                    destination = sibling_new_name(&original);
                }
                None => {
                    let err =
                        "a file already exists at the original location".to_string();
                    failed += 1;
                    failures.push(FailedItem { path: item.original_path.clone(), error: err.clone() });
                    store.update_operation_item(item_row, OpItemStatus::Failed, None, Some(&err))?;
                    continue;
                }
            }
        } else {
            destination = original.clone();
        }

        match transfer(&src, &destination, item.size_bytes, item.hash.as_deref()) {
            Ok(()) => {
                store.update_operation_item(
                    item_row,
                    OpItemStatus::Completed,
                    Some(&path_to_string(&destination)),
                    None,
                )?;
                store.quarantine_set_status(item.id, QuarantineItemStatus::Restored)?;
                succeeded += 1;
                logutil::info("file_restored", &[("to", &path_to_string(&destination))]);
            }
            Err(err) => {
                failed += 1;
                failures.push(FailedItem { path: item.original_path.clone(), error: err.clone() });
                store.update_operation_item(item_row, OpItemStatus::Failed, None, Some(&err))?;
                logutil::warn("restore_failed", &[("path", &item.original_path), ("error", &err)]);
            }
        }
    }

    let status = if failed == 0 {
        OperationStatus::Completed
    } else if succeeded > 0 || !kept_existing.is_empty() {
        OperationStatus::PartiallyCompleted
    } else {
        OperationStatus::Failed
    };
    store.finish_operation(op_id, status, None)?;

    logutil::info("restore_finished", &[("succeeded", &succeeded.to_string()), ("failed", &failed.to_string())]);
    Ok(RestoreBatchResult {
        report: BatchReport { operation_id: op_id, batch_id, succeeded, failed, failures },
        kept_existing,
    })
}

/// Generate a non-conflicting sibling name like "report (restored).pdf".
fn sibling_new_name(original: &Path) -> PathBuf {
    let dir = original.parent().unwrap_or_else(|| Path::new("."));
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = original
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    for n in 1..10_000u32 {
        let candidate_name = if ext.is_empty() {
            format!("{stem} (restored{n})")
        } else {
            format!("{stem} (restored{n}).{ext}")
        };
        let candidate = dir.join(sanitize_filename(&candidate_name));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(sanitize_filename(&format!(
        "{stem} (restored {}).{}",
        crate::store::now_ms(),
        if ext.is_empty() { "bin".to_string() } else { ext }
    )))
}
