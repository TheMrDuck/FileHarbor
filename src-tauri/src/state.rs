use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use organizer_core::{CancelToken, store::Store};

pub struct ScanEntry {
    pub cancel: CancelToken,
}

pub struct BackupEntry {
    pub cancel: CancelToken,
}

pub struct AppState {
    pub store: Mutex<Store>,
    pub scans: Mutex<HashMap<i64, ScanEntry>>,
    pub backups: Mutex<HashMap<i64, BackupEntry>>,
    pub quarantine_root: PathBuf,
    /// Kept for future phases (backup module, log paths, manifests).
    #[allow(dead_code)]
    pub app_data_dir: PathBuf,
}
