use organizer_core::backup::*;
use organizer_core::backup::engine::BackupEngine;
use organizer_core::backup::manifest::{BackupManifest, ManifestSource, ManifestFile};
use organizer_core::backup::retention::{apply_retention, RetentionResult};
use organizer_core::backup::test_helpers::{create_test_backup_run, create_test_backup_snapshot};
use organizer_core::cancel::CancelToken;
use organizer_core::store::Store;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

fn test_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn setup_test_db() -> (Store, PathBuf) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();
    (store, dir.path().to_path_buf())
}

#[test]
fn test_create_and_list_backup_set() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let set = create_backup_set(
        &store,
        "Test Backup",
        "/source/path",
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Daily,
        RetentionPolicy::KeepLastN(5),
        VerificationMode::Standard,
    ).unwrap();
    
    assert_eq!(set.name, "Test Backup");
    assert_eq!(set.source_path, "/source/path");
    assert_eq!(set.schedule, BackupSchedule::Daily);
    assert!(matches!(set.retention_policy, RetentionPolicy::KeepLastN(5)));
    
    let sets = list_backup_sets(&store).unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].id, set.id);
}

#[test]
fn test_backup_destination_validation() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let validation = destination::validate_destination(
        std::path::Path::new("/source"),
        dest_dir.path(),
    );
    assert!(validation.valid);
    
    let dest = destination::ensure_destination_identity(
        &store,
        dest_dir.path(),
        Some("Test Dest".to_string()),
    ).unwrap();
    assert!(!dest.destination_id.is_empty());
    assert_eq!(dest.path, dest_dir.path().to_str().unwrap());
    assert!(dest.filesystem_id.is_some());
    
    // Second validation should return same destination_id
    let dest2 = destination::ensure_destination_identity(
        &store,
        dest_dir.path(),
        Some("Test Dest".to_string()),
    ).unwrap();
    assert_eq!(dest.destination_id, dest2.destination_id);
}

#[test]
fn test_backup_runs_crud() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let set = create_backup_set(
        &store,
        "Test Backup",
        "/source",
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepAll,
        VerificationMode::Standard,
    ).unwrap();
    
    let runs = get_backup_runs(&store, set.id, 10).unwrap();
    assert_eq!(runs.len(), 0);
}

#[test]
fn test_backup_snapshots_crud() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let set = create_backup_set(
        &store,
        "Test Backup",
        "/source",
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepAll,
        VerificationMode::Standard,
    ).unwrap();
    
    let snapshots = get_backup_snapshots(&store, set.id).unwrap();
    assert_eq!(snapshots.len(), 0);
}

#[test]
fn test_retention_policy_keep_last_n() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let set = create_backup_set(
        &store,
        "Test Backup",
        "/source",
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepLastN(2),
        VerificationMode::Standard,
    ).unwrap();
    
    // Create snapshots using test helpers
    let run_id = create_test_backup_run(&store, set.id, BackupRunStatus::Completed);
    
    // Insert 3 snapshots with increasing timestamps
    let base_time = test_now_ms();
    let _snap1 = create_test_backup_snapshot(&store, set.id, run_id, "snap-1", "verified", Some(base_time));
    let snap2 = create_test_backup_snapshot(&store, set.id, run_id, "snap-2", "verified", Some(base_time + 1000));
    let snap3 = create_test_backup_snapshot(&store, set.id, run_id, "snap-3", "verified", Some(base_time + 2000));
    
    // Apply retention - should keep last 2
    let result: RetentionResult = apply_retention(&store, set.id, &set.retention_policy).unwrap();
    assert_eq!(result.deleted_snapshots, 1);
    
    // snap1 should be deleted, snap2 and snap3 should remain
    let snapshots = get_backup_snapshots(&store, set.id).unwrap();
    assert_eq!(snapshots.len(), 2);
    assert!(snapshots.iter().any(|s| s.id == snap2));
    assert!(snapshots.iter().any(|s| s.id == snap3));
}

#[test]
fn test_retention_policy_keep_all() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let set = create_backup_set(
        &store,
        "Test Backup",
        "/source",
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepAll,
        VerificationMode::Standard,
    ).unwrap();
    
    let run_id = create_test_backup_run(&store, set.id, BackupRunStatus::Completed);
    
    // Create multiple snapshots
    let base_time = test_now_ms();
    for i in 1..=3 {
        create_test_backup_snapshot(&store, set.id, run_id, &format!("snap-{}", i), "verified", Some(base_time + i as i64 * 1000));
    }
    
    // Apply retention - should keep all
    let result: RetentionResult = apply_retention(&store, set.id, &set.retention_policy).unwrap();
    assert_eq!(result.deleted_snapshots, 0);
    
    let snapshots = get_backup_snapshots(&store, set.id).unwrap();
    assert_eq!(snapshots.len(), 3);
}

