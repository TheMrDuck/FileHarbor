//! Backup data models.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Role of a file copy in the system.
pub use crate::models::CopyRole;

/// Status of a backup set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupSetStatus {
    Active,
    Outdated,
    AtRisk,
    Paused,
    Disabled,
}

impl BackupSetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupSetStatus::Active => "active",
            BackupSetStatus::Outdated => "outdated",
            BackupSetStatus::AtRisk => "at_risk",
            BackupSetStatus::Paused => "paused",
            BackupSetStatus::Disabled => "disabled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "active" => BackupSetStatus::Active,
            "outdated" => BackupSetStatus::Outdated,
            "at_risk" => BackupSetStatus::AtRisk,
            "paused" => BackupSetStatus::Paused,
            _ => BackupSetStatus::Disabled,
        }
    }
}

/// Status of a backup run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupRunStatus {
    Running,
    Verifying,
    Committing,
    Completed,
    Failed,
    Partial,
    Cancelled,
}

impl BackupRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupRunStatus::Running => "running",
            BackupRunStatus::Verifying => "verifying",
            BackupRunStatus::Committing => "committing",
            BackupRunStatus::Completed => "completed",
            BackupRunStatus::Failed => "failed",
            BackupRunStatus::Partial => "partial",
            BackupRunStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "running" => BackupRunStatus::Running,
            "verifying" => BackupRunStatus::Verifying,
            "committing" => BackupRunStatus::Committing,
            "completed" => BackupRunStatus::Completed,
            "failed" => BackupRunStatus::Failed,
            "partial" => BackupRunStatus::Partial,
            _ => BackupRunStatus::Cancelled,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            BackupRunStatus::Completed | BackupRunStatus::Failed | BackupRunStatus::Partial | BackupRunStatus::Cancelled
        )
    }
}

/// Status of a backup snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupSnapshotStatus {
    Verified,
    Unverified,
    Corrupted,
}

impl BackupSnapshotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupSnapshotStatus::Verified => "verified",
            BackupSnapshotStatus::Unverified => "unverified",
            BackupSnapshotStatus::Corrupted => "corrupted",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "verified" => BackupSnapshotStatus::Verified,
            "unverified" => BackupSnapshotStatus::Unverified,
            _ => BackupSnapshotStatus::Corrupted,
        }
    }
}

/// Status of an individual backed-up file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupFileStatus {
    Verified,
    Pending,
    Failed,
    Skipped,
    SourceMissing,
    Unstable,
}

impl BackupFileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupFileStatus::Verified => "verified",
            BackupFileStatus::Pending => "pending",
            BackupFileStatus::Failed => "failed",
            BackupFileStatus::Skipped => "skipped",
            BackupFileStatus::SourceMissing => "source_missing",
            BackupFileStatus::Unstable => "unstable",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "verified" => BackupFileStatus::Verified,
            "pending" => BackupFileStatus::Pending,
            "failed" => BackupFileStatus::Failed,
            "skipped" => BackupFileStatus::Skipped,
            "source_missing" => BackupFileStatus::SourceMissing,
            _ => BackupFileStatus::Unstable,
        }
    }
}

/// Protection status of a source file relative to backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileProtectionStatus {
    Protected,
    Outdated,
    NeverBackedUp,
    BackupMissing,
    SourceMissing,
    Unstable,
    Error,
}

impl FileProtectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileProtectionStatus::Protected => "protected",
            FileProtectionStatus::Outdated => "outdated",
            FileProtectionStatus::NeverBackedUp => "never_backed_up",
            FileProtectionStatus::BackupMissing => "backup_missing",
            FileProtectionStatus::SourceMissing => "source_missing",
            FileProtectionStatus::Unstable => "unstable",
            FileProtectionStatus::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "protected" => FileProtectionStatus::Protected,
            "outdated" => FileProtectionStatus::Outdated,
            "never_backed_up" => FileProtectionStatus::NeverBackedUp,
            "backup_missing" => FileProtectionStatus::BackupMissing,
            "source_missing" => FileProtectionStatus::SourceMissing,
            "unstable" => FileProtectionStatus::Unstable,
            _ => FileProtectionStatus::Error,
        }
    }
}

/// Schedule for automatic backups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackupSchedule {
    #[default]
    Manual,
    Daily,
    Weekly,
    OnExternalDriveConnected,
}

impl BackupSchedule {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupSchedule::Manual => "manual",
            BackupSchedule::Daily => "daily",
            BackupSchedule::Weekly => "weekly",
            BackupSchedule::OnExternalDriveConnected => "on_external_drive_connected",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "manual" => BackupSchedule::Manual,
            "daily" => BackupSchedule::Daily,
            "weekly" => BackupSchedule::Weekly,
            _ => BackupSchedule::OnExternalDriveConnected,
        }
    }
}

impl std::str::FromStr for BackupSchedule {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(BackupSchedule::parse(s))
    }
}

/// Verification mode for backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    Standard,
    Deep,
}

impl VerificationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationMode::Standard => "standard",
            VerificationMode::Deep => "deep",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "standard" => VerificationMode::Standard,
            _ => VerificationMode::Deep,
        }
    }
}

impl std::str::FromStr for VerificationMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(VerificationMode::parse(s))
    }
}

/// Retention policy for old snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RetentionPolicy {
    #[default]
    KeepAll,
    KeepLastN(u32),
}


