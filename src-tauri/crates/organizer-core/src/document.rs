//! Document near-duplicate detection and version lineage engine.
//!
//! Uses in-memory 64-bit Locality-Sensitive Hashing (SimHash) over word n-grams
//! to detect revisions, draft sprawl, and near-identical documents without
//! persisting private document contents to disk.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// The determined role of a document within a version lineage cluster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocumentRevisionRole {
    /// The newest and most complete version (recommended to keep).
    FinalVersion,
    /// An older, intermediate draft that can be safely archived or quarantined.
    ObsoleteDraft,
    /// A branch or parallel variant with divergent content.
    Variant,
}

/// Irreversible mathematical fingerprint of a document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentFingerprint {
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub modified_at_ms: Option<i64>,
    pub word_count: usize,
    /// 64-bit Locality-Sensitive SimHash.
    pub simhash: u64,
    pub format: String,
}

/// A member file in a document revision lineage cluster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentLineageMember {
    pub fingerprint: DocumentFingerprint,
    pub role: DocumentRevisionRole,
    pub similarity_to_final: f32,
}

/// A cluster of documents representing multiple revisions of the same work.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentLineageCluster {
    pub cluster_id: String,
    pub similarity_percentage: f32,
    pub primary_final_path: String,
    pub members: Vec<DocumentLineageMember>,
    pub explanation: String,
}

/// Computes Hamming distance between two 64-bit hashes.
#[inline]
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Converts Hamming distance (0..64) to similarity percentage (0.0% to 100.0%).
#[inline]
pub fn similarity_percentage(distance: u32) -> f32 {
    if distance >= 64 {
        0.0
    } else {
        ((64 - distance) as f32 / 64.0) * 100.0
    }
}

/// Simple 64-bit FNV-1a hash for string tokens.
fn fnv1a_64(token: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in token.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Computes a 64-bit SimHash from a stream of words/tokens, including 2-gram context.
pub fn compute_simhash<I, S>(tokens: I) -> (u64, usize)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut v = [0i32; 64];
    let mut count = 0;
    let mut prev_token = String::new();

    for token in tokens {
        let t = token.as_ref();
        if t.is_empty() {
            continue;
        }
        count += 1;

        // Individual word hash (weight 1)
        let h_single = fnv1a_64(t);
        for (i, val) in v.iter_mut().enumerate() {
            let bit = (h_single >> i) & 1;
            if bit == 1 {
                *val += 1;
            } else {
                *val -= 1;
            }
        }

        // Bi-gram hash with previous word (weight 2 for sequence continuity)
        if !prev_token.is_empty() {
            let bigram = format!("{}_{}", prev_token, t);
            let h_bi = fnv1a_64(&bigram);
            for (i, val) in v.iter_mut().enumerate() {
                let bit = (h_bi >> i) & 1;
                if bit == 1 {
                    *val += 2;
                } else {
                    *val -= 2;
                }
            }
        }
        prev_token = t.to_string();
    }

    let mut simhash: u64 = 0;
    for (i, &val) in v.iter().enumerate() {
        if val > 0 {
            simhash |= 1 << i;
        }
    }

    (simhash, count)
}

/// Extracts text words from a plain text or markup file in streaming mode.
pub fn extract_text_tokens(path: &Path) -> std::io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut tokens = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue, // ignore non-utf8 lines
        };

        for word in line.split_whitespace() {
            // Clean punctuation
            let cleaned: String = word
                .chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect();

            if cleaned.len() >= 2 {
                tokens.push(cleaned);
            }
        }
    }

    Ok(tokens)
}

/// Extracts words from XML/ZIP document formats (such as .docx or .odt)
/// by scanning XML text nodes between '>' and '<'.
pub fn extract_doc_archive_tokens(path: &Path) -> std::io::Result<Vec<String>> {
    let file = File::open(path)?;
    // Read up to 2MB of document streams for tokenization
    let mut buffer = Vec::new();
    file.take(2 * 1024 * 1024).read_to_end(&mut buffer)?;

    let mut tokens = Vec::new();
    let mut current_word = String::new();
    let mut inside_tag = false;

    for &b in &buffer {
        if b == b'<' {
            inside_tag = true;
            if !current_word.is_empty() {
                if current_word.len() >= 2 {
                    tokens.push(current_word.to_lowercase());
                }
                current_word.clear();
            }
        } else if b == b'>' {
            inside_tag = false;
        } else if !inside_tag {
            if b.is_ascii_alphanumeric() {
                current_word.push(b as char);
            } else if !current_word.is_empty() {
                if current_word.len() >= 2 {
                    tokens.push(current_word.to_lowercase());
                }
                current_word.clear();
            }
        }
    }

    if !current_word.is_empty() && current_word.len() >= 2 {
        tokens.push(current_word.to_lowercase());
    }

    Ok(tokens)
}

