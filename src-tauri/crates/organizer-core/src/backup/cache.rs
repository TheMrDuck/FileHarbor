//! Backup cache: optimizes backup performance by caching file hashes.
//!
//! Cache is an optimization, never the source of truth.
//! Verification must always be able to recalculate the hash when required.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A cached hash entry for a file.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub path: String,
    pub size_bytes: u64,
    pub mtime_ms: i64,
    pub sha256: String,
    pub cached_at_ms: i64,
}

/// In-memory backup hash cache.
pub struct BackupCache {
    entries: HashMap<String, CacheEntry>,
    max_entries: usize,
}

impl BackupCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Looks up a cached hash for a file.
    /// Returns the hash if the path, size, and mtime all match.
    pub fn get(&self, path: &str, size_bytes: u64, mtime_ms: i64) -> Option<&str> {
        self.entries.get(path).and_then(|entry| {
            if entry.size_bytes == size_bytes && entry.mtime_ms == mtime_ms {
                Some(entry.sha256.as_str())
            } else {
                None
            }
        })
    }

    /// Stores a hash in the cache.
    pub fn put(&mut self, path: String, size_bytes: u64, mtime_ms: i64, sha256: String) {
        if self.entries.len() >= self.max_entries {
            // Evict oldest entry
            if let Some(oldest) = self.entries.values()
                .min_by_key(|e| e.cached_at_ms)
                .map(|e| e.path.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let path_for_entry = path.clone();
        self.entries.insert(path, CacheEntry {
            path: path_for_entry,
            size_bytes,
            mtime_ms,
            sha256,
            cached_at_ms: now,
        });
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for BackupCache {
    fn default() -> Self {
        Self::new(10000)
    }
}