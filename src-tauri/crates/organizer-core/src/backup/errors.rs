//! Backup error types.

use std::fmt;
use thiserror::Error;
use serde::{Deserialize, Serialize};

/// Errors that can occur during backup operations.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Source path does not exist: {0}")]
    SourceMissing(String),

    #[error("Source file changed during backup: {0}")]
    SourceChanged(String),

    #[error("Source file is unreadable: {0}")]
    SourceUnreadable(String),

    #[error("Destination is unavailable: {0}")]
    DestinationUnavailable(String),

    #[error("Destination is read-only: {0}")]
    DestinationReadOnly(String),

    #[error("Insufficient space: required {required} bytes, available {available} bytes")]
    InsufficientSpace { required: u64, available: u64 },

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Transfer failed: {0}")]
    TransferFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Manifest error: {0}")]
    ManifestError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Destination is inside source: {0}")]
    DestinationInsideSource(String),

    #[error("Source is inside destination: {0}")]
    SourceInsideDestination(String),

    #[error("Symlink would escape destination: {0}")]
    SymlinkEscape(String),

    #[error("File was unstable during hash: {0}")]
    UnstableFile(String),

    #[error("Backup run not found: {0}")]
    RunNotFound(i64),

    #[error("Backup set not found: {0}")]
    SetNotFound(i64),

    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(i64),

    #[error("Content not found: {0}")]
    ContentNotFound(String),

    #[error("Path traversal attempt: {0}")]
    PathTraversal(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl BackupError {
    /// Classify the error for UI display.
    pub fn classification(&self) -> ErrorClassification {
        match self {
            BackupError::SourceMissing(_) => ErrorClassification::SourceMissing,
            BackupError::SourceChanged(_) => ErrorClassification::SourceChanged,
            BackupError::SourceUnreadable(_) => ErrorClassification::SourceUnreadable,
            BackupError::DestinationUnavailable(_) => ErrorClassification::DestinationUnavailable,
            BackupError::DestinationReadOnly(_) => ErrorClassification::DestinationReadOnly,
            BackupError::InsufficientSpace { .. } => ErrorClassification::InsufficientSpace,
            BackupError::PermissionDenied(_) => ErrorClassification::PermissionDenied,
            BackupError::TransferFailed(_) => ErrorClassification::TransferFailed,
            BackupError::VerificationFailed(_) => ErrorClassification::VerificationFailed,
            BackupError::Cancelled => ErrorClassification::Cancelled,
            BackupError::ManifestError(_) => ErrorClassification::ManifestError,
            BackupError::DatabaseError(_) => ErrorClassification::DatabaseError,
            BackupError::InvalidConfiguration(_) => ErrorClassification::InvalidConfiguration,
            BackupError::DestinationInsideSource(_) => ErrorClassification::InvalidConfiguration,
            BackupError::SourceInsideDestination(_) => ErrorClassification::InvalidConfiguration,
            BackupError::SymlinkEscape(_) => ErrorClassification::InvalidConfiguration,
            BackupError::UnstableFile(_) => ErrorClassification::SourceChanged,
            BackupError::RunNotFound(_) => ErrorClassification::DatabaseError,
            BackupError::SetNotFound(_) => ErrorClassification::DatabaseError,
            BackupError::SnapshotNotFound(_) => ErrorClassification::DatabaseError,
            BackupError::ContentNotFound(_) => ErrorClassification::DatabaseError,
            BackupError::PathTraversal(_) => ErrorClassification::InvalidConfiguration,
            BackupError::UnsupportedFileType(_) => ErrorClassification::SourceUnreadable,
            BackupError::Internal(_) => ErrorClassification::Unknown,
        }
    }

    /// Human-readable message for UI.
    pub fn user_message(&self) -> String {
        match self {
            BackupError::SourceMissing(path) => format!("Source file not found: {}", path),
            BackupError::SourceChanged(path) => format!("File changed during backup: {}", path),
            BackupError::SourceUnreadable(path) => format!("Cannot read file: {}", path),
            BackupError::DestinationUnavailable(path) => format!("Backup destination unavailable: {}", path),
            BackupError::DestinationReadOnly(path) => format!("Backup destination is read-only: {}", path),
            BackupError::InsufficientSpace { required, available } => {
                format!(
                    "Not enough space. Required: {} bytes, Available: {} bytes",
                    required, available
                )
            }
            BackupError::PermissionDenied(path) => format!("Permission denied: {}", path),
            BackupError::TransferFailed(msg) => format!("File copy failed: {}", msg),
            BackupError::VerificationFailed(msg) => format!("Verification failed: {}", msg),
            BackupError::Cancelled => "Backup cancelled by user".to_string(),
            BackupError::ManifestError(msg) => format!("Backup metadata error: {}", msg),
            BackupError::DatabaseError(msg) => format!("Database error: {}", msg),
            BackupError::InvalidConfiguration(msg) => format!("Invalid backup configuration: {}", msg),
            BackupError::DestinationInsideSource(path) => format!("Destination cannot be inside source: {}", path),
            BackupError::SourceInsideDestination(path) => format!("Source cannot be inside destination: {}", path),
            BackupError::SymlinkEscape(path) => format!("Symlink would escape backup boundary: {}", path),
            BackupError::UnstableFile(path) => format!("File was modified during backup: {}", path),
            BackupError::RunNotFound(id) => format!("Backup run not found: {}", id),
            BackupError::SetNotFound(id) => format!("Backup set not found: {}", id),
            BackupError::SnapshotNotFound(id) => format!("Backup snapshot not found: {}", id),
            BackupError::ContentNotFound(hash) => format!("Content not found: {}", hash),
            BackupError::PathTraversal(path) => format!("Invalid path (traversal attempt): {}", path),
            BackupError::UnsupportedFileType(path) => format!("Unsupported file type: {}", path),
            BackupError::Internal(msg) => format!("Internal error: {}", msg),
        }
    }
}

/// High-level error classifications for UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClassification {
    SourceMissing,
    SourceChanged,
    SourceUnreadable,
    DestinationUnavailable,
    DestinationReadOnly,
    InsufficientSpace,
    PermissionDenied,
    TransferFailed,
    VerificationFailed,
    Cancelled,
    ManifestError,
    DatabaseError,
    InvalidConfiguration,
    Unknown,
}

