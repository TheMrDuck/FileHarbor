use organizer_core::backup::patrol::run_harbor_patrol;
use organizer_core::backup::models::*;
use organizer_core::backup::test_helpers::{
    create_test_backup_run, create_test_backup_snapshot, create_test_backup_file, get_snapshot_status,
};
use organizer_core::backup::*;
use organizer_core::cancel::CancelToken;
use organizer_core::hasher;
use organizer_core::store::Store;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

fn compute_sha256(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:064x}", h.finalize())
}

#[test]
fn test_harbor_patrol_clean_pool() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();

    let dest_dir = tempdir().unwrap();
    let pool_dir = dest_dir.path().join(".fileharbor").join("pool");

    let content = b"healthy content bytes 12345";
    let token = CancelToken::new();
    let hash = compute_sha256(content);

    let prefix = &hash[0..2];
    let suffix = &hash[2..];
    let obj_dir = pool_dir.join(prefix);
    fs::create_dir_all(&obj_dir).unwrap();
    let obj_file = obj_dir.join(suffix);
    let mut f = File::create(&obj_file).unwrap();
    f.write_all(content).unwrap();
    drop(f);

    let summary = run_harbor_patrol(
        &store,
        1,
        dest_dir.path(),
        true,
        &token,
    ).expect("patrol should succeed");

    assert_eq!(summary.total_pool_objects, 1);
    assert_eq!(summary.scanned_objects, 1);
    assert_eq!(summary.healthy_objects, 1);
    assert_eq!(summary.bit_rot_detected, 0);
    assert_eq!(summary.self_healed_objects, 0);
}

#[test]
fn test_harbor_patrol_detects_bit_rot_and_self_heals() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();

    let source_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    // 1. Create original healthy source file
    let src_file = source_dir.path().join("document.pdf");
    let mut sf = File::create(&src_file).unwrap();
    let original_bytes = b"important financial document healthy version";
    sf.write_all(original_bytes).unwrap();
    drop(sf);

    let token = CancelToken::new();
    let hash = compute_sha256(original_bytes);

    // 2. Register backup set and snapshot in database
    let set = create_backup_set(
        &store,
        "Work Docs",
        source_dir.path().to_str().unwrap(),
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepAll,
        VerificationMode::Standard,
    ).unwrap();

    let run_id = create_test_backup_run(&store, set.id, BackupRunStatus::Completed);
    let snap_id = create_test_backup_snapshot(&store, set.id, run_id, "snap1", "verified", None);
    create_test_backup_file(&store, snap_id, "document.pdf", original_bytes.len() as u64, &hash, "verified");

    // 3. Create corrupted pool object (simulate silent bit-rot on disk)
    let pool_dir = dest_dir.path().join(".fileharbor").join("pool");
    let prefix = &hash[0..2];
    let suffix = &hash[2..];
    let obj_dir = pool_dir.join(prefix);
    fs::create_dir_all(&obj_dir).unwrap();
    let pool_obj_file = obj_dir.join(suffix);

    let mut pf = File::create(&pool_obj_file).unwrap();
    pf.write_all(b"corrupted bit-rot bytes replacing healthy pool object!").unwrap();
    drop(pf);

    // Verify it is indeed corrupted
    let corrupt_hash = hasher::hash_file(&pool_obj_file).unwrap();
    assert_ne!(corrupt_hash, hash);

    // 4. Run Harbor Patrol with auto_heal = true
    let summary = run_harbor_patrol(
        &store,
        set.id,
        dest_dir.path(),
        true,
        &token,
    ).expect("harbor patrol run");

    assert_eq!(summary.total_pool_objects, 1);
    assert_eq!(summary.scanned_objects, 1);
    assert_eq!(summary.bit_rot_detected, 1, "must detect bit-rot");
    assert_eq!(summary.self_healed_objects, 1, "must autonomously heal from source");
    assert_eq!(summary.unrecoverable_objects, 0);

    // 5. Verify pool file on disk is healed
    let restored_hash = hasher::hash_file(&pool_obj_file).unwrap();
    assert_eq!(restored_hash, hash, "pool file must be restored to original hash");
}

#[test]
fn test_harbor_patrol_unrecoverable_when_no_source() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();

    let source_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    let missing_bytes = b"nonexistent source file content";
    let token = CancelToken::new();
    let hash = compute_sha256(missing_bytes);

    let set = create_backup_set(
        &store,
        "Photos",
        source_dir.path().to_str().unwrap(),
        dest_dir.path().to_str().unwrap(),
        BackupSchedule::Manual,
        RetentionPolicy::KeepAll,
        VerificationMode::Standard,
    ).unwrap();

    let run_id = create_test_backup_run(&store, set.id, BackupRunStatus::Completed);
    let snap_id = create_test_backup_snapshot(&store, set.id, run_id, "snap1", "verified", None);
    create_test_backup_file(&store, snap_id, "deleted_file.png", missing_bytes.len() as u64, &hash, "verified");

    // Create corrupted pool object, but source file does not exist on disk
    let pool_dir = dest_dir.path().join(".fileharbor").join("pool");
    let prefix = &hash[0..2];
    let suffix = &hash[2..];
    let obj_dir = pool_dir.join(prefix);
    fs::create_dir_all(&obj_dir).unwrap();
    let pool_obj_file = obj_dir.join(suffix);

    let mut pf = File::create(&pool_obj_file).unwrap();
    pf.write_all(b"bad pool object").unwrap();
    drop(pf);

    let summary = run_harbor_patrol(
        &store,
        set.id,
        dest_dir.path(),
        true,
        &token,
    ).expect("harbor patrol run");

    assert_eq!(summary.bit_rot_detected, 1);
    assert_eq!(summary.self_healed_objects, 0);
    assert_eq!(summary.unrecoverable_objects, 1);
    assert_eq!(summary.unrecoverable_details.len(), 1);

    // Verify snapshot in DB is updated to 'corrupted'
    let snap_status = get_snapshot_status(&store, snap_id);
    assert_eq!(snap_status, "corrupted");
}
