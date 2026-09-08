//! Backup discovery: recognizes FileHarbor-managed backup destinations.
//!
//! When FileHarbor encounters a `.fileharbor/` directory on a drive,
//! it can recognize it as a backup destination rather than treating it
//! as an arbitrary folder.

use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};

use crate::backup::errors::BackupError;

/// Marker directory name used by FileHarbor.
pub const FILEHARBOR_MARKER: &str = ".fileharbor";

/// Destination identity file name.
pub const DESTINATION_JSON: &str = "destination.json";

/// Checks if a directory is a FileHarbor backup destination.
pub fn is_backup_destination(path: &Path) -> bool {
    path.join(FILEHARBOR_MARKER).is_dir()
}

/// Checks if a directory contains FileHarbor backup data.
pub fn contains_backup_data(path: &Path) -> bool {
    path.join(FILEHARBOR_MARKER).exists()
}

/// Reads the destination identity from a FileHarbor backup destination.
pub fn read_destination_identity(path: &Path) -> Option<DestinationIdentity> {
    let identity_path = path.join(FILEHARBOR_MARKER).join(DESTINATION_JSON);
    if !identity_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&identity_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Writes the destination identity to a FileHarbor backup destination.
pub fn write_destination_identity(path: &Path, identity: &DestinationIdentity) -> Result<(), BackupError> {
    let marker_dir = path.join(FILEHARBOR_MARKER);
    fs::create_dir_all(&marker_dir)
        .map_err(|e| BackupError::DestinationError(e.to_string()))?;
    let identity_path = marker_dir.join(DESTINATION_JSON);
    let content = serde_json::to_string_pretty(identity)
        .map_err(|e| BackupError::SerializationError(e.to_string()))?;
    // Atomic write: write to temp, then rename
    let temp_path = identity_path.with_extension("json.tmp");
    fs::write(&temp_path, content)
        .map_err(|e| BackupError::DestinationError(e.to_string()))?;
    fs::rename(&temp_path, &identity_path)
        .map_err(|e| BackupError::DestinationError(e.to_string()))?;
    Ok(())
}

/// Identity information for a FileHarbor backup destination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationIdentity {
    pub destination_id: String,
    pub created_at_ms: i64,
    pub fileharbor_version: String,
    pub schema_version: u32,
    pub label: Option<String>,
}

/// Scans a directory for FileHarbor backup destinations.
pub fn discover_backup_destinations(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if is_backup_destination(root) {
        found.push(root.to_path_buf());
    }
    // Also check common subdirectories
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && is_backup_destination(&path) {
                found.push(path);
            }
        }
    }
    found
}

/// Returns a human-readable description of a backup destination.
pub fn destination_description(path: &Path) -> String {
    if is_backup_destination(path) {
        format!("FileHarbor backup destination: {}", path.display())
    } else {
        format!("Regular folder: {}", path.display())
    }
}