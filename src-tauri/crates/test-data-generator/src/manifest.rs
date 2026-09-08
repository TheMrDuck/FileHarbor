use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Schema version for the manifest format
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Categories for file classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FileCategory {
    Normal,
    ExactDuplicateOriginal,
    ExactDuplicateCopy,
    SameSizeDifferentContent,
    Empty,
    Nested,
    Unicode,
    LongName,
    LargeFile,
    Symlink,
    BrokenSymlink,
    Inaccessible,
    Unstable,
    Missing,
    BackupLikeCopy,
}

impl FileCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileCategory::Normal => "normal",
            FileCategory::ExactDuplicateOriginal => "exact_duplicate_original",
            FileCategory::ExactDuplicateCopy => "exact_duplicate_copy",
            FileCategory::SameSizeDifferentContent => "same_size_different_content",
            FileCategory::Empty => "empty",
            FileCategory::Nested => "nested",
            FileCategory::Unicode => "unicode",
            FileCategory::LongName => "long_name",
            FileCategory::LargeFile => "large_file",
            FileCategory::Symlink => "symlink",
            FileCategory::BrokenSymlink => "broken_symlink",
            FileCategory::Inaccessible => "inaccessible",
            FileCategory::Unstable => "unstable",
            FileCategory::Missing => "missing",
            FileCategory::BackupLikeCopy => "backup_like_copy",
        }
    }
}

impl std::fmt::Display for FileCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Individual file record in the manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub relative_path: String,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub categories: Vec<FileCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_duplicate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_empty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<DateTime<Utc>>,
}

/// Summary statistics for the manifest
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestSummary {
    pub files: usize,
    pub directories: usize,
    pub empty_files: usize,
    pub duplicate_groups: usize,
    pub duplicate_files: usize,
    pub unicode_files: usize,
    pub large_files: usize,
    pub nested_files: usize,
    pub same_size_different_content_pairs: usize,
    pub symlinks: usize,
    pub broken_symlinks: usize,
    pub backup_like_copies: usize,
}

/// Main manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub generator_version: String,
    pub scenario: ScenarioType,
    pub seed: u64,
    pub generated_at: DateTime<Utc>,
    pub root: String,
    pub summary: ManifestSummary,
    pub files: Vec<FileRecord>,
}

/// Backup manifest structure (for backup destination)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub backup_set_id: String,
    pub snapshot_id: String,
    pub created_at: String,
    pub status: String,
    pub source: ManifestSource,
    pub files: Vec<ManifestFile>,
}

/// Source information for a backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSource {
    pub label: String,
    pub path: String,
}

/// File entry in a backup manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub verified: bool,
}

impl Manifest {
    pub fn new(scenario: ScenarioType, seed: u64, root: String) -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            generator_version: GENERATOR_VERSION.to_string(),
            scenario,
            seed,
            generated_at: Utc::now(),
            root,
            summary: ManifestSummary::default(),
            files: Vec::new(),
        }
    }

    pub fn add_file(&mut self, record: FileRecord) {
        // Update summary counts
        self.summary.files += 1;

        for cat in &record.categories {
            match cat {
                FileCategory::Empty => self.summary.empty_files += 1,
                FileCategory::ExactDuplicateOriginal | FileCategory::ExactDuplicateCopy => {
                    self.summary.duplicate_files += 1;
                }
                FileCategory::Unicode => self.summary.unicode_files += 1,
                FileCategory::LargeFile => self.summary.large_files += 1,
                FileCategory::Nested => self.summary.nested_files += 1,
                FileCategory::Symlink => self.summary.symlinks += 1,
                FileCategory::BrokenSymlink => self.summary.broken_symlinks += 1,
                FileCategory::BackupLikeCopy => self.summary.backup_like_copies += 1,
                _ => {}
            }
        }

        if record.duplicate_group.is_some() {
            // Count unique duplicate groups
        }

        self.files.push(record);
    }

    pub fn finalize(&mut self) {
        // Calculate duplicate groups from file records
        let mut groups: HashMap<String, usize> = HashMap::new();
        for f in &self.files {
            if let Some(group) = &f.duplicate_group {
                *groups.entry(group.clone()).or_default() += 1;
            }
        }
        self.summary.duplicate_groups = groups.len();
    }
}