#[test]
fn test_manifest_write_and_read() {
    let (_store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let manifest_data = BackupManifest {
        schema_version: 1,
        backup_set_id: "1".to_string(),
        snapshot_id: "1".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        status: "verified".to_string(),
        source: ManifestSource {
            label: "Test".to_string(),
            path: "/source".to_string(),
        },
        files: vec![
            ManifestFile {
                relative_path: "test.txt".to_string(),
                size_bytes: 100,
                sha256: "abc123".to_string(),
                verified: true,
                content_object: None,
            },
        ],
    };
    
    manifest::write_manifest(&manifest_data, dest_dir.path(), "snapshot-1").unwrap();
    let read_manifest = manifest::read_manifest(dest_dir.path(), "snapshot-1").unwrap();
    
    assert_eq!(read_manifest.schema_version, 1);
    assert_eq!(read_manifest.files.len(), 1);
    assert_eq!(read_manifest.files[0].relative_path, "test.txt");
}

#[test]
fn test_backup_health_computation() {
    let (store, _temp) = setup_test_db();
    let dest_dir = tempdir().unwrap();
    
    let set = create_backup_set(
        &store,
        "Test Backup",
        "/source",
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepAll,
        VerificationMode::Standard,
    ).unwrap();
    
    let health = get_backup_health(&store).unwrap();
    assert_eq!(health.sets.len(), 1);
    assert_eq!(health.sets[0].backup_set_id, set.id);
    assert_eq!(health.sets[0].status, BackupSetStatus::Active);
}

#[test]
fn test_backup_file_status_enum() {
    // Test serialization/deserialization of backup file status
    let status = BackupFileStatus::Verified;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"verified\"");
    
    let parsed: BackupFileStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, BackupFileStatus::Verified);
}

#[test]
fn test_backup_schedule_serialization() {
    let schedule = BackupSchedule::Daily;
    let json = serde_json::to_string(&schedule).unwrap();
    assert_eq!(json, "\"daily\"");
    
    let parsed: BackupSchedule = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, BackupSchedule::Daily);
}

#[test]
fn test_retention_policy_serialization() {
    let policy = RetentionPolicy::KeepLastN(5);
    let json = serde_json::to_string(&policy).unwrap();
    // The enum serializes as an object with the variant name as key
    assert_eq!(json, "{\"keep_last_n\":5}");
    
    let parsed: RetentionPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, RetentionPolicy::KeepLastN(5));
    
    // Also test KeepAll
    let keep_all = RetentionPolicy::KeepAll;
    let json = serde_json::to_string(&keep_all).unwrap();
    assert_eq!(json, "\"keep_all\"");
}

#[test]
fn test_backup_set_status_variants() {
    assert_eq!(BackupSetStatus::Active.as_str(), "active");
    assert_eq!(BackupSetStatus::Outdated.as_str(), "outdated");
    assert_eq!(BackupSetStatus::AtRisk.as_str(), "at_risk");
    assert_eq!(BackupSetStatus::Paused.as_str(), "paused");
    assert_eq!(BackupSetStatus::Disabled.as_str(), "disabled");
    
    assert_eq!(BackupSetStatus::parse("active"), BackupSetStatus::Active);
    assert_eq!(BackupSetStatus::parse("outdated"), BackupSetStatus::Outdated);
    assert_eq!(BackupSetStatus::parse("at_risk"), BackupSetStatus::AtRisk);
    assert_eq!(BackupSetStatus::parse("paused"), BackupSetStatus::Paused);
    assert_eq!(BackupSetStatus::parse("disabled"), BackupSetStatus::Disabled);
}

