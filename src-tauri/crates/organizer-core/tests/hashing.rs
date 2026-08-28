mod common;

use organizer_core::cancel::CancelToken;
use organizer_core::hasher::{self, HashOutcome};

use common::*;

const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn identical_content_produces_identical_hash() {
    let root = temp_root("hash-same");
    let a = write_file(&root, "a.bin", b"hello world");
    let b = write_file(&root, "sub/b-copy.bin", b"hello world");

    let token = CancelToken::new();
    let ha = match hasher::sha256_file(&a, &token, 4096) {
        Ok(HashOutcome::Computed(h)) => h,
        other => panic!("unexpected: {other:?}"),
    };
    let hb = match hasher::sha256_file(&b, &token, 4096) {
        Ok(HashOutcome::Computed(h)) => h,
        other => panic!("unexpected: {other:?}"),
    };
    assert_eq!(ha, hb);
}

#[test]
fn different_content_produces_different_hash() {
    let root = temp_root("hash-diff");
    let a = write_file(&root, "a.txt", b"content one");
    let b = write_file(&root, "b.txt", b"content two");

    let token = CancelToken::new();
    let ha = match hasher::sha256_file(&a, &token, 4096) {
        Ok(HashOutcome::Computed(h)) => h,
        other => panic!("unexpected: {other:?}"),
    };
    let hb = match hasher::sha256_file(&b, &token, 4096) {
        Ok(HashOutcome::Computed(h)) => h,
        other => panic!("unexpected: {other:?}"),
    };
    assert_ne!(ha, hb);
}

#[test]
fn empty_file_hashes_to_known_sha256() {
    let root = temp_root("hash-empty");
    let f = write_file(&root, "empty.dat", b"");
    assert_eq!(hasher::empty_sha256(), EMPTY_SHA);

    let token = CancelToken::new();
    match hasher::sha256_file(&f, &token, 4096) {
        Ok(HashOutcome::Computed(h)) => assert_eq!(h, EMPTY_SHA),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn large_file_streams_correctly_across_many_chunks() {
    let root = temp_root("hash-large");
    // 3 MiB of repeating pattern: spans many 64 KiB chunks.
    let chunk: Vec<u8> = (0..u8::MAX).collect();
    let big: Vec<u8> = chunk.repeat(12_600); // ~3.05 MiB
    let f = write_file(&root, "big.bin", &big);

    let token = CancelToken::new();

    // Same content hashed with different chunk sizes must agree.
    let h_small = match hasher::sha256_file(&f, &token, 65_536) {
        Ok(HashOutcome::Computed(h)) => h,
        other => panic!("unexpected: {other:?}"),
    };
    let h_big = match hasher::sha256_file(&f, &token.clone(), 1024 * 1024) {
        Ok(HashOutcome::Computed(h)) => h,
        other => panic!("unexpected: {other:?}"),
    };
    assert_eq!(h_small, h_big);
}

#[test]
fn file_changing_during_hash_is_reported_unstable() {
    let root = temp_root("hash-unstable");
    let f = write_file(&root, "changing.bin", b"short content");

    // Simulate growth between stat() and read completion by forcing an
    // expected size larger than reality - exactly what the instability
    // detector sees when a file changes mid-read.
    let token = CancelToken::new();
    match hasher::sha256_file_internal(&f, &token, 4096, Some(999_999)) {
        Ok(HashOutcome::Unstable) => {}
        other => panic!("expected Unstable, got {other:?}"),
    }
}

#[test]
fn missing_file_reports_failure_not_empty() {
    let root = temp_root("hash-missing");
    let f = root.join("does-not-exist.txt");

    let token = CancelToken::new();
    match hasher::sha256_file(&f, &token, 4096) {
        Ok(HashOutcome::Failed(_)) => {}
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn cancelled_hash_returns_err() {
    let root = temp_root("hash-cancelled");
    let f = write_file(&root, "x.bin", b"data");

    let token = CancelToken::new();
    token.cancel(); // cancel before starting
    match hasher::sha256_file(&f, &token, 4096) {
        Err(_) => {}
        other => panic!("expected cancellation, got {other:?}"),
    }
}

#[cfg(windows)]
#[test]
fn locked_file_reports_hash_failed() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = temp_root("hash-locked");
    let f = write_file(&root, "locked.db", b"database being used by another app");

    // Hold the file with no sharing allowed, like another process would.
    let _lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&f)
        .expect("open lock handle");

    let token = CancelToken::new();
    match hasher::sha256_file(&f, &token, 4096) {
        Ok(HashOutcome::Failed(_)) => {}
        other => panic!("expected Failed for locked file, got {other:?}"),
    }
}

