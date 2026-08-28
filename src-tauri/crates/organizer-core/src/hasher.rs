//! Streaming SHA-256 hashing.
//!
//! Files are read in fixed-size chunks; nothing large is ever held in
//! memory. If the file's size changes while it is being read, the hash
//! is reported as Unstable and must not be used as content identity.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::cancel::CancelToken;
use crate::fs_layer;

/// 256 KiB read chunks: good throughput without large memory use.
pub const DEFAULT_CHUNK_SIZE: u64 = 256 * 1024;

pub const HASH_ALGORITHM: &str = "sha-256";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashOutcome {
    Computed(String),
    /// File changed while being hashed; result discarded on purpose.
    Unstable,
    Failed(String),
}

/// The hash was cancelled before completion; no result exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashCancelled;

#[derive(Debug, PartialEq, Eq)]
enum StreamResult {
    Done(String),
    Cancelled,
}

/// Hash an arbitrary reader with a cancellation check between chunks.
/// Returns Err only on I/O failure; cancellation returns Ok(Cancelled).
fn hash_reader(
    mut reader: impl Read,
    expected_size: Option<u64>,
    cancel: &CancelToken,
    chunk_size: u64,
) -> Result<StreamResult, String> {
    let mut hasher = Sha256::new();
    let chunk_size = chunk_size.max(1024) as usize;
    let mut buf = vec![0u8; chunk_size];
    let mut total_read: u64 = 0;

    loop {
        if cancel.is_cancelled() {
            return Ok(StreamResult::Cancelled);
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total_read += n as u64;
                hasher.update(&buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }

    if let Some(expected) = expected_size {
        if expected != total_read {
            return Ok(StreamResult::Done(String::new())); // signal instability via size check below
        }
    }
    let digest = hasher.finalize();
    Ok(StreamResult::Done(hex(&digest)))
}

/// Compute SHA-256 of a file, streaming in chunks.
///
/// Detects files that change size during hashing and reports them as
/// [`HashOutcome::Unstable`] so they are never treated as reliable
/// duplicate candidates.
pub fn sha256_file(
    path: &Path,
    cancel: &CancelToken,
    chunk_size: u64,
) -> Result<HashOutcome, HashCancelled> {
    sha256_file_internal(path, cancel, chunk_size, None)
}

/// Internal variant allowing tests to inject a fake expected size to
/// simulate mid-hash modification.
pub fn sha256_file_internal(
    path: &Path,
    cancel: &CancelToken,
    chunk_size: u64,
    forced_expected_size: Option<u64>,
) -> Result<HashOutcome, HashCancelled> {
    let info = match fs_layer::inspect(path) {
        Ok(info) if info.kind == fs_layer::EntryKind::RegularFile => info,
        _ => return Ok(HashOutcome::Failed("file is not accessible".to_string())),
    };
    let expected = forced_expected_size.unwrap_or(info.size_bytes);

    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => return Ok(HashOutcome::Failed(format_io(&e))),
    };

    match hash_reader(file, Some(expected), cancel, chunk_size) {
        Err(io_err) => Ok(HashOutcome::Failed(format!(
            "unable to read this file: {io_err}"
        ))),
        Ok(StreamResult::Cancelled) => Err(HashCancelled),
        Ok(StreamResult::Done(hash)) => {
            if hash.is_empty() {
                // Size changed during read: unstable by definition.
                return Ok(HashOutcome::Unstable);
            }
            // Defense-in-depth: re-stat and confirm the size still matches
            // what we actually hashed.
            if let Ok(post) = fs_layer::inspect(path) {
                if post.size_bytes != expected {
                    return Ok(HashOutcome::Unstable);
                }
            }
            Ok(HashOutcome::Computed(hash))
        }
    }
}

fn format_io(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "permission denied".to_string()
        }
        std::io::ErrorKind::NotFound => "file disappeared".to_string(),
        _ => e.to_string(),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// SHA-256 of empty input, exposed for tests/documentation.
pub fn empty_sha256() -> String {
    hex(Sha256::digest([]).as_slice())
}