#[test]
fn test_engine_full_backup_creates_verified_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    fs::write(source_dir.join("report.pdf"), b"report content").unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "Documents", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::default(), VerificationMode::Standard).unwrap();

    let sets = list_backup_sets(&store).unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].id, set.id);

    eprintln!("set.id = {}", set.id);
    let direct = get_backup_set(&store, set.id).unwrap();
    eprintln!("direct = {:?}", direct.is_some());

    let engine = BackupEngine::new(&store);
    let summary = engine.run_backup(&set, &dest_dir).unwrap();

    assert_eq!(summary.status, BackupRunStatus::Completed);
    assert_eq!(summary.files_total, 1);
    assert_eq!(summary.files_new, 1);
    assert_eq!(summary.files_failed, 0);
    assert_eq!(summary.snapshot_id, 1);
    let pool_dir = dest_dir.join(".fileharbor").join("pool");
    assert!(pool_dir.exists());

    // Verify restoration directly from pool
    let restore_dir = tmp.path().join("restored");
    let res = restore::restore_from_backup(&store, summary.snapshot_id, &restore_dir, &[]).unwrap();
    assert_eq!(res.restored, 1);
    assert_eq!(res.verified, 1);
    assert!(restore_dir.join("report.pdf").exists());
    assert_eq!(fs::read(restore_dir.join("report.pdf")).unwrap(), b"report content");

    let snapshots = get_backup_snapshots(&store, set.id).unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].status, BackupSnapshotStatus::Verified);
    assert_eq!(snapshots[0].files_count, 1);
}

#[test]
fn test_engine_incremental_backup_reuses_unchanged_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    fs::write(source_dir.join("a.txt"), b"hello").unwrap();
    fs::write(source_dir.join("b.txt"), b"world").unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "Docs", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::default(), VerificationMode::Standard).unwrap();

    let engine = BackupEngine::new(&store);
    let s1 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s1.status, BackupRunStatus::Completed);
    assert_eq!(s1.files_new, 2);

    fs::write(source_dir.join("b.txt"), b"world updated").unwrap();

    let s2 = engine.run_backup(&set, &dest_dir).unwrap();
    eprintln!("s2: new={}, changed={}, unchanged={}, deleted={}", s2.files_new, s2.files_changed, s2.files_unchanged, s2.files_deleted);
    assert_eq!(s2.status, BackupRunStatus::Completed);
    assert_eq!(s2.files_new, 0);
    assert_eq!(s2.files_changed, 1);
    assert_eq!(s2.files_unchanged, 1);
    assert_eq!(s2.files_failed, 0);
}

#[test]
fn test_engine_backup_respects_cancellation() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    for i in 0..50 {
        fs::write(source_dir.join(format!("file_{i}.bin")), vec![0u8; 1024]).unwrap();
    }

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "Many", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::default(), VerificationMode::Standard).unwrap();

    let token = CancelToken::new();
    let engine = BackupEngine::new(&store)
        .with_cancel_token(token.clone());

    token.cancel();
    let result = engine.run_backup(&set, &dest_dir);
    assert!(matches!(result, Err(BackupError::Cancelled)));
}

#[test]
fn test_engine_backup_handles_deleted_files() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    fs::write(source_dir.join("keep.txt"), b"keep").unwrap();
    fs::write(source_dir.join("remove.txt"), b"remove").unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "Del", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::default(), VerificationMode::Standard).unwrap();

    let engine = BackupEngine::new(&store);
    let s1 = engine.run_backup(&set, &dest_dir).unwrap();
    eprintln!("s1: total={}, new={}, changed={}, unchanged={}, deleted={}", s1.files_total, s1.files_new, s1.files_changed, s1.files_unchanged, s1.files_deleted);
    assert_eq!(s1.files_total, 2);

    fs::remove_file(source_dir.join("remove.txt")).unwrap();

    let s2 = engine.run_backup(&set, &dest_dir).unwrap();
    eprintln!("s2: total={}, new={}, changed={}, unchanged={}, deleted={}", s2.files_total, s2.files_new, s2.files_changed, s2.files_unchanged, s2.files_deleted);
    assert_eq!(s2.files_total, 2);
    assert_eq!(s2.files_deleted, 1);
    assert_eq!(s2.files_new, 0);
    assert_eq!(s2.files_changed, 0);
    assert_eq!(s2.files_unchanged, 1);
}

