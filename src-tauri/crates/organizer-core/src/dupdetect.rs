//! Exact duplicate detection.
//!
//! Definition of an exact duplicate (Phase 1):
//!   identical content bytes, established by
//!   matching size + matching SHA-256 content hash.
//!
//! Filenames, extensions, timestamps and metadata are NEVER used as
//! identity. Empty files are excluded from duplicate groups; they are
//! presented separately because "empty" does not mean "useless".

use std::collections::HashMap;

use crate::models::*;
use crate::store::{DuplicateGroupInput, Store};

/// Heuristic filename penalty used ONLY for the keep recommendation.
const GENERATED_NAME_MARKERS: &[&str] = &[
    "copy", "duplicate", "kopie", "backup", "(1)", "(2)", "(3)", "(4)", "(5)",
    "- backup", "final", "final_", "_final", "~", "recovered_", "download",
];

/// Build and persist duplicate groups for a session based on stored
/// hash values. Files without a Computed hash are ignored.
pub fn build_groups(store: &Store, session_id: i64) -> Result<(), String> {
    let sizes = store.candidate_sizes(session_id)?;
    let candidates = store.files_with_sizes(session_id, &sizes)?;

    // Group by (size, hash): both must match. Size is verified again here
    // as defense-in-depth alongside the SHA-256 identity check.
    let mut buckets: HashMap<(u64, String), Vec<FileRecord>> = HashMap::new();
    for f in candidates {
        if f.is_empty {
            continue;
        }
        if f.scan_status != ScanStatus::Completed || f.hash_status != HashStatus::Computed {
            continue;
        }
        let Some(hash) = &f.hash else { continue };
        buckets.entry((f.size_bytes, hash.clone())).or_default().push(f);
    }

    let mut inputs: Vec<DuplicateGroupInput> = Vec::new();
    let mut grouped: Vec<(_, Vec<FileRecord>)> = buckets.into_iter().filter(|(_, v)| v.len() > 1).collect();
    // Largest groups first for stable ordering.
    grouped.sort_by_key(|(k, _)| std::cmp::Reverse(k.0));
    for ((size, hash), members) in grouped {
        let mut members = members;
        members.sort_by(|a, b| a.path.cmp(&b.path));
        let recommended_keep_file_id = recommend_keep(&members);
        inputs.push(DuplicateGroupInput {
            hash,
            size_bytes: size,
            member_file_ids: members.iter().map(|m| m.id).collect(),
            recommended_keep_file_id,
        });
    }
    store.replace_duplicate_groups(session_id, &inputs)
}

/// Recommendation only — never an automatic decision.
///
/// Scoring prefers:
/// 1. Original-looking filenames (penalizes "copy", "(1)", etc.)
/// 2. Shallower directory depth (closer to user-organized locations)
/// 3. Oldest modification date (most likely the original)
pub fn recommend_keep(members: &[FileRecord]) -> i64 {
    // Primary key: heuristic score (lower is better).
    // Tiebreakers: oldest modification date first, then path.
    let mut ranked: Vec<(u64, i64, &str, i64)> = members
        .iter()
        .map(|m| {
            let name_lower = m.file_name.to_lowercase();
            let mut score: u64 = 0;
            if GENERATED_NAME_MARKERS.iter().any(|marker| name_lower.contains(marker)) {
                score += 100;
            }
            score += m.path.matches(['/', '\\']).count() as u64;
            if m.scan_status != ScanStatus::Completed {
                score += 10_000;
            }
            if m.modified_at_ms.is_none() {
                score += 50;
            }
            (score, -(m.modified_at_ms.unwrap_or(0)), m.path.as_str(), m.id)
        })
        .collect();
    ranked.sort();
    ranked.first().map(|r| r.3).unwrap_or(0)
}

