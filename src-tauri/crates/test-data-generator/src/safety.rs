use anyhow::Result;
use std::path::Path;
use std::fs;

/// Check if a path is dangerous (system directories, user home, etc.)
pub fn is_dangerous_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // Check for filesystem roots
    if path_str == "/" || path_str == "\\" || path_str == "c:\\" || path_str == "c:/" || path_str == "c:" {
        return true;
    }

    // Explicitly designated test data folders (e.g., AppData/.../TestData or /test-data) are safe
    // as long as they are not inside critical OS system folders.
    let is_test_data = path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.eq_ignore_ascii_case("testdata") || s.eq_ignore_ascii_case("test-data")
    });

    if is_test_data {
        let critical_system_dirs = ["windows", "system32", "syswow64", "program files", "program files (x86)"];
        for dir in &critical_system_dirs {
            if path_str.contains(&format!("\\{dir}")) || path_str.contains(&format!("/{dir}")) {
                return true;
            }
        }
        return false;
    }
    
    // Check for user home directory
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_lowercase();
        if path_str.starts_with(&home_str) {
            return true;
        }
    }
    
    // Check for common user directories
    let dangerous_dirs = [
        "downloads",
        "documents",
        "desktop",
        "pictures",
        "music",
        "videos",
        "appdata",
        "localappdata",
        "programdata",
        "program files",
        "program files (x86)",
        "windows",
        "system32",
        "syswow64",
    ];
    
    for dir in &dangerous_dirs {
        if path_str.contains(&format!("\\{}\\", dir)) 
            || path_str.contains(&format!("/{}/", dir))
            || path_str.ends_with(&format!("\\{}", dir))
            || path_str.ends_with(&format!("/{}", dir)) {
            return true;
        }
    }
    
    // Check for project root (current directory)
    if let Ok(current) = std::env::current_dir() {
        let current_str = current.to_string_lossy().to_lowercase();
        if path_str.starts_with(&current_str) {
            return true;
        }
    }
    
    false
}

/// Validate that the output path is safe for generation
pub fn validate_output_path(path: &Path) -> Result<()> {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    
    if is_dangerous_path(&absolute) {
        anyhow::bail!(
            "Refusing to generate in dangerous path: {}\n\
            Please specify a dedicated test data directory (e.g., ./test-data/scenario-name)",
            absolute.display()
        );
    }
    
    // Ensure parent exists or can be created
    if let Some(parent) = absolute.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    
    Ok(())
}

/// Atomic manifest writer - writes to temp file then renames
pub struct AtomicManifestWriter;

impl AtomicManifestWriter {
    pub fn write<P: AsRef<std::path::Path>>(path: P, manifest: &crate::manifest::Manifest) -> Result<()> {
        let path = path.as_ref();
        let temp_path = path.with_extension("json.tmp");
        
        // Write to temp file
        let json = serde_json::to_string_pretty(manifest)?;
        std::fs::write(&temp_path, json)?;
        
        // Verify the temp file can be read back
        let verify_json = std::fs::read_to_string(&temp_path)?;
        let _verified: crate::manifest::Manifest = serde_json::from_str(&verify_json)?;
        
        // Atomic rename
        #[cfg(unix)]
        {
            std::fs::rename(&temp_path, path)?;
        }
        
        #[cfg(windows)]
        {
            // On Windows, remove target first if exists
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            std::fs::rename(&temp_path, path)?;
        }
        
        Ok(())
    }
}

/// Safe cleanup - only deletes directories with valid manifests
pub struct SafeCleanup;

impl SafeCleanup {
    /// Clean a generated test dataset directory
    pub fn clean<P: AsRef<std::path::Path>>(path: P, force: bool) -> Result<()> {
        let path = path.as_ref();
        
        // Verify it contains a valid manifest
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            anyhow::bail!(
                "Refusing to clean: {}\n\
                This directory does not contain a valid Test Data Generator manifest.",
                path.display()
            );
        }
        
        // Verify manifest belongs to our generator
        let manifest_content = std::fs::read_to_string(&manifest_path)?;
        let manifest: crate::manifest::Manifest = serde_json::from_str(&manifest_content)?;
        
        if manifest.schema_version != crate::manifest::MANIFEST_SCHEMA_VERSION {
            anyhow::bail!(
                "Refusing to clean: {}\n\
                Manifest schema version mismatch (expected {}, found {})",
                path.display(),
                crate::manifest::MANIFEST_SCHEMA_VERSION,
                manifest.schema_version
            );
        }
        
