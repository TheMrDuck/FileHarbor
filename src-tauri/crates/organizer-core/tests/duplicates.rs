mod common;

use std::fs;

use organizer_core::models::*;
use organizer_core::store::{FileFilter, Store};

use common::*;

fn store() -> Store {
    let dir = temp_root("store");
    Store::open(&dir.join("test.db")).expect("open store")
}

#[test]
fn empty_directory_scan_produces_zero_files() {
    let s = store();
    let root = temp_root("scan-empty-dir");
    fs::create_dir_all(root.join("nothing/here")).unwrap();

    let session = scan(&s, &[root], "empty-dir");
    let summary = s.scan_summary(session).unwrap();
    assert_eq!(summary.files_scanned, 0);
    assert_eq!(summary.duplicate_groups, 0);
    assert_eq!(summary.empty_files, 0);
}

#[test]
fn nested_directories_are_scanned_recursively() {
    let s = store();
    let root = temp_root("scan-nested");
    write_file(&root, "a.txt", b"aaa");
    write_file(&root, "level1/level2/deep.bin", b"deep");
    write_file(&root, "level1/mid.txt", b"mid");

    let session = scan(&s, &[root], "nested");
    let page = s.list_files(session, &FileFilter { limit: 100, ..Default::default() }).unwrap();
    assert_eq!(page.total, 3);
}

#[test]
fn exact_duplicate_matrix() {
    // A == B -> duplicate (same name even)
    // A != C -> not duplicate (same name!)
    // same size + different content -> not duplicate
    // different names + identical content -> duplicate
    let s = store();
    let root = temp_root("scan-dupes");
    write_file(&root, "report.txt", b"identical payload");       // A
    write_file(&root, "sub/report.txt", b"identical payload");   // B: dup of A
    write_file(&root, "other/report.txt", b"DIFFERENT content!");// C: same name, diff content
    write_file(&root, "x1.dat", b"0123456789");
    write_file(&root, "x2.dat", b"9876543210");                  // same size as x1, diff content
    write_file(&root, "renamed_copy.bin", b"identical payload"); // D: renamed dup of A

    let session = scan(&s, &[root.clone()], "dup-matrix");
    let groups = duplicate_groups(&s, session);

    assert_eq!(groups.len(), 1, "expected exactly one duplicate group, got {groups:?}");
    let g = &groups[0];
    assert_eq!(g.size_bytes, b"identical payload".len() as u64);
    assert_eq!(g.members.len(), 3, "A, B and the renamed copy form one group");

    let names: Vec<&str> =
        g.members.iter().map(|m| m.file_name.as_str()).collect();
    assert!(names.contains(&"renamed_copy.bin"), "renamed duplicate must be included");

    // Reclaimable = total - one kept copy.
    assert_eq!(g.reclaimable_bytes(), b"identical payload".len() as u64 * 2);

    let summary = s.scan_summary(session).unwrap();
    assert_eq!(summary.duplicate_groups, 1);
    assert_eq!(summary.duplicate_files, 3);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn unique_size_files_are_never_duplicates() {
    let s = store();
    let root = temp_root("scan-unique");
    write_file(&root, "one.bin", b"11111");
    write_file(&root, "two.bin", b"222222222");

    let session = scan(&s, &[root], "unique-sizes");
    assert_eq!(duplicate_groups(&s, session).len(), 0);
}

#[test]
fn empty_files_detected_separately_and_not_duplicate_grouped() {
    let s = store();
    let root = temp_root("scan-empty-files");
    write_file(&root, ".gitkeep", b"");
    write_file(&root, "placeholder.dat", b"");
    write_file(&root, "real.txt", b"real");

    let session = scan(&s, &[root], "empty-files");
    let summary = s.scan_summary(session).unwrap();
    assert_eq!(summary.empty_files, 2);
    // Two zero-byte files must NOT appear as a duplicate group.
    assert_eq!(summary.duplicate_groups, 0);

    let empty_page = s.list_files(
        session,
        &FileFilter { empty_only: true, limit: 100, ..Default::default() },
    ).unwrap();
    assert_eq!(empty_page.total, 2);
}

#[test]
fn symlinked_directories_are_not_followed() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let s = store();
        let root = temp_root("scan-symlink");
        write_file(&root, "real/file.txt", b"content");
        symlink(root.join("real"), root.join("loop")).expect("create symlink");

        let session = scan(&s, &[root.clone()], "symlink");
        let page = s
            .list_files(session, &FileFilter { limit: 100, ..Default::default() })
            .unwrap();
        // Only the real file; nothing reached through the symlink.
        assert_eq!(page.total, 1);
        assert_ne!(page.items[0].path.find("loop"), Some(0));
        let _ = fs::remove_dir_all(root);
    }

    // On Windows creating symlinks requires privileges; junction policy is
    // covered by the traversal code skipping non-regular entries.
}

