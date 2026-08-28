//! Batch operation ids + small helpers shared by services.

use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a sortable, collision-resistant batch id such as
/// `quarantine-20260826-183200-a1b2c3`.
pub fn new_batch_id(kind: &str) -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let millis = d.subsec_millis();
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    // Simple date math from epoch days (valid through year 2099).
    let (year, month, day) = civil_from_days(days as i64);
    let rand: u32 = ((secs as u32) ^ (millis << 8)) & 0xFFFFFF;
    format!("{kind}-{year:04}{month:02}{day:02}-{h:02}{m:02}{s:02}-{rand:06x}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