        // Calculate size for confirmation
        let (file_count, total_size) = Self::calculate_size(path)?;
        
        if !force {
            println!("Delete generated test dataset?");
            println!();
            println!("Path: {}", path.display());
            println!("Files: {}", file_count);
            println!("Size: {}", Self::format_bytes(total_size));
            println!();
            print!("[y/N] ");
            use std::io::{self, Write};
            io::stdout().flush()?;
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cleanup cancelled.");
                return Ok(());
            }
        }
        
        println!("Cleaning up {}...", path.display());
        
        // Remove the directory
        // On Windows, we need to handle read-only files
        Self::remove_dir_all(path)?;
        
        println!("✓ Cleanup complete");
        Ok(())
    }
    
    fn calculate_size(path: &Path) -> Result<(usize, u64)> {
        let mut file_count = 0;
        let mut total_size = 0u64;
        
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                file_count += 1;
                total_size += entry.metadata()?.len();
            }
        }
        
        Ok((file_count, total_size))
    }
    
    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;
        
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }
        
        if unit_idx == 0 {
            format!("{} {}", bytes, UNITS[unit_idx])
        } else {
            format!("{:.2} {}", size, UNITS[unit_idx])
        }
    }
    
    fn remove_dir_all(path: &Path) -> Result<()> {
        // Handle read-only files on Windows
        #[cfg(windows)]
        {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let mut perms = std::fs::metadata(entry.path())?.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    perms.set_readonly(false);
                    std::fs::set_permissions(entry.path(), perms)?;
                }
            }
        }
        
        // Remove all
        fs::remove_dir_all(path)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_dangerous_path_detection() {
        // Test root paths
        assert!(is_dangerous_path(Path::new("/")));
        assert!(is_dangerous_path(Path::new("C:\\")));
        
        // Test home directory
        if let Some(home) = dirs::home_dir() {
            assert!(is_dangerous_path(&home));
            assert!(is_dangerous_path(&home.join("Downloads")));
            assert!(is_dangerous_path(&home.join("Documents")));
            assert!(is_dangerous_path(&home.join("Desktop")));
        }
    }

    #[test]
    fn test_safe_path() {
        // Use a path that's definitely not under the home directory
        #[cfg(windows)]
        let safe_path = PathBuf::from("C:\\test-data\\scenario");
        #[cfg(not(windows))]
        let safe_path = PathBuf::from("/tmp/test-data/scenario");
        
        // Create the path if it doesn't exist
        let _ = std::fs::create_dir_all(&safe_path);
        assert!(!is_dangerous_path(&safe_path));
    }

    #[test]
    fn test_appdata_testdata_path_is_safe() {
        let app_data = dirs::data_dir().unwrap_or_else(|| PathBuf::from("C:\\AppData"));
        let path = app_data.join("com.fileharbor.app").join("TestData").join("phase1-baseline");
        assert!(!is_dangerous_path(&path));
    }

    #[test]
    fn test_atomic_manifest_write() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manifest_path = temp_dir.path().join("manifest.json");
        
        let manifest = crate::manifest::Manifest::new(
            crate::manifest::ScenarioType::Phase1Baseline,
            12345,
            "/test".to_string(),
        );
        
        AtomicManifestWriter::write(&manifest_path, &manifest).unwrap();
        assert!(manifest_path.exists());
        
        // Verify it can be read back
        let content = std::fs::read_to_string(&manifest_path).unwrap();
        let _parsed: crate::manifest::Manifest = serde_json::from_str(&content).unwrap();
    }

    #[test]
    fn test_cleanup_refuses_non_manifest_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = SafeCleanup::clean(temp_dir.path(), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not contain a valid Test Data Generator manifest"));
    }

    #[test]
    fn test_cleanup_works_with_manifest() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manifest_path = temp_dir.path().join("manifest.json");
        
        let manifest = crate::manifest::Manifest::new(
            crate::manifest::ScenarioType::Phase1Baseline,
            12345,
            "/test".to_string(),
        );
        
        crate::safety::AtomicManifestWriter::write(&manifest_path, &manifest).unwrap();
        
        // Create a dummy file
        fs::write(temp_dir.path().join("dummy.txt"), b"test").unwrap();
        
        let result = SafeCleanup::clean(temp_dir.path(), true);
        assert!(result.is_ok());
        assert!(!temp_dir.path().exists());
    }
}