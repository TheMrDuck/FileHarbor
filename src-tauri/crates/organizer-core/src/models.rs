use serde::{Deserialize, Serialize};

/// Lifecycle state of a single file within a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Ready,
    Scanning,
    Hashing,
    Completed,
    Inaccessible,
    ChangedDuringScan,
    HashFailed,
    Skipped,
}

impl ScanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScanStatus::Ready => "ready",
            ScanStatus::Scanning => "scanning",
            ScanStatus::Hashing => "hashing",
            ScanStatus::Completed => "completed",
            ScanStatus::Inaccessible => "inaccessible",
            ScanStatus::ChangedDuringScan => "changed_during_scan",
            ScanStatus::HashFailed => "hash_failed",
            ScanStatus::Skipped => "skipped",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "ready" => ScanStatus::Ready,
            "scanning" => ScanStatus::Scanning,
            "hashing" => ScanStatus::Hashing,
            "inaccessible" => ScanStatus::Inaccessible,
            "changed_during_scan" => ScanStatus::ChangedDuringScan,
            "hash_failed" => ScanStatus::HashFailed,
            "skipped" => ScanStatus::Skipped,
            _ => ScanStatus::Completed,
        }
    }
}

/// State of the content hash for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashStatus {
    NotComputed,
    Pending,
    Computed,
    Failed,
    Unstable,
}

impl HashStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HashStatus::NotComputed => "not_computed",
            HashStatus::Pending => "pending",
            HashStatus::Computed => "computed",
            HashStatus::Failed => "failed",
            HashStatus::Unstable => "unstable",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "pending" => HashStatus::Pending,
            "computed" => HashStatus::Computed,
            "failed" => HashStatus::Failed,
            "unstable" => HashStatus::Unstable,
            _ => HashStatus::NotComputed,
        }
    }
}

/// Lifecycle of a scan session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Running => "running",
            SessionStatus::Completed => "completed",
            SessionStatus::Cancelled => "cancelled",
            SessionStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "cancelled" => SessionStatus::Cancelled,
            "failed" => SessionStatus::Failed,
            "running" => SessionStatus::Running,
            _ => SessionStatus::Completed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Quarantine,
    Restore,
    DeletePermanent,
    Backup,
    BackupVerify,
    BackupRestore,
    BackupCancel,
}

impl OperationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationKind::Quarantine => "quarantine",
            OperationKind::Restore => "restore",
            OperationKind::DeletePermanent => "delete_permanent",
            OperationKind::Backup => "backup",
            OperationKind::BackupVerify => "backup_verify",
            OperationKind::BackupRestore => "backup_restore",
            OperationKind::BackupCancel => "backup_cancel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    PartiallyCompleted,
    Cancelled,
}

impl OperationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationStatus::Pending => "pending",
            OperationStatus::InProgress => "in_progress",
            OperationStatus::Completed => "completed",
            OperationStatus::Failed => "failed",
            OperationStatus::PartiallyCompleted => "partially_completed",
            OperationStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "pending" => OperationStatus::Pending,
            "in_progress" => OperationStatus::InProgress,
            "failed" => OperationStatus::Failed,
            "partially_completed" => OperationStatus::PartiallyCompleted,
            "cancelled" => OperationStatus::Cancelled,
            _ => OperationStatus::Completed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpItemStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl OpItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpItemStatus::Pending => "pending",
            OpItemStatus::InProgress => "in_progress",
            OpItemStatus::Completed => "completed",
            OpItemStatus::Failed => "failed",
            OpItemStatus::Skipped => "skipped",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "pending" => OpItemStatus::Pending,
            "in_progress" => OpItemStatus::InProgress,
            "failed" => OpItemStatus::Failed,
            "skipped" => OpItemStatus::Skipped,
            _ => OpItemStatus::Completed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineItemStatus {
    Stored,
    Restored,
    Deleted,
}

impl QuarantineItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuarantineItemStatus::Stored => "stored",
            QuarantineItemStatus::Restored => "restored",
            QuarantineItemStatus::Deleted => "deleted",
        }
    }
}

/// A single file discovered during a scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub session_id: i64,
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    /// Unix epoch milliseconds, if the platform provides the timestamp.
    pub created_at_ms: Option<i64>,
    pub modified_at_ms: Option<i64>,
    pub accessed_at_ms: Option<i64>,
    pub is_empty: bool,
    pub hash: Option<String>,
    pub hash_algorithm: Option<String>,
    pub hash_status: HashStatus,
    pub scan_status: ScanStatus,
    pub error: Option<String>,
}

/// New file record before it has a database id.
#[derive(Debug, Clone)]
pub struct NewFileRecord {
    pub session_id: i64,
    pub path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub created_at_ms: Option<i64>,
    pub modified_at_ms: Option<i64>,
    pub accessed_at_ms: Option<i64>,
    pub is_empty: bool,
    pub scan_status: ScanStatus,
    pub error: Option<String>,
}

/// A scan run over one or more root directories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSession {
    pub id: i64,
    pub roots: Vec<String>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub status: SessionStatus,
}

/// Aggregated statistics for a completed (or cancelled) scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub session: ScanSession,
    pub files_scanned: u64,
    pub total_size_bytes: u64,
    pub empty_files: u64,
    pub inaccessible_files: u64,
    pub error_files: u64,
    pub duplicate_groups: u64,
    pub duplicate_files: u64,
    pub potential_reclaimable_bytes: u64,
}

/// A group of files with byte-identical content (same size + same SHA-256).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub id: i64,
    pub session_id: i64,
    pub hash: String,
    pub size_bytes: u64,
    pub members: Vec<FileRecord>,
    /// Recommendation only - the user always decides which copies to keep.
    pub recommended_keep_file_id: i64,
}

impl DuplicateGroup {
    pub fn reclaimable_bytes(&self) -> u64 {
        self.size_bytes
            .saturating_mul(self.members.len().saturating_sub(1) as u64)
    }
}

/// A file currently held in the application quarantine area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineItem {
    pub id: i64,
    pub operation_id: i64,
    pub original_path: String,
    pub quarantine_path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub hash: Option<String>,
    pub quarantined_at_ms: i64,
    pub status: QuarantineItemStatus,
}

/// A batch filesystem operation recorded in the operation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperation {
    pub id: i64,
    pub batch_id: String,
    pub kind: OperationKind,
    pub status: OperationStatus,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub detail: Option<String>,
}

/// Per-file result line belonging to a [`FileOperation`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationItem {
    pub id: i64,
    pub operation_id: i64,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub file_id: Option<i64>,
    pub status: OpItemStatus,
    pub error: Option<String>,
}

/// Outcome of a per-item action inside a batch operation.
#[derive(Debug, Clone)]
pub enum ItemOutcome {
    Success,
    Failure(String),
    Conflict(String),
}

/// Result of executing a batch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchReport {
    pub operation_id: i64,
    pub batch_id: String,
    pub succeeded: usize,
    pub failed: usize,
    pub failures: Vec<FailedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedItem {
    pub path: String,
    pub error: String,
}