/// Computes a document fingerprint in memory.
pub fn compute_document_fingerprint(path: &Path) -> Option<DocumentFingerprint> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }

    let file_size = meta.len();
    if file_size == 0 || file_size > 50 * 1024 * 1024 {
        // Skip 0-byte or massive files (>50MB) for text extraction
        return None;
    }

    let modified_at_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);

    let file_name = path.file_name()?.to_string_lossy().to_string();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let (tokens, format_str) = match ext.as_str() {
        "txt" | "md" | "rtf" | "csv" | "log" | "json" => {
            let t = extract_text_tokens(path).ok()?;
            (t, ext.to_uppercase())
        }
        "docx" | "odt" => {
            let t = extract_doc_archive_tokens(path).ok()?;
            (t, ext.to_uppercase())
        }
        "pdf" => {
            // Lightweight text stream extraction
            let t = extract_doc_archive_tokens(path).ok()?;
            (t, "PDF".into())
        }
        _ => return None,
    };

    if tokens.len() < 5 {
        // Too few words to reliably compute semantic SimHash
        return None;
    }

    let (simhash, word_count) = compute_simhash(&tokens);

    Some(DocumentFingerprint {
        path: path.to_string_lossy().to_string(),
        file_name,
        file_size,
        modified_at_ms,
        word_count,
        simhash,
        format: format_str,
    })
}