/// Scenario types supported by the generator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ScenarioType {
    Phase1Baseline,
    Duplicates,
    EmptyFiles,
    Unicode,
    LargeFiles,
    PerformanceSmall,
    PerformanceMedium,
    PerformanceLarge,
    PerformanceStress,
    RecoveredHdd,
    Unstable,
    MissingFiles,
    Symlinks,
    LockedFiles,
    Permissions,
    Regression,
    BackupBaseline,
    BackupIncremental,
    BackupChangedFiles,
    BackupDeletedFiles,
    BackupRestore,
    BackupCorruption,
    BackupInterrupted,
    BackupLarge,
    BackupDuplicateContent,
}

impl std::fmt::Display for ScenarioType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            ScenarioType::Phase1Baseline => "phase1-baseline",
            ScenarioType::Duplicates => "duplicates",
            ScenarioType::EmptyFiles => "empty-files",
            ScenarioType::Unicode => "unicode",
            ScenarioType::LargeFiles => "large-files",
            ScenarioType::PerformanceSmall => "performance-small",
            ScenarioType::PerformanceMedium => "performance-medium",
            ScenarioType::PerformanceLarge => "performance-large",
            ScenarioType::PerformanceStress => "performance-stress",
            ScenarioType::RecoveredHdd => "recovered-hdd",
            ScenarioType::Unstable => "unstable",
            ScenarioType::MissingFiles => "missing-files",
            ScenarioType::Symlinks => "symlinks",
            ScenarioType::LockedFiles => "locked-files",
            ScenarioType::Permissions => "permissions",
            ScenarioType::Regression => "regression",
            ScenarioType::BackupBaseline => "backup-baseline",
            ScenarioType::BackupIncremental => "backup-incremental",
            ScenarioType::BackupChangedFiles => "backup-changed-files",
            ScenarioType::BackupDeletedFiles => "backup-deleted-files",
            ScenarioType::BackupRestore => "backup-restore",
            ScenarioType::BackupCorruption => "backup-corruption",
            ScenarioType::BackupInterrupted => "backup-interrupted",
            ScenarioType::BackupLarge => "backup-large",
            ScenarioType::BackupDuplicateContent => "backup-duplicate-content",
        })
    }
}

impl std::str::FromStr for ScenarioType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "phase1-baseline" => Ok(ScenarioType::Phase1Baseline),
            "duplicates" => Ok(ScenarioType::Duplicates),
            "empty-files" => Ok(ScenarioType::EmptyFiles),
            "unicode" => Ok(ScenarioType::Unicode),
            "large-files" => Ok(ScenarioType::LargeFiles),
            "performance-small" => Ok(ScenarioType::PerformanceSmall),
            "performance-medium" => Ok(ScenarioType::PerformanceMedium),
            "performance-large" => Ok(ScenarioType::PerformanceLarge),
            "performance-stress" => Ok(ScenarioType::PerformanceStress),
            "recovered-hdd" => Ok(ScenarioType::RecoveredHdd),
            "unstable" => Ok(ScenarioType::Unstable),
            "missing-files" => Ok(ScenarioType::MissingFiles),
            "symlinks" => Ok(ScenarioType::Symlinks),
            "locked-files" => Ok(ScenarioType::LockedFiles),
            "permissions" => Ok(ScenarioType::Permissions),
            "regression" => Ok(ScenarioType::Regression),
            "backup-baseline" => Ok(ScenarioType::BackupBaseline),
            "backup-incremental" => Ok(ScenarioType::BackupIncremental),
            "backup-changed-files" => Ok(ScenarioType::BackupChangedFiles),
            "backup-deleted-files" => Ok(ScenarioType::BackupDeletedFiles),
            "backup-restore" => Ok(ScenarioType::BackupRestore),
            "backup-corruption" => Ok(ScenarioType::BackupCorruption),
            "backup-interrupted" => Ok(ScenarioType::BackupInterrupted),
            "backup-large" => Ok(ScenarioType::BackupLarge),
            "backup-duplicate-content" => Ok(ScenarioType::BackupDuplicateContent),
            _ => anyhow::bail!("Unknown scenario type: {}", s),
        }
    }
}