fn count_pool_objects(dest_dir: &std::path::Path) -> usize {
    let pool_dir = dest_dir.join(".fileharbor").join("pool");
    if !pool_dir.exists() {
        return 0;
    }
    let mut count = 0;
    if let Ok(shards) = fs::read_dir(&pool_dir) {
        for shard in shards.flatten() {
            if shard.path().is_dir() {
                if let Ok(objs) = fs::read_dir(shard.path()) {
                    for obj in objs.flatten() {
                        if obj.path().is_file() {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    count
}

#[test]
fn test_historical_version_restoration() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    let file_path = source_dir.join("document.txt");
    fs::write(&file_path, b"Version 1 content").unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "Docs", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::KeepAll, VerificationMode::Standard).unwrap();

    let engine = BackupEngine::new(&store);
    let s1 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s1.status, BackupRunStatus::Completed);

    // Modify source file to Version 2
    fs::write(&file_path, b"Version 2 modified content").unwrap();

    let s2 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s2.status, BackupRunStatus::Completed);

    // Both versions must exist in the pool
    assert_eq!(count_pool_objects(&dest_dir), 2);

    // Restore Snapshot 1 -> must yield Version 1
    let restore_s1 = tmp.path().join("restore_s1");
    let r1 = restore::restore_from_backup(&store, s1.snapshot_id, &restore_s1, &[]).unwrap();
    assert_eq!(r1.restored, 1);
    assert_eq!(r1.verified, 1);
    assert_eq!(fs::read(restore_s1.join("document.txt")).unwrap(), b"Version 1 content");

    // Restore Snapshot 2 -> must yield Version 2
    let restore_s2 = tmp.path().join("restore_s2");
    let r2 = restore::restore_from_backup(&store, s2.snapshot_id, &restore_s2, &[]).unwrap();
    assert_eq!(r2.restored, 1);
    assert_eq!(r2.verified, 1);
    assert_eq!(fs::read(restore_s2.join("document.txt")).unwrap(), b"Version 2 modified content");
}

#[test]
fn test_same_content_multi_snapshot_deduplication() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "Dedup", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::KeepLastN(1), VerificationMode::Standard).unwrap();
    let engine = BackupEngine::new(&store);

    // Snapshot 1: A.txt = "hello"
    fs::write(source_dir.join("A.txt"), b"hello").unwrap();
    let s1 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s1.status, BackupRunStatus::Completed);
    assert_eq!(count_pool_objects(&dest_dir), 1);

    // Snapshot 2: B.txt = "hello" (same content, different filename)
    fs::remove_file(source_dir.join("A.txt")).unwrap();
    fs::write(source_dir.join("B.txt"), b"hello").unwrap();
    let s2 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s2.status, BackupRunStatus::Completed);
    // Dedup: pool still contains exactly 1 physical object!
    assert_eq!(count_pool_objects(&dest_dir), 1);

    // Snapshot 3: C.txt = "hello"
    fs::remove_file(source_dir.join("B.txt")).unwrap();
    fs::write(source_dir.join("C.txt"), b"hello").unwrap();
    let s3 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s3.status, BackupRunStatus::Completed);
    assert_eq!(count_pool_objects(&dest_dir), 1);

    // Apply retention KeepLastN(1) -> retains only S3
    let ret_res = apply_retention(&store, set.id, &RetentionPolicy::KeepLastN(1)).unwrap();
    assert_eq!(ret_res.deleted_snapshots, 2);

    // Pool object for "hello" must still remain because S3 references it!
    assert_eq!(count_pool_objects(&dest_dir), 1);

    // Restore S3 to verify it remains 100% restorable
    let restore_dir = tmp.path().join("restore_s3");
    let res = restore::restore_from_backup(&store, s3.snapshot_id, &restore_dir, &[]).unwrap();
    assert_eq!(res.restored, 1);
    assert_eq!(res.verified, 1);
    assert_eq!(fs::read(restore_dir.join("C.txt")).unwrap(), b"hello");
}

