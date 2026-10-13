//! Application settings with safe defaults.

use serde::{Deserialize, Serialize};

use crate::store::Store;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Remembered default scan location (never auto-scanned).
    #[serde(default)]
    pub default_scan_location: Option<String>,
    /// Days to keep quarantined files before suggesting cleanup.
    /// Phase 1 never deletes automatically regardless of this value.
    #[serde(default = "default_retention")]
    pub quarantine_retention_days: u32,
    /// Parallel hashing workers.
    #[serde(default = "default_concurrency")]
    pub max_hash_concurrency: usize,
    /// Automatic shielding of sensitive directories (.ssh, .aws, credentials, etc.)
    #[serde(default = "default_true")]
    pub privacy_shield_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_retention() -> u32 {
    90
}

fn default_concurrency() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_scan_location: None,
            quarantine_retention_days: default_retention(),
            max_hash_concurrency: default_concurrency(),
            privacy_shield_enabled: true,
        }
    }
}

impl AppSettings {
    pub fn load(store: &Store) -> Self {
        store
            .get_setting("app_settings")
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, store: &Store) -> Result<(), String> {
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        store.set_setting("app_settings", &json)
    }
}
