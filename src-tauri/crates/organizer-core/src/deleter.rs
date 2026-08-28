//! Permanent deletion service.
//!
//! SAFETY INVARIANT: this module only ever deletes files that are
//! currently inside the application quarantine area (QuarantineItems).
//! It has no ability to delete arbitrary user paths. Confirmation is
//! enforced in the UI layer; the core records every deletion.

use std::fs;

use crate::logutil;
use crate::models::*;
use crate::ops;
use crate::store::Store;

/// Permanently delete quarantined files after explicit user confirmation.
/// The confirmation itself is the UI's responsibility; this function
/// assumes it was obtained and focuses on safe execution + recording.
pub fn delete_permanently(store: &Store, item_ids: &[i64]) -> Result<BatchReport, String> {
    let batch_id = ops::new_batch_id("delete");
    logutil::info(
        "deletion_requested",
        &[("batch", &batch_id), ("count", &item_ids.len().to_string())],
    );

    let op_id = store.insert_operation(&batch_id, OperationKind::DeletePermanent)?;
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<FailedItem> = Vec::new();

    for id in item_ids {
        let item = match store.get_quarantine_item(*id) {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                failures.push(FailedItem { path: format!("#{id}"), error: e });
                continue;
            }
        };
        // Defense-in-depth: refuse to delete paths outside quarantine.
        if !is_inside_quarantine(store, &item.quarantine_path) {
            let err = "refused: path is not inside the managed quarantine area".to_string();
            failed += 1;
            failures.push(FailedItem { path: item.original_path.clone(), error: err });
            continue;
        }

        let item_row = store.insert_operation_item(
            op_id,
            Some(&item.quarantine_path),
            None,
            None,
            OpItemStatus::InProgress,
            None,
        )?;

        match fs::remove_file(&item.quarantine_path) {
            Ok(()) => {
                store.update_operation_item(item_row, OpItemStatus::Completed, None, None)?;
                store.quarantine_set_status(item.id, QuarantineItemStatus::Deleted)?;
                succeeded += 1;
                logutil::info("file_deleted_permanently", &[("quarantine_path", &item.quarantine_path)]);
            }
            Err(e) => {
                let err = format!("unable to delete this file: {e}");
                failed += 1;
                failures.push(FailedItem { path: item.original_path.clone(), error: err.clone() });
                store.update_operation_item(item_row, OpItemStatus::Failed, None, Some(&err))?;
                logutil::warn("deletion_failed", &[("path", &item.quarantine_path), ("error", &e.to_string())]);
            }
        }
    }

    let detail = format!(r#"{{"succeeded":{succeeded},"failed":{failed}}}"#);
    let status = if failed == 0 {
        OperationStatus::Completed
    } else if succeeded > 0 {
        OperationStatus::PartiallyCompleted
    } else {
        OperationStatus::Failed
    };
    store.finish_operation(op_id, status, Some(&detail))?;

    logutil::info("deletion_completed", &[("succeeded", &succeeded.to_string()), ("failed", &failed.to_string())]);
    Ok(BatchReport { operation_id: op_id, batch_id, succeeded, failed, failures })
}

/// The quarantine root is stored as a setting by the app layer; verify
/// the path lives under it.
fn is_inside_quarantine(store: &Store, quarantine_path: &str) -> bool {
    let Some(root) = store.get_setting("quarantine_root") else {
        return false;
    };
    let root = root.trim_end_matches(['/', '\\']).to_lowercase();
    let p = quarantine_path.to_lowercase();
    p.starts_with(&root)
        && matches!(p.as_bytes().get(root.len()), Some(b'\\') | Some(b'/'))
}