impl RetentionPolicy {
    pub fn as_str(&self) -> String {
        match self {
            RetentionPolicy::KeepAll => "keep_all".to_string(),
            RetentionPolicy::KeepLastN(n) => format!("keep_last_{}", n),
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "keep_all" {
            RetentionPolicy::KeepAll
        } else if let Some(n) = s.strip_prefix("keep_last_") {
            n.parse().map(RetentionPolicy::KeepLastN).unwrap_or(RetentionPolicy::KeepAll)
        } else {
            RetentionPolicy::KeepAll
        }
    }
}

impl std::str::FromStr for RetentionPolicy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "keep_all" {
            Ok(RetentionPolicy::KeepAll)
        } else if let Some(n) = s.strip_prefix("keep_last_") {
            n.parse().map(RetentionPolicy::KeepLastN).map_err(|_| ())
        } else {
            Err(())
        }
    }
}

/// A backup set configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSet {
    pub id: i64,
    pub name: String,
    pub source_path: String,
    pub destination_id: i64,
    pub schedule: BackupSchedule,
    pub retention_policy: RetentionPolicy,
    pub verification_mode: VerificationMode,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// A backup destination with identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupDestination {
    pub id: i64,
    pub destination_id: String,
    pub path: String,
    pub label: Option<String>,
    pub filesystem_id: Option<String>,
    pub created_at_ms: i64,
    pub last_seen_at_ms: Option<i64>,
}

/// A single backup run execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRun {
    pub id: i64,
    pub backup_set_id: i64,
    pub snapshot_id: Option<i64>,
    pub status: BackupRunStatus,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub files_total: u64,
    pub files_new: u64,
    pub files_changed: u64,
    pub files_unchanged: u64,
    pub files_deleted: u64,
    pub files_failed: u64,
    pub bytes_transferred: u64,
    pub bytes_verified: u64,
    pub error_summary: Option<String>,
}

/// A backup snapshot (point-in-time state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub id: i64,
    pub backup_set_id: i64,
    pub run_id: i64,
    pub label: String,
    pub status: BackupSnapshotStatus,
    pub created_at_ms: i64,
    pub files_count: u64,
    pub total_size_bytes: u64,
}

/// A file in a backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub id: i64,
    pub snapshot_id: i64,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub status: BackupFileStatus,
    pub content_id: Option<i64>,
    pub error: Option<String>,
}

/// Content-addressable storage entry (deduplicated file content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupContent {
    pub id: i64,
    pub sha256: String,
    pub size_bytes: u64,
    pub reference_count: u64,
    pub first_seen_ms: i64,
}

/// Backup plan result from planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPlan {
    pub new_files: Vec<PlannedFile>,
    pub changed_files: Vec<PlannedFile>,
    pub unchanged_files: Vec<PlannedFile>,
    pub deleted_files: Vec<PlannedFile>,
    pub unavailable_files: Vec<PlannedFile>,
    pub total_files: u64,
    pub total_new_size: u64,
    pub total_changed_size: u64,
    pub estimated_transfer_bytes: u64,
}

/// A file in the backup plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedFile {
    pub source_path: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub modified_at_ms: Option<i64>,
}

/// Progress of a backup run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupProgress {
    pub run_id: i64,
    pub status: BackupRunStatus,
    pub current_file: Option<String>,
    pub files_processed: u64,
    pub files_total: u64,
    pub bytes_transferred: u64,
    pub bytes_total: u64,
    pub bytes_verified: u64,
    pub files_verified: u64,
    pub files_failed: u64,
    pub speed_bytes_per_sec: f64,
    pub eta_seconds: Option<u64>,
}

/// Backup health summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupHealth {
    pub total_files: u64,
    pub protected_files: u64,
    pub outdated_files: u64,
    pub never_backed_up_files: u64,
    pub protection_percentage: f64,
    pub sets: Vec<BackupSetHealth>,
}

/// Health of a single backup set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSetHealth {
    pub backup_set_id: i64,
    pub name: String,
    pub status: BackupSetStatus,
    pub last_verified_at_ms: Option<i64>,
    pub protected_files: u64,
    pub outdated_files: u64,
    pub never_backed_up_files: u64,
    pub protection_percentage: f64,
}

/// Backup destination validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationValidation {
    pub valid: bool,
    pub destination_id: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub available_space_bytes: Option<u64>,
    pub estimated_required_bytes: Option<u64>,
}

/// Restore conflict resolution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreConflictPolicy {
    KeepExisting,
    RestoreWithNewName,
    Cancel,
}

impl RestoreConflictPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            RestoreConflictPolicy::KeepExisting => "keep_existing",
            RestoreConflictPolicy::RestoreWithNewName => "restore_with_new_name",
            RestoreConflictPolicy::Cancel => "cancel",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "keep_existing" => RestoreConflictPolicy::KeepExisting,
            "restore_with_new_name" => RestoreConflictPolicy::RestoreWithNewName,
            _ => RestoreConflictPolicy::Cancel,
        }
    }
}

impl std::str::FromStr for RestoreConflictPolicy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RestoreConflictPolicy::parse(s))
    }
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub requested: u64,
    pub restored: u64,
    pub verified: u64,
    pub failed: u64,
    pub skipped: u64,
    pub conflicts_resolved: u64,
    pub errors: Vec<RestoreError>,
}

/// Individual restore error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreError {
    pub relative_path: String,
    pub error: String,
}

/// Summary of a completed backup run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRunSummary {
    pub run_id: i64,
    pub snapshot_id: i64,
    pub status: BackupRunStatus,
    pub files_total: u64,
    pub files_new: u64,
    pub files_changed: u64,
    pub files_unchanged: u64,
    pub files_deleted: u64,
    pub files_failed: u64,
    pub bytes_transferred: u64,
    pub bytes_verified: u64,
    pub duration_ms: u64,
}