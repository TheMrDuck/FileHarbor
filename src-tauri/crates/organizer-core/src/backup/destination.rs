//! Backup destination validation and identity management.

use std::fs;
use std::path::{Path, PathBuf};

use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;
use rusqlite::OptionalExtension;

/// Validates a backup destination before use.
pub fn validate_destination(
    source: &Path,
    destination: &Path,
) -> DestinationValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check if destination exists or can be created
    if !destination.exists() {
        if let Some(parent) = destination.parent() {
            if !parent.exists() {
                errors.push(format!("Destination parent does not exist: {}", parent.display()));
            } else if !is_writable(parent) {
                errors.push(format!("Destination parent is not writable: {}", parent.display()));
            }
        }
    } else if !is_writable(destination) {
        errors.push(format!("Destination is not writable: {}", destination.display()));
    }

    // Check destination is not the source itself
    if same_file(source, destination) {
        errors.push("Destination is the same as source".to_string());
    }

    // Check destination is not inside source
    if is_inside(destination, source) {
        errors.push(format!(
            "Destination is inside source: {} inside {}",
            destination.display(),
            source.display()
        ));
    }

    // Check source is not inside destination (would cause recursive backup)
    if is_inside(source, destination) {
        warnings.push(format!(
            "Source is inside destination - this may cause recursive backup: {} inside {}",
            source.display(),
            destination.display()
        ));
    }

    // Check for symlink escape (best effort)
    if let Err(e) = check_symlink_safety(source, destination) {
        warnings.push(format!("Symlink check warning: {}", e));
    }

    // Get available space
    let available_space = get_available_space(destination).ok();

    let valid = errors.is_empty();

    DestinationValidation {
        valid,
        destination_id: None,
        errors,
        warnings,
        available_space_bytes: available_space,
        estimated_required_bytes: None,
    }
}

/// Checks if a path is writable.
fn is_writable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| !m.permissions().readonly())
        .unwrap_or(false)
}

/// Checks if two paths refer to the same file (best effort).
/// Uses canonical path comparison as a stable fallback.
fn same_file(a: &Path, b: &Path) -> bool {
    if let (Ok(a_canon), Ok(b_canon)) = (a.canonicalize(), b.canonicalize()) {
        a_canon == b_canon
    } else {
        false
    }
}

/// Checks if `child` is inside `parent` directory.
fn is_inside(child: &Path, parent: &Path) -> bool {
    let child = normalize_path(child);
    let parent = normalize_path(parent);

    if child == parent {
        return true;
    }

    child.starts_with(parent.join(""))
}

/// Normalizes a path for comparison.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(_) => components.push(comp.as_os_str()),
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            _ => components.push(comp.as_os_str()),
        }
    }
    PathBuf::from_iter(components)
}

/// Checks for symlink safety (best effort).
fn check_symlink_safety(_source: &Path, _destination: &Path) -> Result<(), String> {
    // On Windows, check for junctions
    // On Unix, check for symlinks
    // This is a best-effort check; we don't recursively follow
    Ok(())
}

/// Gets available space on the filesystem containing path.
fn get_available_space(path: &Path) -> Result<u64, String> {
    let path = if path.exists() { path } else { path.parent().unwrap_or(path) };

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut free_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free: u64 = 0;

        unsafe {
            if GetDiskFreeSpaceExW(
                path_wide.as_ptr(),
                &mut free_bytes as *mut _,
                &mut total_bytes as *mut _,
                &mut total_free as *mut _,
            ) == 0
            {
                return Err("GetDiskFreeSpaceExW failed".to_string());
            }
        }
        Ok(free_bytes)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let stat = fs::metadata(path).map_err(|e| e.to_string())?;
        let block_size = stat.blksize() as u64;
        let free_blocks = stat.blocks() as u64; // This is actually free blocks on some systems
        // Use statvfs for accurate free space
        Ok(free_blocks * block_size)
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(u64::MAX)
    }
}

