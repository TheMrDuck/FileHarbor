use crate::manifest::{FileCategory, FileRecord, Manifest};
use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Verification result
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub expected_files: usize,
    pub found_files: usize,
    pub missing_files: Vec<String>,
    pub unexpected_files: Vec<String>,
    pub size_mismatches: Vec<SizeMismatch>,
    pub hash_mismatches: Vec<HashMismatch>,
    pub duplicate_group_results: Vec<DuplicateGroupResult>,
    pub empty_file_results: Vec<EmptyFileResult>,
    pub missing_expected: usize,
    pub missing_actual: usize,
}

#[derive(Debug, Clone)]
pub struct SizeMismatch {
    pub path: String,
    pub expected: u64,
    pub actual: u64,
}

#[derive(Debug, Clone)]
pub struct HashMismatch {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroupResult {
    pub group_id: String,
    pub expected_count: usize,
    pub actual_count: usize,
    pub members: Vec<String>,
    pub all_hashes_match: bool,
}

#[derive(Debug, Clone)]
pub struct EmptyFileResult {
    pub path: String,
    pub expected_empty: bool,
    pub actual_empty: bool,
}

/// Verify a generated dataset against its manifest
pub fn verify_dataset<P: AsRef<std::path::Path>>(manifest_path: P) -> Result<VerificationResult> {
    let manifest_path = manifest_path.as_ref();
    
    // Read manifest
    let manifest_content = std::fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_content)?;
    
    let root_dir = manifest_path.parent().unwrap();
    let files_dir = root_dir.join("files");
    
    println!("Dataset Verification");
    println!();
    println!("Manifest: {}", manifest_path.display());
    println!("Scenario: {}", manifest.scenario);
    println!("Seed: {}", manifest.seed);
    println!();
    
    let mut result = VerificationResult {
        expected_files: manifest.files.len(),
        found_files: 0,
        missing_files: Vec::new(),
        unexpected_files: Vec::new(),
        size_mismatches: Vec::new(),
        hash_mismatches: Vec::new(),
        duplicate_group_results: Vec::new(),
        empty_file_results: Vec::new(),
        missing_expected: 0,
        missing_actual: 0,
    };
    
    // Build a map of expected files by relative path
    let mut expected_by_path: HashMap<String, &FileRecord> = HashMap::new();
    for file in &manifest.files {
        expected_by_path.insert(file.relative_path.clone(), file);
    }
    
    // Scan actual files
    let mut found_paths = std::collections::HashSet::new();
    
    if files_dir.exists() {
        for entry in WalkDir::new(&files_dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let relative = entry.path().strip_prefix(&files_dir)?
                    .to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
                    .to_string();
                found_paths.insert(relative.clone());
                result.found_files += 1;
                
                // Check against expected
                if let Some(expected) = expected_by_path.get(&relative) {
                    verify_file(&relative, entry.path(), expected, &mut result)?;
                } else {
                    result.unexpected_files.push(relative);
                }
            }
        }
    }
    
    // Check for missing files
    for (path, expected) in &expected_by_path {
        if !found_paths.contains(path) {
            // Check if it's expected to be missing
            if expected.categories.contains(&FileCategory::Missing) {
                result.missing_expected += 1;
            } else {
                result.missing_files.push(path.clone());
            }
        }
    }
    
    // Verify duplicate groups
    verify_duplicate_groups(&manifest, &files_dir, &mut result)?;
    
    // Verify empty files
    verify_empty_files(&manifest, &mut result)?;
    
    // Print summary
    print_verification_summary(&result);
    
    Ok(result)
}

