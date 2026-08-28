//! Shared helpers for filesystem integration tests.
//!
//! Every test operates exclusively inside a `tempfile` temporary
//! directory - never on real user folders.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use organizer_core::cancel::CancelToken;
use organizer_core::models::*;
use organizer_core::scanner::{self, ScannerConfig};
use organizer_core::store::Store;

static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn temp_root(tag: &str) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "fo-test-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("create temp root");
    base
}

pub fn write_file(dir: &Path, rel: &str, contents: &[u8]) -> PathBuf {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(&path, contents).expect("write file");
    path
}

/// Run a complete scan over `roots` synchronously.
pub fn scan(store: &Store, roots: &[PathBuf], tag: &str) -> i64 {
    let session_id = store.insert_session(
        &roots.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
    )
    .expect("insert session");
    let cfg = ScannerConfig {
        roots: roots.to_vec(),
        ..ScannerConfig::default()
    };
    let cancel = CancelToken::new();
    let status = scanner::run_scan(store, &cfg, session_id, &cancel, &|_| {});
    assert_eq!(status, SessionStatus::Completed, "scan should complete for {tag}");
    session_id
}

pub fn duplicate_groups(store: &Store, session_id: i64) -> Vec<DuplicateGroup> {
    let mut all = Vec::new();
    let mut page = 0;
    loop {
        let (groups, total) =
            store.duplicate_groups_page(session_id, 25, page * 25).expect("groups");
        all.extend(groups);
        if all.len() as i64 >= total {
            break;
        }
        page += 1;
    }
    all
}
