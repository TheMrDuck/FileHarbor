//! Structured logging for backup operations.
//!
//! Logs must be useful without leaking unnecessary sensitive information.
//! Avoid dumping paths or filenames into logs unnecessarily.

use serde::{Serialize, Deserialize};
use std::sync::Mutex;
use std::collections::VecDeque;

/// Log levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

/// A log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp_ms: i64,
    pub level: LogLevel,
    pub component: String,
    pub message: String,
    pub context: Option<String>,
}

/// Simple in-memory log buffer for backup operations.
pub struct BackupLogger {
    entries: Mutex<VecDeque<LogEntry>>,
    max_entries: usize,
}

impl BackupLogger {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_entries)),
            max_entries,
        }
    }

    /// Logs a message at the given level.
    pub fn log(&self, level: LogLevel, component: &str, message: &str) {
        self.log_with_context(level, component, message, None);
    }

    /// Logs a message with additional context.
    pub fn log_with_context(&self, level: LogLevel, component: &str, message: &str, context: Option<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let entry = LogEntry {
            timestamp_ms: now,
            level,
            component: component.to_string(),
            message: message.to_string(),
            context,
        };
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= self.max_entries {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    /// Returns all log entries.
    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().map(|e| e.iter().cloned().collect()).unwrap_or_default()
    }

    /// Returns entries at or above the given level.
    pub fn entries_at_level(&self, level: LogLevel) -> Vec<LogEntry> {
        self.entries().into_iter().filter(|e| e.level >= level).collect()
    }

    /// Clears all log entries.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

impl Default for BackupLogger {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Convenience methods for common log levels.
impl BackupLogger {
    pub fn info(&self, component: &str, message: &str) {
        self.log(LogLevel::Info, component, message);
    }
    pub fn warn(&self, component: &str, message: &str) {
        self.log(LogLevel::Warn, component, message);
    }
    pub fn error(&self, component: &str, message: &str) {
        self.log(LogLevel::Error, component, message);
    }
    pub fn debug(&self, component: &str, message: &str) {
        self.log(LogLevel::Debug, component, message);
    }
}