fn verify_file(
    relative_path: &str,
    actual_path: &Path,
    expected: &FileRecord,
    result: &mut VerificationResult,
) -> Result<()> {
    let metadata = std::fs::metadata(actual_path)?;
    let actual_size = metadata.len();
    
    // Check size
    if actual_size != expected.size_bytes {
        result.size_mismatches.push(SizeMismatch {
            path: relative_path.to_string(),
            expected: expected.size_bytes,
            actual: actual_size,
        });
    }
    
    // Check hash (for non-empty files)
    if expected.size_bytes > 0 {
        if let Some(expected_hash) = &expected.sha256 {
            let content = std::fs::read(actual_path)?;
            let actual_hash = format!("{:x}", Sha256::digest(&content));
            if actual_hash != *expected_hash {
                result.hash_mismatches.push(HashMismatch {
                    path: relative_path.to_string(),
                    expected: expected_hash.clone(),
                    actual: actual_hash,
                });
            }
        }
    } else if let Some(expected_hash) = &expected.sha256 {
        // Empty file - check against known empty hash
        let empty_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        if expected_hash != empty_hash {
            result.hash_mismatches.push(HashMismatch {
                path: relative_path.to_string(),
                expected: expected_hash.clone(),
                actual: empty_hash.to_string(),
            });
        }
    }
    
    Ok(())
}

fn verify_duplicate_groups(
    manifest: &Manifest,
    files_dir: &Path,
    result: &mut VerificationResult,
) -> Result<()> {
    // Group expected files by duplicate group
    let mut expected_groups: HashMap<String, Vec<&FileRecord>> = HashMap::new();
    for file in &manifest.files {
        if let Some(group) = &file.duplicate_group {
            expected_groups.entry(group.clone()).or_default().push(file);
        }
    }
    
    for (group_id, expected_members) in &expected_groups {
        let mut actual_members = Vec::new();
        let mut all_hashes_match = true;
        
        for member in expected_members {
            let full_path = files_dir.join(&member.relative_path);
            if full_path.exists() {
                actual_members.push(member.relative_path.clone());
                
                // Verify hash matches
                if member.size_bytes > 0 {
                    if let Some(expected_hash) = &member.sha256 {
                        let content = std::fs::read(&full_path)?;
                        let actual_hash = format!("{:x}", Sha256::digest(&content));
                        if actual_hash != *expected_hash {
                            all_hashes_match = false;
                        }
                    }
                }
            }
        }
        
        result.duplicate_group_results.push(DuplicateGroupResult {
            group_id: group_id.clone(),
            expected_count: expected_members.len(),
            actual_count: actual_members.len(),
            members: actual_members,
            all_hashes_match,
        });
    }
    
    Ok(())
}

fn verify_empty_files(manifest: &Manifest, result: &mut VerificationResult) -> Result<()> {
    for file in &manifest.files {
        let is_expected_empty = file.categories.contains(&FileCategory::Empty) || file.expected_empty == Some(true);
        let actual_empty = file.size_bytes == 0;
        
        result.empty_file_results.push(EmptyFileResult {
            path: file.relative_path.clone(),
            expected_empty: is_expected_empty,
            actual_empty,
        });
    }
    
    Ok(())
}

