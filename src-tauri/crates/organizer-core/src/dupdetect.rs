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
            if m.copy_role == CopyRole::Backup {
                score += 1_000_000;
            }
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

/// Generates a human-readable explanation of why a group was formed and why a particular file was recommended to keep.
pub fn explain_duplicate_group(group: &DuplicateGroup) -> DuplicateExplanation {
    let keep_id = group.recommended_keep_file_id;
    let keep_member = group.members.iter().find(|m| m.id == keep_id);

    let keep_reason = match keep_member {
        Some(m) => {
            let mut reasons = Vec::new();
            let name_lower = m.file_name.to_lowercase();
            let has_markers = GENERATED_NAME_MARKERS.iter().any(|marker| name_lower.contains(marker));
            if !has_markers {
                reasons.push("cleanest filename without copy/duplicate suffixes".to_string());
            }
            if let Some(mod_time) = m.modified_at_ms {
                let is_oldest = group.members.iter().all(|other| other.modified_at_ms.unwrap_or(i64::MAX) >= mod_time);
                if is_oldest {
                    reasons.push("oldest modification timestamp (likely the original)".to_string());
                }
            }
            let depth = m.path.matches(['/', '\\']).count();
            let is_shallowest = group.members.iter().all(|other| other.path.matches(['/', '\\']).count() >= depth);
            if is_shallowest {
                reasons.push("shallowest directory depth (highest-level organized folder)".to_string());
            }

            if reasons.is_empty() {
                "selected based on deterministic directory structure and modification precedence".to_string()
            } else {
                format!("Recommended to keep because it has {}", reasons.join(", and "))
            }
        }
        None => "No specific keep recommendation could be established.".to_string(),
    };

    let member_details = group.members.iter().map(|m| {
        let name_lower = m.file_name.to_lowercase();
        let has_markers = GENERATED_NAME_MARKERS.iter().any(|marker| name_lower.contains(marker));
        let path_depth = m.path.matches(['/', '\\']).count();
        let role_protection_note = match m.copy_role {
            CopyRole::Backup => Some("Protected backup copy — safe from deletion".to_string()),
            CopyRole::Quarantine => Some("Currently in quarantine storage".to_string()),
            _ => None,
        };

        DuplicateMemberExplanation {
            file_id: m.id,
            file_name: m.file_name.clone(),
            path: m.path.clone(),
            modified_at_ms: m.modified_at_ms,
            copy_role: m.copy_role,
            is_recommended_keep: m.id == keep_id,
            has_duplicate_markers: has_markers,
            path_depth,
            role_protection_note,
        }
    }).collect();

    DuplicateExplanation {
        group_id: group.id,
        hash: group.hash.clone(),
        size_bytes: group.size_bytes,
        total_members: group.members.len(),
        match_level: "Level 1: Exact Binary Duplicate (SHA-256)".to_string(),
        confidence_percentage: 100.0,
        keep_recommendation_reason: keep_reason,
        member_details,
    }
}