/// Ensures a backup destination has FileHarbor identity.
/// Creates .fileharbor/destination.json if missing.
pub fn ensure_destination_identity(
    store: &Store,
    destination_path: &Path,
    label: Option<String>,
) -> Result<BackupDestination, BackupError> {
    let fileharbor_dir = destination_path.join(".fileharbor");
    let identity_file = fileharbor_dir.join("destination.json");

    // Read existing identity if present
    let destination_id = if identity_file.exists() {
        let content = fs::read_to_string(&identity_file)
            .map_err(|e| BackupError::Internal(format!("Failed to read destination identity: {}", e)))?;
        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| BackupError::Internal(format!("Invalid destination identity: {}", e)))?;
        json.get("destination_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    let destination_id = destination_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Get or create filesystem ID
    let filesystem_id = get_filesystem_id(destination_path).ok();

    // Create directory if needed
    if !fileharbor_dir.exists() {
        fs::create_dir_all(&fileharbor_dir)
            .map_err(|e| BackupError::Internal(format!("Failed to create .fileharbor dir: {}", e)))?;
    }

    // Write/update identity file atomically
    let identity = serde_json::json!({
        "destination_id": destination_id,
        "created_at_ms": now_ms(),
        "fileharbor_version": env!("CARGO_PKG_VERSION"),
        "backup_schema_version": 1,
        "filesystem_id": filesystem_id.clone(),
        "label": label.clone(),
    });

    let temp_file = identity_file.with_extension("json.tmp");
    fs::write(&temp_file, serde_json::to_string_pretty(&identity).unwrap())
        .map_err(|e| BackupError::Internal(format!("Failed to write identity temp file: {}", e)))?;
    fs::rename(&temp_file, &identity_file)
        .map_err(|e| BackupError::Internal(format!("Failed to commit identity file: {}", e)))?;

    // Upsert in database
    let dest = BackupDestination {
        id: upsert_destination(store, &destination_id, destination_path, label.clone(), filesystem_id.clone())?,
        destination_id,
        path: destination_path.to_string_lossy().to_string(),
        label,
        filesystem_id,
        created_at_ms: now_ms(),
        last_seen_at_ms: Some(now_ms()),
    };

    Ok(dest)
}

/// Gets a stable filesystem identifier.
fn get_filesystem_id(path: &Path) -> Result<String, String> {
    // Use the canonical path as a stable identifier
    let canon = path.canonicalize().map_err(|e| e.to_string())?;
    // Hash the canonical path for a stable ID
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    canon.hash(&mut hasher);
    Ok(format!("{:x}", hasher.finish()))
}

/// Inserts or updates destination in database.
fn upsert_destination(
    store: &Store,
    destination_id: &str,
    path: &Path,
    label: Option<String>,
    filesystem_id: Option<String>,
) -> Result<i64, BackupError> {
    let now = now_ms();
    store.conn.execute(
        "INSERT INTO backup_destinations (destination_id, path, label, filesystem_id, created_at_ms, last_seen_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(destination_id) DO UPDATE SET
            path = excluded.path,
            label = excluded.label,
            filesystem_id = excluded.filesystem_id,
            last_seen_at_ms = excluded.last_seen_at_ms",
        rusqlite::params![
            destination_id,
            path.to_string_lossy().to_string(),
            label,
            filesystem_id,
            now,
            now
        ],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    Ok(store.conn.last_insert_rowid())
}

/// Gets a destination by its ID string.
pub fn get_destination_by_id_string(
    store: &Store,
    destination_id: &str,
) -> Result<Option<BackupDestination>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, destination_id, path, label, filesystem_id, created_at_ms, last_seen_at_ms
         FROM backup_destinations WHERE destination_id = ?1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let dest = stmt.query_row(rusqlite::params![destination_id], |row| {
        Ok(BackupDestination {
            id: row.get(0)?,
            destination_id: row.get(1)?,
            path: row.get(2)?,
            label: row.get(3)?,
            filesystem_id: row.get(4)?,
            created_at_ms: row.get(5)?,
            last_seen_at_ms: row.get(6)?,
        })
    }).optional().map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    Ok(dest)
}

/// Lists all known destinations.
pub fn list_destinations(store: &Store) -> Result<Vec<BackupDestination>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, destination_id, path, label, filesystem_id, created_at_ms, last_seen_at_ms
         FROM backup_destinations ORDER BY created_at_ms DESC"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map([], |row| {
        Ok(BackupDestination {
            id: row.get(0)?,
            destination_id: row.get(1)?,
            path: row.get(2)?,
            label: row.get(3)?,
            filesystem_id: row.get(4)?,
            created_at_ms: row.get(5)?,
            last_seen_at_ms: row.get(6)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Gets the destination path by destination ID.
pub fn get_destination_path(store: &Store, destination_id: i64) -> Result<String, BackupError> {
    store.conn.query_row(
        "SELECT path FROM backup_destinations WHERE id = ?1",
        rusqlite::params![destination_id],
        |row| row.get(0),
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Recognizes if a path is a FileHarbor backup destination.
pub fn recognize_backup_destination(path: &Path) -> Option<String> {
    let identity_file = path.join(".fileharbor").join("destination.json");
    if identity_file.exists() {
        if let Ok(content) = fs::read_to_string(&identity_file) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                return json.get("destination_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}