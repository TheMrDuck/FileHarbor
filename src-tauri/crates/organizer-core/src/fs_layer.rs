//! Safe filesystem inspection primitives.
//!
//! Symlink policy (documented behavior):
//! - Directory symlinks/junctions are NEVER followed during traversal.
//!   This prevents cycles and prevents the same physical content from
//!   being counted multiple times through links.
//! - File symlinks are recorded as `EntryKind::Symlink` and are skipped
//!   entirely for duplicate detection. A symlink is not a file copy.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    RegularFile,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone)]
pub struct EntryInfo {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size_bytes: u64,
    pub created_ms: Option<i64>,
    pub modified_ms: Option<i64>,
    pub accessed_ms: Option<i64>,
}

/// Human-readable description of a filesystem failure, suitable for
/// showing directly to the user. Technical codes stay in logs.
#[derive(Debug, Clone)]
pub struct FsFailure {
    pub message: String,
    pub technical: String,
}

pub fn inspect(path: &Path) -> Result<EntryInfo, FsFailure> {
    // symlink_metadata: never follow the final component.
    let lmeta = fs::symlink_metadata(path).map_err(|e| failure(path, &e))?;

    if lmeta.file_type().is_symlink() {
        return Ok(EntryInfo {
            path: path.to_path_buf(),
            kind: EntryKind::Symlink,
            size_bytes: 0,
            created_ms: None,
            modified_ms: None,
            accessed_ms: None,
        });
    }

    let meta = fs::metadata(path).map_err(|e| failure(path, &e))?;
    let kind = if meta.is_file() {
        EntryKind::RegularFile
    } else if meta.is_dir() {
        EntryKind::Directory
    } else {
        EntryKind::Other
    };

    Ok(EntryInfo {
        path: path.to_path_buf(),
        kind,
        size_bytes: meta.len(),
        created_ms: time_to_ms(meta.created().ok()),
        modified_ms: time_to_ms(meta.modified().ok()),
        accessed_ms: time_to_ms(meta.accessed().ok()),
    })
}

fn time_to_ms(t: Option<std::time::SystemTime>) -> Option<i64> {
    t.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

fn failure(path: &Path, e: &std::io::Error) -> FsFailure {
    let message = match e.kind() {
        std::io::ErrorKind::NotFound => {
            "The file no longer exists. It may have been moved or deleted while scanning."
                .to_string()
        }
        std::io::ErrorKind::PermissionDenied => {
            "You do not have permission to access this item.".to_string()
        }
        _ => "This location is unavailable. The drive may be disconnected or the path invalid."
            .to_string(),
    };
    FsFailure {
        message,
        technical: format!("{}: {e}", path.display()),
    }
}

/// Normalizes a user exclusion path to a comparable string form.
pub fn normalize_exclusion(path: &Path) -> String {
    path.to_string_lossy().trim_end_matches(['/', '\\']).to_lowercase()
}

/// True when `candidate` is inside (or equal to) an excluded directory.
pub fn is_excluded(candidate: &Path, normalized_exclusions: &[String]) -> bool {
    let lower = candidate.to_string_lossy().to_lowercase();
    normalized_exclusions.iter().any(|ex| {
        lower == *ex || lower.starts_with(ex) && matches!(lower.as_bytes().get(ex.len()), Some(b'\\') | Some(b'/'))
    })
}

/// Consolidate a duplicate file into a filesystem hardlink pointing to the original canonical file.
/// If successful, the duplicate path remains fully accessible to the OS and applications,
/// but physical disk consumption for the duplicate is reduced to zero bytes.
pub fn consolidate_as_hardlink(original: &Path, duplicate: &Path) -> Result<(), String> {
    if !original.exists() {
        return Err(format!("Original file does not exist: {}", original.display()));
    }
    if !duplicate.exists() {
        return Err(format!("Duplicate file does not exist: {}", duplicate.display()));
    }
    if original == duplicate {
        return Ok(());
    }
    let parent = duplicate.parent().ok_or_else(|| "Duplicate file has no parent directory".to_string())?;
    let temp_name = format!(".tmp_hlink_{}.tmp", uuid::Uuid::new_v4());
    let temp_link = parent.join(temp_name);

    // Attempt hardlink at temporary path first (verifies same volume & permissions)
    fs::hard_link(original, &temp_link).map_err(|e| {
        format!("Hardlink creation failed (cross-volume hardlinks are not supported by OS): {e}")
    })?;

    // Safely replace duplicate with the hardlink
    if let Err(e) = fs::remove_file(duplicate) {
        let _ = fs::remove_file(&temp_link);
        return Err(format!("Failed to remove duplicate prior to hardlink swap: {e}"));
    }

    if let Err(e) = fs::rename(&temp_link, duplicate) {
        return Err(format!("Failed to place hardlink at destination: {e}"));
    }

    Ok(())
}