fn print_verification_summary(result: &VerificationResult) {
    println!("Expected files: {}", result.expected_files);
    println!("Found:          {}", result.found_files);
    println!();
    let total_missing = result.missing_files.len() + result.missing_expected;
    println!("Missing:            {} ({} unexpected, {} expected)", total_missing, result.missing_files.len(), result.missing_expected);
    println!("Unexpected:         {}", result.unexpected_files.len());
    println!("Size mismatches:    {}", result.size_mismatches.len());
    println!("Hash mismatches:    {}", result.hash_mismatches.len());
    println!();
    
    let total_groups = result.duplicate_group_results.len();
    let verified_groups = result.duplicate_group_results.iter()
        .filter(|g| g.actual_count == g.expected_count && g.all_hashes_match)
        .count();
    println!("Duplicate groups:");
    println!("  Expected: {}", total_groups);
    println!("  Verified: {}", verified_groups);
    println!();
    
    if result.missing_expected > 0 || result.missing_actual > 0 {
        println!("Expected missing: {}", result.missing_expected);
        println!("Actual missing:   {}", result.missing_actual);
        println!();
    }
    
    let has_errors = !result.missing_files.is_empty() 
        || result.missing_expected > 0
        || !result.unexpected_files.is_empty()
        || !result.size_mismatches.is_empty()
        || !result.hash_mismatches.is_empty()
        || result.duplicate_group_results.iter().any(|g| g.actual_count != g.expected_count || !g.all_hashes_match);
    
    if has_errors {
        println!("✗ Dataset has errors");
        
        if !result.missing_files.is_empty() || result.missing_expected > 0 {
            println!();
            println!("Missing from disk:");
            for f in &result.missing_files {
                println!("  - {} (missing from disk)", f);
            }
            if result.missing_expected > 0 {
                println!("  ... and {} more (expected missing per manifest)", result.missing_expected);
            }
        }
        
        if !result.unexpected_files.is_empty() {
            println!();
            println!("Unexpected files:");
            for f in &result.unexpected_files {
                println!("  - {}", f);
            }
        }
        
        if !result.size_mismatches.is_empty() {
            println!();
            println!("Size mismatches:");
            for m in &result.size_mismatches {
                println!("  - {}: expected {}, got {}", m.path, m.expected, m.actual);
            }
        }
        
        if !result.hash_mismatches.is_empty() {
            println!();
            println!("Hash mismatches:");
            for m in &result.hash_mismatches {
                println!("  - {}: expected {}..., got {}...", 
                    m.path, &m.expected[..16.min(m.expected.len())], &m.actual[..16.min(m.actual.len())]);
            }
        }
        
        for g in &result.duplicate_group_results {
            if g.actual_count != g.expected_count || !g.all_hashes_match {
                println!();
                println!("Duplicate group mismatch: {}", g.group_id);
                println!("  Expected: {} files, Actual: {} files", g.expected_count, g.actual_count);
                println!("  All hashes match: {}", g.all_hashes_match);
            }
        }
    } else {
        println!("✓ Dataset is valid");
    }
}

/// Quick verification - just check manifest exists and is valid
pub fn quick_verify<P: AsRef<std::path::Path>>(manifest_path: P) -> Result<bool> {
    let manifest_path = manifest_path.as_ref();
    if !manifest_path.exists() {
        return Ok(false);
    }
    
    let content = std::fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&content)?;
    
    // Check schema version
    if manifest.schema_version != crate::manifest::MANIFEST_SCHEMA_VERSION {
        return Ok(false);
    }
    
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentGenerator;
    use crate::generator::TestDataGenerator;
    use crate::manifest::{FileCategory, FileRecord, Manifest, ScenarioType};
    use crate::safety::AtomicManifestWriter;
    use tempfile::tempdir;

    #[test]
    fn test_verify_valid_dataset() {
        let temp_dir = tempdir().unwrap();
        let mut config = crate::scenarios::GenerationConfig::default();
        config.total_files = 100;
        
        let mut generator = TestDataGenerator::new_with_options("phase1-baseline", temp_dir.path().to_path_buf(), config, true).unwrap();
        let _manifest = generator.generate().unwrap();
        
        let manifest_path = temp_dir.path().join("manifest.json");
        let result = verify_dataset(&manifest_path).unwrap();
        
        assert_eq!(result.missing_files.len(), 0);
        assert_eq!(result.unexpected_files.len(), 0);
        assert_eq!(result.size_mismatches.len(), 0);
        assert_eq!(result.hash_mismatches.len(), 0);
    }

    #[test]
    fn test_verify_missing_file() {
        let temp_dir = tempdir().unwrap();
        let _manifest_path = temp_dir.path().join("manifest.json");
        
        // Create a simple manifest with one file
        let mut manifest = Manifest::new(
            ScenarioType::Phase1Baseline,
            12345,
            temp_dir.path().to_string_lossy().to_string(),
        );
        
        let record = FileRecord {
            id: "file-000001".to_string(),
            relative_path: "test.txt".to_string(),
            size_bytes: 100,
            sha256: Some("a".repeat(64)),
            categories: vec![FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: None,
            content_type: Some("test".to_string()),
            modified_at: None,
        };
        
        manifest.add_file(record);
        manifest.finalize();
        
        AtomicManifestWriter::write(&temp_dir.path().join("manifest.json"), &manifest).unwrap();
        
        // Don't create the actual file - it should be reported as missing
        let result = verify_dataset(&temp_dir.path().join("manifest.json")).unwrap();
        
        assert_eq!(result.missing_files.len(), 1);
    }
}