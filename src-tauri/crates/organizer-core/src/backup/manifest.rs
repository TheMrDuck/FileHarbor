//! Backup manifest handling: atomic writes, schema versioning.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use crate::backup::models::*;
use crate::backup::errors::*;
use crate::store::Store;
use rusqlite::OptionalExtension;

const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Manifest for a backup snapshot (machine-readable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub backup_set_id: String,
    pub snapshot_id: String,
    pub created_at: String, // ISO 8601
    pub status: String,
    pub source: ManifestSource,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSource {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_object: Option<String>,
}

/// Validates that a string is a canonical 64-character lowercase hexadecimal SHA-256 digest.
pub fn validate_canonical_sha256(hash: &str) -> Result<(), BackupError> {
    if hash.len() != 64 {
        return Err(BackupError::InvalidConfiguration(format!(
            "Invalid SHA-256 length: expected 64 hex chars, got {}",
            hash.len()
        )));
    }
    for b in hash.bytes() {
        if !matches!(b, b'0'..=b'9' | b'a'..=b'f') {
            return Err(BackupError::InvalidConfiguration(format!(
                "Invalid SHA-256 character '{}': must be lowercase hex",
                b as char
            )));
        }
    }
    Ok(())
}

/// Derives the relative pool path for a SHA-256 hash: `<hash[0..2]>/<hash>`.
/// Strictly validates the hash to ensure no path traversal or directory escaping.
pub fn pool_rel_path(sha256: &str) -> Result<std::path::PathBuf, BackupError> {
    validate_canonical_sha256(sha256)?;
    let shard = &sha256[0..2];
    Ok(std::path::Path::new(shard).join(sha256))
}

/// Writes a manifest atomically (temp -> verify -> rename).
pub fn write_manifest(
    manifest: &BackupManifest,
    destination_root: &Path,
    snapshot_label: &str,
) -> Result<(), BackupError> {
    let manifest_dir = destination_root.join(".fileharbor").join("manifests");
    fs::create_dir_all(&manifest_dir)
        .map_err(|e| BackupError::ManifestError(format!("Failed to create manifest dir: {}", e)))?;

    let manifest_file = manifest_dir.join(format!("{}.json", snapshot_label));
    let temp_file = manifest_file.with_extension("json.tmp");

    // Write to temp file
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| BackupError::ManifestError(format!("Failed to serialize manifest: {}", e)))?;

    fs::write(&temp_file, json)
        .map_err(|e| BackupError::ManifestError(format!("Failed to write manifest temp file: {}", e)))?;

    // Verify by reading back
    let verify_content = fs::read_to_string(&temp_file)
        .map_err(|e| BackupError::ManifestError(format!("Failed to verify manifest: {}", e)))?;
    let _: BackupManifest = serde_json::from_str(&verify_content)
        .map_err(|e| BackupError::ManifestError(format!("Manifest verification failed: {}", e)))?;

    // Atomic rename
    fs::rename(&temp_file, &manifest_file)
        .map_err(|e| BackupError::ManifestError(format!("Failed to commit manifest: {}", e)))?;

    Ok(())
}

/// Reads a manifest.
pub fn read_manifest(
    destination_root: &Path,
    snapshot_label: &str,
) -> Result<BackupManifest, BackupError> {
    let manifest_file = destination_root.join(".fileharbor").join("manifests").join(format!("{}.json", snapshot_label));
    
    if !manifest_file.exists() {
        return Err(BackupError::ManifestError(format!("Manifest not found: {}", manifest_file.display())));
    }

    let content = fs::read_to_string(&manifest_file)
        .map_err(|e| BackupError::ManifestError(format!("Failed to read manifest: {}", e)))?;

    let manifest: BackupManifest = serde_json::from_str(&content)
        .map_err(|e| BackupError::ManifestError(format!("Failed to parse manifest: {}", e)))?;

    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(BackupError::ManifestError(format!(
            "Unsupported manifest schema version: {} (expected {})",
            manifest.schema_version, MANIFEST_SCHEMA_VERSION
        )));
    }

    Ok(manifest)
}

/// Lists all manifests for a destination.
pub fn list_manifests(destination_root: &Path) -> Result<Vec<BackupManifest>, BackupError> {
    let manifest_dir = destination_root.join(".fileharbor").join("manifests");
    if !manifest_dir.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = Vec::new();
    for entry in fs::read_dir(&manifest_dir)
        .map_err(|e| BackupError::ManifestError(format!("Failed to read manifest dir: {}", e)))? 
    {
        let entry = entry.map_err(|e| BackupError::ManifestError(e.to_string()))?;
        let path = entry.path();
        if path.extension().map(|s| s == "json").unwrap_or(false) {
            if let Ok(manifest) = read_manifest(destination_root, &path.file_stem().unwrap().to_string_lossy()) {
                manifests.push(manifest);
            }
        }
    }

    manifests.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(manifests)
}

