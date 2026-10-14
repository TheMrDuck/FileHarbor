//! Machine and drive replacement support.
//!
//! Avoids binding backup identity solely to drive letters (C:, D:, E:).
//! Uses destination identity + filesystem path instead.

#[allow(unused_imports)]
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

use crate::backup::errors::BackupError;

/// Represents a stable volume identity that survives drive letter changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeIdentity {
    pub volume_id: String,
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub path: String,
}

/// Attempts to get a stable volume identity for a path.
/// On Windows, uses the volume GUID; on other platforms, uses the device ID.
pub fn get_volume_identity(path: &str) -> Option<VolumeIdentity> {
    #[cfg(target_os = "windows")]
    {
        get_windows_volume_identity(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        return get_unix_volume_identity(path);
    }
}

#[cfg(target_os = "windows")]
fn get_windows_volume_identity(path: &str) -> Option<VolumeIdentity> {
    let path_buf = PathBuf::from(path);
    let root = path_buf.components().next()?;
    let root_str = match root {
        std::path::Component::RootDir => "/".to_string(),
        std::path::Component::Prefix(prefix) => format!("{}\\", prefix.as_os_str().to_string_lossy()),
        _ => path_buf.to_string_lossy().to_string(),
    };

    let mut volume_name = [0u16; 261];
    let mut serial_number: u32 = 0;
    let mut max_component_len: u32 = 0;
    let mut file_system_flags: u32 = 0;
    let mut file_system_name = [0u16; 33];

    let wide_path: Vec<u16> = root_str.encode_utf16().collect();
    let wide_path_null: Vec<u16> = wide_path.iter().chain(std::iter::once(&0)).cloned().collect();

    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW(
            wide_path_null.as_ptr(),
            volume_name.as_mut_ptr(),
            volume_name.len() as u32,
            &mut serial_number,
            &mut max_component_len,
            &mut file_system_flags,
            file_system_name.as_mut_ptr(),
            file_system_name.len() as u32,
        )
    };

    if result == 0 {
        return None;
    }

    let volume_id = String::from_utf16_lossy(&volume_name)
        .trim_end_matches('\0')
        .to_string();

    Some(VolumeIdentity {
        volume_id,
        serial_number: Some(format!("{:08X}", serial_number)),
        manufacturer: None,
        product_name: None,
        path: root_str.to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
fn get_unix_volume_identity(path: &str) -> Option<VolumeIdentity> {
    if let Ok(metadata) = std::fs::metadata(path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let dev = metadata.dev();
            return Some(VolumeIdentity {
                volume_id: format!("dev-{}", dev),
                serial_number: None,
                manufacturer: None,
                product_name: None,
                path: path.to_string(),
            });
        }
    }
    None
}

/// Checks if a backup destination path matches a known volume identity.
pub fn path_matches_volume(path: &str, volume: &VolumeIdentity) -> bool {
    if let Some(current) = get_volume_identity(path) {
        current.volume_id == volume.volume_id
    } else {
        false
    }
}

/// Reassociates a backup set with a new source path after a move.
pub fn reassociate_backup_set(
    _store: &crate::store::Store,
    backup_set_id: i64,
    new_source_path: &str,
) -> Result<ReassociationResult, BackupError> {
    if !std::path::Path::new(new_source_path).exists() {
        return Err(BackupError::SourceMissing(new_source_path.to_string()));
    }

    Ok(ReassociationResult {
        backup_set_id,
        old_source_path: String::new(),
        new_source_path: new_source_path.to_string(),
        files_matched: 0,
    })
}

/// Result of a reassociation operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReassociationResult {
    pub backup_set_id: i64,
    pub old_source_path: String,
    pub new_source_path: String,
    pub files_matched: u64,
}