impl fmt::Display for ErrorClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ErrorClassification::SourceMissing => "SourceMissing",
            ErrorClassification::SourceChanged => "SourceChanged",
            ErrorClassification::SourceUnreadable => "SourceUnreadable",
            ErrorClassification::DestinationUnavailable => "DestinationUnavailable",
            ErrorClassification::DestinationReadOnly => "DestinationReadOnly",
            ErrorClassification::InsufficientSpace => "InsufficientSpace",
            ErrorClassification::PermissionDenied => "PermissionDenied",
            ErrorClassification::TransferFailed => "TransferFailed",
            ErrorClassification::VerificationFailed => "VerificationFailed",
            ErrorClassification::Cancelled => "Cancelled",
            ErrorClassification::ManifestError => "ManifestError",
            ErrorClassification::DatabaseError => "DatabaseError",
            ErrorClassification::InvalidConfiguration => "InvalidConfiguration",
            ErrorClassification::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

impl From<&str> for BackupError {
    fn from(s: &str) -> Self {
        BackupError::Internal(s.to_string())
    }
}

impl From<String> for BackupError {
    fn from(s: String) -> Self {
        BackupError::Internal(s)
    }
}

impl From<&String> for BackupError {
    fn from(s: &String) -> Self {
        BackupError::Internal(s.clone())
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::TransferFailed(e.to_string())
    }
}

impl From<rusqlite::Error> for BackupError {
    fn from(e: rusqlite::Error) -> Self {
        BackupError::DatabaseError(e.to_string())
    }
}