/// Scans a folder recursively and clusters near-duplicate document revisions.
pub fn scan_and_cluster_document_lineages(
    folder: &Path,
    max_hamming_distance: u32,
) -> Result<Vec<DocumentLineageCluster>, String> {
    if !folder.exists() || !folder.is_dir() {
        return Err(format!("Folder does not exist: {}", folder.display()));
    }

    let mut fingerprints = Vec::new();
    let mut stack = vec![folder.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "node_modules" && !crate::privacy::is_sensitive_path(&p) {
                    stack.push(p);
                }
                continue;
            }

            if !p.is_file() || crate::privacy::is_sensitive_path(&p) {
                continue;
            }

            if let Some(fp) = compute_document_fingerprint(&p) {
                fingerprints.push(fp);
            }
        }
    }

    // Cluster near-duplicates
    let mut clusters: Vec<DocumentLineageCluster> = Vec::new();
    let mut visited = vec![false; fingerprints.len()];

    for i in 0..fingerprints.len() {
        if visited[i] {
            continue;
        }

        let mut current_members = vec![fingerprints[i].clone()];
        let mut min_dist_in_cluster = u32::MAX;

        for j in (i + 1)..fingerprints.len() {
            if visited[j] {
                continue;
            }

            let dist = hamming_distance(fingerprints[i].simhash, fingerprints[j].simhash);
            if dist <= max_hamming_distance {
                visited[j] = true;
                current_members.push(fingerprints[j].clone());
                if dist < min_dist_in_cluster {
                    min_dist_in_cluster = dist;
                }
            }
        }

        if current_members.len() > 1 {
            visited[i] = true;

            // Sort members to determine version lineage:
            // The newest file with high word count is the FinalVersion.
            current_members.sort_by(|a, b| {
                let mtime_a = a.modified_at_ms.unwrap_or(0);
                let mtime_b = b.modified_at_ms.unwrap_or(0);
                if mtime_a != mtime_b {
                    mtime_b.cmp(&mtime_a) // Newest first
                } else {
                    b.word_count.cmp(&a.word_count) // Most content first
                }
            });

            let final_doc = current_members[0].clone();
            let primary_final_path = final_doc.path.clone();

            let mut lineage_members = Vec::new();
            for (idx, m) in current_members.iter().enumerate() {
                let dist_to_final = hamming_distance(final_doc.simhash, m.simhash);
                let sim = similarity_percentage(dist_to_final);

                let role = if idx == 0 {
                    DocumentRevisionRole::FinalVersion
                } else if m.word_count <= final_doc.word_count {
                    DocumentRevisionRole::ObsoleteDraft
                } else {
                    DocumentRevisionRole::Variant
                };

                lineage_members.push(DocumentLineageMember {
                    fingerprint: m.clone(),
                    role,
                    similarity_to_final: sim,
                });
            }

            let cluster_similarity = similarity_percentage(min_dist_in_cluster);
            let obsolete_count = lineage_members
                .iter()
                .filter(|m| m.role == DocumentRevisionRole::ObsoleteDraft)
                .count();

            let explanation = format!(
                "Detected {} revision(s) of '{}'. '{}' is identified as the final version ({} older draft(s) available for archiving).",
                lineage_members.len(),
                final_doc.file_name,
                final_doc.file_name,
                obsolete_count
            );

            clusters.push(DocumentLineageCluster {
                cluster_id: format!("doc_{:016x}", final_doc.simhash),
                similarity_percentage: cluster_similarity,
                primary_final_path,
                members: lineage_members,
                explanation,
            });
        }
    }

    Ok(clusters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_simhash_identical_text() {
        let doc_a = ["this", "is", "a", "confidential", "project", "proposal", "for", "fileharbor"];
        let doc_b = ["this", "is", "a", "confidential", "project", "proposal", "for", "fileharbor"];

        let (hash_a, count_a) = compute_simhash(doc_a);
        let (hash_b, count_b) = compute_simhash(doc_b);

        assert_eq!(count_a, count_b);
        assert_eq!(hash_a, hash_b);
        assert_eq!(hamming_distance(hash_a, hash_b), 0);
        assert_eq!(similarity_percentage(0), 100.0);
    }

    #[test]
    fn test_simhash_near_duplicate_text() {
        let doc_a = vec![
            "the", "fileharbor", "backup", "system", "provides", "safe", "local", "snapshots",
            "and", "content", "addressed", "pools", "with", "guaranteed", "data", "integrity",
            "every", "file", "is", "hashed", "with", "sha256", "and", "stored", "safely",
            "the", "system", "never", "deletes", "any", "user", "file", "without", "confirmation",
            "quarantine", "stores", "everything", "securely", "on", "disk", "until", "reviewed",
        ];

        // doc_b has minor edits in a larger document
        let doc_b = vec![
            "the", "fileharbor", "backup", "engine", "provides", "safe", "local", "snapshots",
            "and", "immutable", "pools", "with", "guaranteed", "data", "integrity",
            "every", "file", "is", "hashed", "with", "sha256", "and", "stored", "reliably",
            "the", "system", "never", "deletes", "any", "user", "file", "without", "confirmation",
            "quarantine", "stores", "everything", "securely", "on", "disk", "until", "reviewed",
        ];

        let (hash_a, _) = compute_simhash(doc_a);
        let (hash_b, _) = compute_simhash(doc_b);

        let dist = hamming_distance(hash_a, hash_b);
        assert!(dist <= 12, "Expected close distance, got {dist}");
        assert!(similarity_percentage(dist) >= 80.0);
    }

    #[test]
    fn test_document_lineage_scan() {
        let dir = tempdir().unwrap();

        let doc1 = dir.path().join("report_v1.txt");
        let mut f1 = File::create(&doc1).unwrap();
        f1.write_all(b"Project status report for quarter three with complete milestones achieved on time and budget.\n").unwrap();
        drop(f1);

        let doc2 = dir.path().join("report_final.txt");
        let mut f2 = File::create(&doc2).unwrap();
        f2.write_all(b"Project status report for quarter three with complete milestones achieved on time and budget with extra testing.\n").unwrap();
        drop(f2);

        let clusters = scan_and_cluster_document_lineages(dir.path(), 12).unwrap();
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members.len(), 2);

        // One must be FinalVersion and one ObsoleteDraft
        let roles: Vec<_> = clusters[0].members.iter().map(|m| &m.role).collect();
        assert!(roles.contains(&&DocumentRevisionRole::FinalVersion));
        assert!(roles.contains(&&DocumentRevisionRole::ObsoleteDraft));
    }
}