/// Builds a manifest from database snapshot.
pub fn build_manifest(
    store: &Store,
    backup_set: &BackupSet,
    snapshot: &BackupSnapshot,
    _destination_root: &Path,
) -> Result<BackupManifest, BackupError> {
    let files = get_snapshot_files(store, snapshot.id)?;

    let manifest_files: Vec<ManifestFile> = files.into_iter().map(|f| {
        let content_object = pool_rel_path(&f.sha256).ok().map(|p| p.to_string_lossy().replace('\\', "/"));
        ManifestFile {
            relative_path: f.relative_path,
            size_bytes: f.size_bytes,
            sha256: f.sha256,
            verified: f.status == BackupFileStatus::Verified,
            content_object,
        }
    }).collect();

    Ok(BackupManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        backup_set_id: backup_set.id.to_string(),
        snapshot_id: snapshot.id.to_string(),
        created_at: chrono::DateTime::<chrono::Utc>::from(
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(snapshot.created_at_ms as u64)
        ).to_rfc3339(),
        status: snapshot.status.as_str().to_string(),
        source: ManifestSource {
            label: backup_set.name.clone(),
            path: backup_set.source_path.clone(),
        },
        files: manifest_files,
    })
}

/// Gets all files in a snapshot.
fn get_snapshot_files(
    store: &Store,
    snapshot_id: i64,
) -> Result<Vec<BackupFile>, BackupError> {
    let mut stmt = store.conn.prepare(
        "SELECT id, snapshot_id, relative_path, size_bytes, sha256, status, content_id, error
         FROM backup_files WHERE snapshot_id = ?1"
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    let rows = stmt.query_map(rusqlite::params![snapshot_id], |row| {
        Ok(BackupFile {
            id: row.get(0)?,
            snapshot_id: row.get(1)?,
            relative_path: row.get(2)?,
            size_bytes: row.get(3)?,
            sha256: row.get(4)?,
            status: BackupFileStatus::parse(&row.get::<_, String>(5)?),
            content_id: row.get(6)?,
            error: row.get(7)?,
        })
    }).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| BackupError::DatabaseError(e.to_string()))
}

/// Rebuilds backup index from destination manifests (for disaster recovery).
pub fn rebuild_backup_index(
    store: &Store,
    destination_root: &Path,
) -> Result<usize, BackupError> {
    let manifests = list_manifests(destination_root)?;
    let mut restored = 0;

    for manifest in manifests {
        // Check if we already have this snapshot
        let snapshot_id: i64 = match manifest.snapshot_id.parse() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let exists: bool = store.conn.query_row(
            "SELECT 1 FROM backup_snapshots WHERE id = ?1",
            rusqlite::params![snapshot_id],
            |_| Ok(true),
        ).optional().map_err(|e| BackupError::DatabaseError(e.to_string()))?.unwrap_or(false);

        if !exists {
            // Restore from manifest
            restore_snapshot_from_manifest(store, &manifest)?;
            restored += 1;
        }
    }

    Ok(restored)
}

/// Restores a snapshot from its manifest.
fn restore_snapshot_from_manifest(
    store: &Store,
    manifest: &BackupManifest,
) -> Result<(), BackupError> {
    let tx = store.conn.unchecked_transaction()
        .map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    // Insert snapshot
    tx.execute(
        "INSERT OR IGNORE INTO backup_snapshots
         (id, backup_set_id, run_id, label, status, created_at_ms, files_count, total_size_bytes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            manifest.snapshot_id.parse::<i64>().unwrap_or(0),
            manifest.backup_set_id.parse::<i64>().unwrap_or(0),
            0, // run_id unknown
            manifest.files.len(),
            "unverified",
            manifest.created_at.parse::<chrono::DateTime<chrono::Utc>>()
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_else(|_| now_ms()),
            manifest.files.len() as u64,
            manifest.files.iter().map(|f| f.size_bytes).sum::<u64>(),
        ],
    ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

    // Insert files
    {
        let mut stmt = tx.prepare(
            "INSERT INTO backup_files
             (snapshot_id, relative_path, size_bytes, sha256, status, content_id, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        ).map_err(|e| BackupError::DatabaseError(e.to_string()))?;

        for f in &manifest.files {
            stmt.execute(rusqlite::params![
                manifest.snapshot_id.parse::<i64>().unwrap_or(0),
                f.relative_path,
                f.size_bytes as i64,
                f.sha256,
                if f.verified { "verified" } else { "pending" },
                None::<i64>,
                None::<String>,
            ]).map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        }
    } // stmt dropped here

    tx.commit().map_err(|e| BackupError::DatabaseError(e.to_string()))?;
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}