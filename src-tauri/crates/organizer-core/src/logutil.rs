//! Minimal structured logging.
//!
//! Writes one JSON line per event to stderr and to `<app_dir>/logs/app.log`.
//! Never logs file contents; paths only where necessary.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE: OnceLock<Option<Mutex<PathBuf>>> = OnceLock::new();

pub fn init_file_logging(app_data_dir: &std::path::Path) {
    let logs_dir = app_data_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    let path = logs_dir.join("app.log");
    let _ = LOG_FILE.set(Some(Mutex::new(path)));
}

pub fn log_event(level: &str, event: &str, fields: &[(&str, &str)]) {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut json = format!(
        "{{\"ts\":{ts_ms},\"level\":\"{level}\",\"event\":\"{event}\""
    );
    for (k, v) in fields {
        let escaped = serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into());
        json.push_str(&format!(",\"{k}\":{escaped}"));
    }
    json.push('}');

    eprintln!("{json}");

    if let Some(Some(mutex)) = LOG_FILE.get() {
        if let Ok(path) = mutex.lock() {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&*path) {
                let _ = writeln!(f, "{json}");
            }
        }
    }
}

pub fn info(event: &str, fields: &[(&str, &str)]) {
    log_event("info", event, fields);
}

pub fn warn(event: &str, fields: &[(&str, &str)]) {
    log_event("warn", event, fields);
}

pub fn error(event: &str, fields: &[(&str, &str)]) {
    log_event("error", event, fields);
}