#[test]
fn test_complex_retention_and_survivability() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "ComplexRetention", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::KeepAll, VerificationMode::Standard).unwrap();
    let engine = BackupEngine::new(&store);

    // Snapshot 1: report.pdf = A
    fs::write(source_dir.join("report.pdf"), b"content A").unwrap();
    let s1 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s1.status, BackupRunStatus::Completed);

    // Snapshot 2: report.pdf = B, photo.jpg = X
    fs::write(source_dir.join("report.pdf"), b"content B").unwrap();
    fs::write(source_dir.join("photo.jpg"), b"content X").unwrap();
    let s2 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s2.status, BackupRunStatus::Completed);

    // Snapshot 3: report.pdf = C, photo.jpg = X (unchanged)
    fs::write(source_dir.join("report.pdf"), b"content C").unwrap();
    let s3 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s3.status, BackupRunStatus::Completed);

    // Pool contains 4 distinct objects: A, B, C, X
    assert_eq!(count_pool_objects(&dest_dir), 4);

    // Prune Snapshot 1 by applying KeepLastN(2)
    let ret_res = apply_retention(&store, set.id, &RetentionPolicy::KeepLastN(2)).unwrap();
    assert_eq!(ret_res.deleted_snapshots, 1);

    // Expected: A is unreferenced and deleted. B, C, X remain! (3 pool objects)
    assert_eq!(count_pool_objects(&dest_dir), 3);

    // Restore Snapshot 2 -> must yield report=B, photo=X
    let restore_s2 = tmp.path().join("restore_s2");
    let r2 = restore::restore_from_backup(&store, s2.snapshot_id, &restore_s2, &[]).unwrap();
    assert_eq!(r2.restored, 2);
    assert_eq!(r2.verified, 2);
    assert_eq!(fs::read(restore_s2.join("report.pdf")).unwrap(), b"content B");
    assert_eq!(fs::read(restore_s2.join("photo.jpg")).unwrap(), b"content X");

    // Restore Snapshot 3 -> must yield report=C, photo=X
    let restore_s3 = tmp.path().join("restore_s3");
    let r3 = restore::restore_from_backup(&store, s3.snapshot_id, &restore_s3, &[]).unwrap();
    assert_eq!(r3.restored, 2);
    assert_eq!(r3.verified, 2);
    assert_eq!(fs::read(restore_s3.join("report.pdf")).unwrap(), b"content C");
    assert_eq!(fs::read(restore_s3.join("photo.jpg")).unwrap(), b"content X");
}

#[test]
fn test_corrupt_or_missing_object_safely_aborts_gc_and_restore() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    fs::write(source_dir.join("critical.data"), b"important payload").unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "SafeFail", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::KeepAll, VerificationMode::Standard).unwrap();
    let engine = BackupEngine::new(&store);
    let s1 = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(s1.status, BackupRunStatus::Completed);

    // Create Snapshot 2
    fs::write(source_dir.join("other.data"), b"other payload").unwrap();
    let _s2 = engine.run_backup(&set, &dest_dir).unwrap();

    // Manually delete pool object for critical.data
    let hash = organizer_core::hasher::hash_file(&source_dir.join("critical.data")).unwrap();
    let rel_pool = manifest::pool_rel_path(&hash).unwrap();
    let pool_file = dest_dir.join(".fileharbor").join("pool").join(&rel_pool);
    assert!(pool_file.exists());
    fs::remove_file(&pool_file).unwrap();

    // Attempting to restore Snapshot 1 must fail safely
    let restore_dir = tmp.path().join("failed_restore");
    let r_fail = restore::restore_from_backup(&store, s1.snapshot_id, &restore_dir, &[]);
    // Should report failure
    if let Ok(res) = r_fail {
        assert_eq!(res.failed, 1);
        assert!(!restore_dir.join("critical.data").exists());
    }

    // Attempting retention must detect missing object in retained snapshot and abort pre-flight!
    let gc_fail = apply_retention(&store, set.id, &RetentionPolicy::KeepLastN(1));
    assert!(gc_fail.is_err());
}

#[test]
fn test_interrupted_staging_recovery() {
    let tmp = tempfile::tempdir().unwrap();
    let store_path = tmp.path().join("store.db");
    let source_dir = tmp.path().join("source");
    let dest_dir = tmp.path().join("backup");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    // Inject stale partial artifacts in .fileharbor-partial
    let stale_dir = dest_dir.join(".fileharbor-partial").join("pool");
    fs::create_dir_all(&stale_dir).unwrap();
    fs::write(stale_dir.join("deadbeef.partial"), b"incomplete bytes").unwrap();

    fs::write(source_dir.join("fresh.txt"), b"fresh content").unwrap();

    let store = Store::open(&store_path).unwrap();
    let set = create_backup_set(&store, "StagingRecovery", &source_dir.to_string_lossy(), &dest_dir.to_string_lossy(), BackupSchedule::Manual, RetentionPolicy::KeepAll, VerificationMode::Standard).unwrap();
    let engine = BackupEngine::new(&store);

    let summary = engine.run_backup(&set, &dest_dir).unwrap();
    assert_eq!(summary.status, BackupRunStatus::Completed);
    assert_eq!(summary.files_new, 1);

    // Stale partial directory must have been cleaned up
    assert!(!dest_dir.join(".fileharbor-partial").exists());

    // Restore should succeed cleanly
    let restore_dir = tmp.path().join("restored_fresh");
    let res = restore::restore_from_backup(&store, summary.snapshot_id, &restore_dir, &[]).unwrap();
    assert_eq!(res.restored, 1);
    assert_eq!(res.verified, 1);
    assert_eq!(fs::read(restore_dir.join("fresh.txt")).unwrap(), b"fresh content");
}
