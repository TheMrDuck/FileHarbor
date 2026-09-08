//! Backup scheduling: determines when backup runs should occur.
//!
//! Supports Manual, Daily, Weekly, and DriveConnected schedules.
//! The scheduler is intentionally simple — no cron-like complexity.
//!
//! Note: DriveConnected scheduling requires destination path resolution
//! which is handled by the caller (the engine or Tauri command layer).

use crate::backup::models::BackupSchedule;
use std::path::Path;

/// Determines whether a backup set should run now based on its schedule.
/// For DriveConnected, the caller must check `is_drive_connected` separately.
pub fn should_run_now(backup_set: &BackupSet, now_ms: i64) -> bool {
    match backup_set.schedule {
        BackupSchedule::Manual => false,
        BackupSchedule::Daily => {
            let last_run = get_last_run_time(backup_set.id).unwrap_or(0);
            if last_run == 0 {
                return true;
            }
            let day_ms = 24 * 60 * 60 * 1000;
            now_ms - last_run >= day_ms
        }
        BackupSchedule::Weekly => {
            let last_run = get_last_run_time(backup_set.id).unwrap_or(0);
            if last_run == 0 {
                return true;
            }
            let week_ms = 7 * 24 * 60 * 60 * 1000;
            now_ms - last_run >= week_ms
        }
        BackupSchedule::OnExternalDriveConnected => {
            // Caller must check destination connectivity separately
            false
        }
    }
}

/// Returns the timestamp of the last completed backup run for a set, if any.
/// This is a placeholder — the actual implementation queries the database
/// via the store. Callers should use `get_last_run_time_from_store` instead.
pub fn get_last_run_time(_backup_set_id: i64) -> Option<i64> {
    None
}

/// Computes the next scheduled run time for a backup set.
pub fn next_run_time(backup_set: &BackupSet, now_ms: i64) -> Option<i64> {
    match backup_set.schedule {
        BackupSchedule::Manual => None,
        BackupSchedule::Daily => Some(now_ms + 24 * 60 * 60 * 1000),
        BackupSchedule::Weekly => Some(now_ms + 7 * 24 * 60 * 60 * 1000),
        BackupSchedule::OnExternalDriveConnected => None,
    }
}

/// Checks if a drive at the given path is currently connected.
pub fn is_drive_connected(path: &str) -> bool {
    Path::new(path).exists()
}

/// Returns a human-readable description of the schedule.
pub fn schedule_description(schedule: BackupSchedule) -> &'static str {
    match schedule {
        BackupSchedule::Manual => "Manual",
        BackupSchedule::Daily => "Daily",
        BackupSchedule::Weekly => "Weekly",
        BackupSchedule::OnExternalDriveConnected => "When drive is connected",
    }
}

/// Reference to BackupSet for the scheduler module.
pub use crate::backup::models::BackupSet;