#[test]
fn file_symlinks_are_recorded_as_skipped() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let s = store();
        let root = temp_root("scan-file-link");
        let real = write_file(&root, "target.txt", b"target content");
        symlink(&real, root.join("link.txt")).expect("create file symlink");

        let session = scan(&s, &[root.clone()], "file-link");
        let page = s
            .list_files(session, &FileFilter { limit: 100, ..Default::default() })
            .unwrap();
        assert_eq!(page.total, 2);
        let link = page.items.iter().find(|f| f.file_name == "link.txt").unwrap();
        assert_eq!(link.scan_status, ScanStatus::Skipped);
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn inaccessible_entries_become_inaccessible_records_without_crashing() {
    let s = store();
    let root = temp_root("scan-inaccessible");
    write_file(&root, "good.txt", b"good");

    let session_id = s.insert_session(&[root.to_string_lossy().to_string()]).unwrap();
    // A healthy record the way the scanner would create it.
    s.insert_file_records(&[NewFileRecord {
        session_id,
        path: root.join("good.txt").to_string_lossy().to_string(),
        file_name: "good.txt".into(),
        extension: Some("txt".into()),
        size_bytes: 4,
        created_at_ms: None,
        modified_at_ms: None,
        accessed_at_ms: None,
        is_empty: false,
        scan_status: ScanStatus::Completed,
        error: None,
    }])
    .unwrap();
    // Simulate a vanished entry by inserting an inaccessible record the way
    // the scanner would when stat fails.
    s.insert_file_records(&[NewFileRecord {
        session_id: session_id,
        path: root.join("ghost.txt").to_string_lossy().to_string(),
        file_name: "ghost.txt".into(),
        extension: Some("txt".into()),
        size_bytes: 0,
        created_at_ms: None,
        modified_at_ms: None,
        accessed_at_ms: None,
        is_empty: false,
        scan_status: ScanStatus::Inaccessible,
        error: Some("The file no longer exists.".into()),
    }])
    .unwrap();

    let problems = s.problem_files(session_id, 50, 0).unwrap();
    assert_eq!(problems.total, 1);
    assert_eq!(problems.items[0].scan_status, ScanStatus::Inaccessible);
    let summary = s.scan_summary(session_id).unwrap();
    assert_eq!(summary.inaccessible_files, 1);
    // The good file still counts as scanned.
    assert_eq!(summary.files_scanned, 1);
}

#[test]
fn large_number_of_files_scan() {
    let s = store();
    let root = temp_root("scan-many");
    for i in 0..2000u32 {
        let body = format!("payload-{i}").repeat(((i % 7) + 1) as usize);
        write_file(&root, &format!("dir{}/f{i}.txt", i % 10), body.as_bytes());
    }
    // Deliberate duplicate sets spread across directories.
    for d in 0..5u32 {
        write_file(&root, &format!("dir{d}/shared-a.bin"), b"shared duplicate payload A");
        write_file(&root, &format!("dir{d}/shared-b.bin"), b"shared duplicate payload B");
    }

    let session = scan(&s, &[root.clone()], "many-files");
    let page = s.list_files(session, &FileFilter { limit: 1, ..Default::default() }).unwrap();
    assert_eq!(page.total, 2010);

    // Files repeated across dirs with identical bodies group correctly.
    let groups = duplicate_groups(&s, session);
    assert!(!groups.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn search_filter_matches_name_extension_and_path() {
    let s = store();
    let root = temp_root("scan-search");
    write_file(&root, "alpha/report.pdf", b"a-report");
    write_file(&root, "beta/notes.txt", b"notes");
    write_file(&root, "gamma/photo.PDF", b"photo"); // extension case-insensitive

    let session = scan(&s, &[root], "search");

    let by_name = s
        .list_files(
            session,
            &FileFilter { search: Some("report".into()), limit: 100, ..Default::default() },
        )
        .unwrap();
    assert_eq!(by_name.total, 1);

    let by_ext = s
        .list_files(
            session,
            &FileFilter { extension: Some(".pdf".into()), limit: 100, ..Default::default() },
        )
        .unwrap();
    assert_eq!(by_ext.total, 2, "extension filter should be case-insensitive");
}

#[test]
fn hash_cache_reused_for_unchanged_files_only() {
    let s = store();
    let root = temp_root("cache");
    let f = write_file(&root, "doc.txt", b"cached content");
    let path = f.to_string_lossy().to_string();

    s.hash_cache_put(&path, 14, Some(1000), "abc123");
    assert_eq!(
        s.hash_cache_get(&path, 14, Some(1000)),
        Some("abc123".to_string())
    );
    // Any change in identity tuple invalidates the cache entry.
    assert_eq!(s.hash_cache_get(&path, 15, Some(1000)), None);
    assert_eq!(s.hash_cache_get(&path, 14, Some(2000)), None);
}
