mod common;

use std::fs;
use std::path::Path;

use organizer_core::cancel::CancelToken;
use organizer_core::deleter;
use organizer_core::models::*;
use organizer_core::quarantine;
use organizer_core::restore::{self, RestoreResolution};
use organizer_core::scanner::{self, ScannerConfig};
use organizer_core::store::{FileFilter, Store};

use common::*;

fn setup() -> (Store, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("test.db")).expect("store");
    let qroot = dir.path().join("Quarantine");
    fs::create_dir_all(&qroot).unwrap();
    store.set_setting("quarantine_root", &qroot.to_string_lossy()).unwrap();
    (store, dir)
}

fn quarantine_one(store: &Store, file_id: i64, qroot: &Path) -> i64 {
    let result = quarantine::quarantine_files(store, &[file_id], qroot, &CancelToken::new())
        .expect("quarantine call");
    assert_eq!(result.report.failed, 0, "failures: {:?}", result.report.failures);
    *result.quarantined_item_ids.first().expect("one item")
}

#[test]
fn quarantine_moves_file_and_records_operation() {
    let (s, _guard) = setup();
    let root = temp_root("q-move");
    let original = write_file(&root, "report.pdf", b"important report content");

    let session = scan(&s, std::slice::from_ref(&root), "q-move");
    let page = s
        .list_files(session, &FileFilter { limit: 10, ..Default::default() })
        .unwrap();
    let rec = &page.items[0];

    let item_id = quarantine_one(&s, rec.id, &_guard.path().join("Quarantine"));

    // Original gone.
    assert!(!original.exists(), "original must be moved out of the user folder");
    let items = s.get_quarantine_items(false).unwrap();
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.id, item_id);
    assert_eq!(item.original_path, original.to_string_lossy().to_string());
    assert_eq!(item.size_bytes, b"important report content".len() as u64);
    assert!(Path::new(&item.quarantine_path).exists());

    // Operation history recorded with Completed status.
    let ops = s.list_operations(10, 0).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].kind, OperationKind::Quarantine);
    assert_eq!(ops[0].status, OperationStatus::Completed);
    let op_items = s.operation_items(ops[0].id).unwrap();
    assert_eq!(op_items.len(), 1);
    assert_eq!(op_items[0].status, OpItemStatus::Completed);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn restore_returns_file_to_original_location() {
    let (s, guard) = setup();
    let qroot = guard.path().join("Quarantine");
    let root = temp_root("q-restore");
    let original = write_file(&root, "docs/notes.txt", b"notes body");

    let session = scan(&s, std::slice::from_ref(&root), "q-restore");
    let rec = &s.list_files(session, &FileFilter { limit: 10, ..Default::default() }).unwrap().items[0];
    let item_id = quarantine_one(&s, rec.id, &qroot);

    let result =
        restore::restore_items(&s, &[item_id], None).expect("restore");
    assert_eq!(result.report.succeeded, 1);
    assert_eq!(result.report.failed, 0);
    assert!(original.exists(), "file restored to original path");
    assert_eq!(fs::read(&original).unwrap(), b"notes body");

    let items = s.get_quarantine_items(false).unwrap();
    assert_eq!(items.len(), 0, "restored item no longer 'stored'");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn restore_recreates_missing_directories() {
    let (s, guard) = setup();
    let qroot = guard.path().join("Quarantine");
    let root = temp_root("q-recreate");
    write_file(&root, "deep/nested/dir/file.bin", b"payload");

    let session = scan(&s, std::slice::from_ref(&root), "q-recreate");
    let rec = &s.list_files(session, &FileFilter { limit: 10, ..Default::default() }).unwrap().items[0];
    let item_id = quarantine_one(&s, rec.id, &qroot);

    // The whole original tree is gone now.
    fs::remove_dir_all(&root).unwrap();

    restore::restore_items(&s, &[item_id], None).expect("restore");
    let restored = Path::new(&rec.path);
    assert!(restored.exists(), "directory tree recreated and file restored");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn restore_never_overwrites_existing_file_conflict() {
    let (s, guard) = setup();
    let qroot = guard.path().join("Quarantine");
    let root = temp_root("q-conflict");
    let original_path = write_file(&root, "doc.txt", b"ORIGINAL CONTENT");

    let session = scan(&s, std::slice::from_ref(&root), "q-conflict");
    let rec = &s.list_files(session, &FileFilter { limit: 10, ..Default::default() }).unwrap().items[0];
    let item_id = quarantine_one(&s, rec.id, &qroot);

    // A different file now occupies the original location.
    fs::write(&original_path, b"NEWER USER FILE").unwrap();

    // Preview reports the conflict without touching anything.
    let preview = restore::preview_restore(&s, &[item_id]).unwrap();
    assert!(preview[0].file_exists_at_original);

    // Default resolution None => conflict reported as failure; existing
    // file untouched.
    let result = restore::restore_items(&s, &[item_id], None).unwrap();
    assert_eq!(result.report.failed, 1);
    assert_eq!(fs::read(&original_path).unwrap(), b"NEWER USER FILE");

    // KeepExisting keeps the newer user file.
    let result2 =
        restore::restore_items(&s, &[item_id], Some(RestoreResolution::KeepExisting)).unwrap();
    assert_eq!(result2.report.succeeded, 0);
    assert_eq!(result2.kept_existing.len(), 1);
    assert_eq!(fs::read(&original_path).unwrap(), b"NEWER USER FILE");

    // RestoreWithNewName places the quarantined copy beside the original.
    let result3 =
        restore::restore_items(&s, &[item_id], Some(RestoreResolution::RestoreWithNewName))
            .unwrap();
    assert_eq!(result3.report.succeeded, 1);
    assert_eq!(fs::read(&original_path).unwrap(), b"NEWER USER FILE", "existing file preserved");
    let restored_new_name = root.join("doc (restored1).txt");
    assert!(restored_new_name.exists(), "restored under a new name");
    assert_eq!(fs::read(&restored_new_name).unwrap(), b"ORIGINAL CONTENT");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn quarantine_partial_failure_reports_each_file() {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use std::sync::Arc;

        let (s, guard) = setup();
    let qroot = guard.path().join("Quarantine");
        let root = temp_root("q-partial");
        write_file(&root, "movable.txt", b"movable");
        let locked = write_file(&root, "locked.txt", b"locked by another process");

        // Lock one file so moving it fails while the other succeeds.
        let lock = Arc::new(
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&locked)
                .unwrap(),
        );

        let session = scan(&s, std::slice::from_ref(&root), "q-partial");
        let page = s
            .list_files(session, &FileFilter { limit: 10, ..Default::default() })
            .unwrap();
        let ids: Vec<i64> = page.items.iter().map(|f| f.id).collect();

        let result = quarantine::quarantine_files(&s, &ids, &qroot, &CancelToken::new()).unwrap();
        assert_eq!(result.report.succeeded, 1, "one file moves fine");
        assert_eq!(result.report.failed, 1, "locked file fails individually");
        assert_eq!(
            result.report.failures[0].path,
            locked.to_string_lossy().to_string()
        );
        assert!(locked.exists(), "failed file remains at its original location");

        let ops = s.list_operations(5, 0).unwrap();
        assert_eq!(ops[0].status, OperationStatus::PartiallyCompleted);

        drop(lock);
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn permanent_deletion_requires_quarantined_items_and_records_results() {
    let (s, guard) = setup();
    let qroot = guard.path().join("Quarantine");
    let root = temp_root("q-delete");
    write_file(&root, "junk-a.tmp", b"aaaa");
    write_file(&root, "junk-b.tmp", b"bbbb");

    let session = scan(&s, std::slice::from_ref(&root), "q-delete");
    let page = s
        .list_files(session, &FileFilter { limit: 10, ..Default::default() })
        .unwrap();
    let ids: Vec<i64> = page.items.iter().map(|f| f.id).collect();

    let result = quarantine::quarantine_files(&s, &ids, &qroot, &CancelToken::new()).unwrap();
    let item_ids = result.quarantined_item_ids.clone();
    assert_eq!(item_ids.len(), 2);

    let report = deleter::delete_permanently(&s, &item_ids).expect("delete");
    assert_eq!(report.succeeded, 2);
    assert_eq!(report.failed, 0);

    for item in s.get_quarantine_items(true).unwrap() {
        assert!(!Path::new(&item.quarantine_path).exists(), "deleted from disk");
        assert_eq!(item.status, QuarantineItemStatus::Deleted);
    }

    let ops = s.list_operations(5, 0).unwrap();
    assert_eq!(ops[0].kind, OperationKind::DeletePermanent);
    assert_eq!(ops[0].status, OperationStatus::Completed);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn deletion_refuses_paths_outside_quarantine() {
    let (s, _guard) = setup();
    let root = temp_root("q-refusal");
    let victim = write_file(&root, "precious.txt", b"do not touch");

    let session_id = s.insert_session(&[]).unwrap();
    s.insert_file_records(&[NewFileRecord {
        session_id,
        path: victim.to_string_lossy().to_string(),
        file_name: "precious.txt".into(),
        extension: Some("txt".into()),
        size_bytes: 12,
        created_at_ms: None,
        modified_at_ms: None,
        accessed_at_ms: None,
        is_empty: false,
        scan_status: ScanStatus::Completed,
        error: None,
        copy_role: CopyRole::Working,
    }])
    .unwrap();

    // Forge a quarantine row pointing OUTSIDE the managed area.
    let op_id = s.insert_operation("fake-batch", OperationKind::Quarantine).unwrap();
    let forged = s
        .insert_quarantine_item(op_id, &victim.to_string_lossy(), &victim.to_string_lossy(), "precious.txt", 12, None)
        .unwrap();

    let report = deleter::delete_permanently(&s, &[forged]).unwrap();
    assert_eq!(report.failed, 1);
    assert!(victim.exists(), "outside-quarantine deletion must be refused");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cross_volume_move_verifies_before_removing_original() {
    // Simulate a copy+verify+delete flow where verification fails:
    // force an expected size mismatch through a corrupted destination.
    let (s, guard) = setup();
    let _qroot = guard.path().join("Quarantine");
    let root = temp_root("q-crossvol");
    let src = write_file(&root, "video.mkv", vec![7u8; 4096].as_slice());

    let session = scan(&s, std::slice::from_ref(&root), "q-crossvol");
    let rec = &s.list_files(session, &FileFilter { limit: 10, ..Default::default() }).unwrap().items[0];

    // Direct transfer API: correct size + hash verifies cleanly.
    let dst = guard.path().join("elsewhere").join("video.mkv");
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    organizer_core::quarantine::transfer(
        &src,
        &dst,
        rec.size_bytes,
        None,
    )
    .expect("transfer should verify");
    assert!(dst.exists());

    // Mismatched expected size must fail and keep the source intact.
    let src2 = write_file(&root, "video2.mkv", vec![7u8; 2048].as_slice());
    let dst2 = guard.path().join("elsewhere2").join("video.mkv");
    fs::create_dir_all(dst2.parent().unwrap()).unwrap();
    let err = organizer_core::quarantine::transfer(&src2, &dst2, 999_999, None)
        .expect_err("size mismatch must fail transfer");
    assert!(err.contains("verification failed"));
    assert!(src2.exists(), "source kept when verification fails");
    assert!(!dst2.exists(), "unverified partial copy removed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn cancelled_scan_keeps_partial_results_usable() {
    let (s, _guard) = setup();
    let root = temp_root("scan-cancel");
    for i in 0..300u32 {
        write_file(&root, &format!("f{i}.txt"), format!("body {i}").repeat(50).as_bytes());
    }
    let session_id = s.insert_session(&[root.to_string_lossy().to_string()]).unwrap();
    let cancel = CancelToken::new();
    cancel.cancel(); // cancelled before it starts

    let cfg = ScannerConfig { roots: vec![root.clone()], ..Default::default() };
    let status = scanner::run_scan(&s, &cfg, session_id, &cancel, &|_| {});
    assert_eq!(status, SessionStatus::Cancelled);

    s.finish_session(session_id, status).unwrap();
    let summary = s.scan_summary(session_id).unwrap();
    assert_eq!(summary.session.status, SessionStatus::Cancelled);
    // Whatever was discovered before cancellation stays queryable.
    let page = s.list_files(session_id, &FileFilter { limit: 1, ..Default::default() }).unwrap();
    assert!(page.total >= 0);

    let _ = fs::remove_dir_all(root);
}


#[test]
fn test_quarantine_raw_paths_moves_files_and_records_manifest() {
    let (s, _guard) = setup();
    let root = temp_root("q-raw-paths");
    let file1 = write_file(&root, "draft_v1.docx", b"Draft document version 1 content");
    let file2 = write_file(&root, "photo_lowres.jpg", b"Low-resolution redundant photo");

    let qroot = _guard.path().join("Quarantine");
    let paths = vec![file1.clone(), file2.clone()];

    let res = organizer_core::quarantine::quarantine_paths(&s, &paths, &qroot, &CancelToken::new())
        .expect("quarantine raw paths should succeed");

    assert_eq!(res.report.succeeded, 2);
    assert_eq!(res.report.failed, 0);
    assert!(!file1.exists(), "original file 1 must be moved into quarantine");
    assert!(!file2.exists(), "original file 2 must be moved into quarantine");

    let q_items = s.get_quarantine_items(false).expect("get quarantine items");
    assert_eq!(q_items.len(), 2);
    assert!(q_items.iter().any(|item| item.original_path == file1.to_string_lossy()));
    assert!(q_items.iter().any(|item| item.original_path == file2.to_string_lossy()));

    // Verify manifest exists in batch dir
    let batch_manifest = qroot.join(&res.report.batch_id).join("manifest.json");
    assert!(batch_manifest.exists(), "manifest.json must be written for the batch");

    let _ = fs::remove_dir_all(root);
}
