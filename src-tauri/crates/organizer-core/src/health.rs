use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

/// Status of header / magic bytes verification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MagicByteStatus {
    /// Valid known magic bytes matching file extension.
    Valid { detected_format: String },
    /// Magic bytes exist but do not match the file extension (potential masquerade or corruption).
    Mismatch { detected_format: String, expected_format: String },
    /// File header is corrupted or unreadable.
    CorruptedHeader { details: String },
    /// File format has no standard static magic byte signature (plain text, raw data, etc.).
    NotApplicable,
}

/// Statistics on null byte runs (typical of bad sector holes on recovered drives).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NullRunStats {
    /// Total bytes examined.
    pub total_bytes_analyzed: u64,
    /// Total count of 0x00 null bytes found.
    pub total_null_bytes: u64,
    /// Percentage of the file that consists of null bytes (0.0 to 100.0).
    pub null_percentage: f64,
    /// Size in bytes of the longest continuous run of 0x00 bytes.
    pub longest_null_run_bytes: u64,
    /// Count of discrete null runs larger than standard sector size (512 bytes).
    pub sector_sized_null_blocks: u32,
}

/// Overall health evaluation score and assessment for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileHealthReport {
    pub file_path: PathBuf,
    pub file_name: String,
    pub file_size: u64,
    pub sha256: String,
    pub magic_status: MagicByteStatus,
    pub null_stats: NullRunStats,
    /// Shannon entropy value (0.0 = completely homogeneous, 8.0 = maximum randomness/compression).
    pub entropy: f64,
    /// Calculated health score from 0 (heavily corrupted / hole-filled) to 100 (intact / healthy).
    pub health_score: u8,
    pub issues: Vec<String>,
}

/// Recommendation on comparing two recovered candidate copies of a file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthRecommendation {
    KeepA,
    KeepB,
    BothHealthy,
    BothCorrupted,
    Inconclusive,
}

/// Side-by-side comparison between two recovered copies of a file with conflicting content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecoveryPairComparison {
    pub pair_id: String,
    pub base_name: String,
    pub candidate_a: FileHealthReport,
    pub candidate_b: FileHealthReport,
    pub recommendation: HealthRecommendation,
    pub explanation: String,
}

/// Detects known magic bytes for common formats.
pub fn check_magic_bytes(path: &Path, header: &[u8]) -> MagicByteStatus {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if header.is_empty() {
        return MagicByteStatus::CorruptedHeader {
            details: "File is 0 bytes or header could not be read".into(),
        };
    }

    // JPEG: FF D8 FF
    if header.len() >= 3 && header[0..3] == [0xFF, 0xD8, 0xFF] {
        return if ext == "jpg" || ext == "jpeg" {
            MagicByteStatus::Valid {
                detected_format: "JPEG".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "JPEG".into(),
                expected_format: ext,
            }
        };
    }

    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if header.len() >= 8 && header[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return if ext == "png" {
            MagicByteStatus::Valid {
                detected_format: "PNG".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "PNG".into(),
                expected_format: ext,
            }
        };
    }

    // GIF: GIF87a or GIF89a
    if header.len() >= 6 && (&header[0..6] == b"GIF87a" || &header[0..6] == b"GIF89a") {
        return if ext == "gif" {
            MagicByteStatus::Valid {
                detected_format: "GIF".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "GIF".into(),
                expected_format: ext,
            }
        };
    }

    // PDF: %PDF
    if header.len() >= 4 && &header[0..4] == b"%PDF" {
        return if ext == "pdf" {
            MagicByteStatus::Valid {
                detected_format: "PDF".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "PDF".into(),
                expected_format: ext,
            }
        };
    }

    // ZIP / Office Open XML (DOCX, XLSX, PPTX): PK\x03\x04
    if header.len() >= 4 && header[0..4] == [0x50, 0x4B, 0x03, 0x04] {
        let is_zip_or_office = matches!(
            ext.as_str(),
            "zip" | "docx" | "xlsx" | "pptx" | "jar" | "apk"
        );
        return if is_zip_or_office {
            MagicByteStatus::Valid {
                detected_format: "ZIP/OfficeArchive".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "ZIP/OfficeArchive".into(),
                expected_format: ext,
            }
        };
    }

    // 7z: 37 7A BC AF 27 1C
    if header.len() >= 6 && header[0..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return if ext == "7z" {
            MagicByteStatus::Valid {
                detected_format: "7-Zip".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "7-Zip".into(),
                expected_format: ext,
            }
        };
    }

    // MP4 / MOV: check for ftyp box at offset 4
    if header.len() >= 8 && &header[4..8] == b"ftyp" {
        let is_mp4_mov = matches!(ext.as_str(), "mp4" | "m4v" | "mov");
        return if is_mp4_mov {
            MagicByteStatus::Valid {
                detected_format: "MP4/ISO Media".into(),
            }
        } else {
            MagicByteStatus::Mismatch {
                detected_format: "MP4/ISO Media".into(),
                expected_format: ext,
            }
        };
    }

    // If extension expects one of the above known types, but bytes were completely zeroed or wrong
    if matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "pdf" | "zip" | "docx" | "xlsx" | "pptx" | "7z" | "mp4"
    ) {
        if header.iter().take(8).all(|&b| b == 0x00) {
            return MagicByteStatus::CorruptedHeader {
                details: "Header completely zeroed out (likely unreadable sector hole)".into(),
            };
        }
        return MagicByteStatus::CorruptedHeader {
            details: format!(
                "Header magic bytes {:02X?} do not match expected format for .{}",
                &header[..header.len().min(8)],
                ext
            ),
        };
    }

    MagicByteStatus::NotApplicable
}

/// Computes byte frequency distribution, null-run metrics, and Shannon entropy in a single streaming pass.
pub fn analyze_file_stream(
    file_path: &Path,
) -> std::io::Result<(NullRunStats, f64, MagicByteStatus, String)> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let mut first_chunk = [0u8; 32];
    let header_bytes_read = reader.read(&mut first_chunk)?;
    let header = &first_chunk[..header_bytes_read];
    let magic_status = check_magic_bytes(file_path, header);

    // Reopen for full streaming SHA-256 and byte analysis
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();

    let mut byte_counts = [0u64; 256];
    let mut total_bytes: u64 = 0;
    let mut total_null_bytes: u64 = 0;
    let mut current_null_run: u64 = 0;
    let mut longest_null_run: u64 = 0;
    let mut sector_sized_null_blocks: u32 = 0;

    let mut buffer = [0u8; 64 * 1024];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        hasher.update(&buffer[..n]);
        total_bytes += n as u64;

        for &byte in &buffer[..n] {
            byte_counts[byte as usize] += 1;
            if byte == 0x00 {
                total_null_bytes += 1;
                current_null_run += 1;
                if current_null_run > longest_null_run {
                    longest_null_run = current_null_run;
                }
                if current_null_run == 512 {
                    sector_sized_null_blocks += 1;
                }
            } else {
                current_null_run = 0;
            }
        }
    }

    let sha256 = format!("{:x}", hasher.finalize());

    // Shannon Entropy: H = - sum( p(x) * log2(p(x)) )
    let entropy = if total_bytes == 0 {
        0.0
    } else {
        let mut h = 0.0f64;
        let total_f = total_bytes as f64;
        for &count in &byte_counts {
            if count > 0 {
                let p = count as f64 / total_f;
                h -= p * p.log2();
            }
        }
        h
    };

    let null_percentage = if total_bytes == 0 {
        0.0
    } else {
        (total_null_bytes as f64 / total_bytes as f64) * 100.0
    };

    let null_stats = NullRunStats {
        total_bytes_analyzed: total_bytes,
        total_null_bytes,
        null_percentage,
        longest_null_run_bytes: longest_null_run,
        sector_sized_null_blocks,
    };

    Ok((null_stats, entropy, magic_status, sha256))
}

/// Evaluates file health and calculates a score between 0 and 100.
pub fn evaluate_file_health(file_path: &Path) -> std::io::Result<FileHealthReport> {
    let metadata = std::fs::metadata(file_path)?;
    let file_size = metadata.len();
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    if file_size == 0 {
        return Ok(FileHealthReport {
            file_path: file_path.to_path_buf(),
            file_name,
            file_size: 0,
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            magic_status: MagicByteStatus::CorruptedHeader {
                details: "Empty 0-byte file".into(),
            },
            null_stats: NullRunStats {
                total_bytes_analyzed: 0,
                total_null_bytes: 0,
                null_percentage: 0.0,
                longest_null_run_bytes: 0,
                sector_sized_null_blocks: 0,
            },
            entropy: 0.0,
            health_score: 0,
            issues: vec!["File is empty (0 bytes)".into()],
        });
    }

    let (null_stats, entropy, magic_status, sha256) = analyze_file_stream(file_path)?;

    let mut issues = Vec::new();
    let mut score: i32 = 100;

    // Header checks
    match &magic_status {
        MagicByteStatus::Valid { .. } => {}
        MagicByteStatus::Mismatch {
            detected_format,
            expected_format,
        } => {
            score -= 30;
            issues.push(format!(
                "File header indicates '{}' but file extension is '.{}'",
                detected_format, expected_format
            ));
        }
        MagicByteStatus::CorruptedHeader { details } => {
            score -= 50;
            issues.push(format!("Corrupted header: {}", details));
        }
        MagicByteStatus::NotApplicable => {}
    }

    // Null runs / sector hole checks
    if null_stats.sector_sized_null_blocks > 0 {
        let penalty = (null_stats.sector_sized_null_blocks as i32 * 5).min(40);
        score -= penalty;
        issues.push(format!(
            "Contains {} disk-sector sized (>=512B) null byte blocks (likely bad sector holes)",
            null_stats.sector_sized_null_blocks
        ));
    }

    if null_stats.null_percentage > 25.0 {
        score -= 20;
        issues.push(format!(
            "{:.1}% of the file content is empty zero bytes",
            null_stats.null_percentage
        ));
    }

    // Entropy check for compressed media formats
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let is_compressed_media =
        matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "mp4" | "zip" | "7z");

    if is_compressed_media && entropy < 5.0 && file_size > 1024 {
        score -= 25;
        issues.push(format!(
            "Abnormally low entropy ({:.2} / 8.0) for a compressed format (possible zero-fill or corrupt data)",
            entropy
        ));
    }

    let final_score = score.clamp(0, 100) as u8;

    Ok(FileHealthReport {
        file_path: file_path.to_path_buf(),
        file_name,
        file_size,
        sha256,
        magic_status,
        null_stats,
        entropy,
        health_score: final_score,
        issues,
    })
}

/// Compares two recovered file candidates (e.g. from an HDD/SSD recovery scan)
/// that have the same or similar name and size, but different SHA-256 hashes due to corruption.
pub fn compare_recovery_pair(
    candidate_a: FileHealthReport,
    candidate_b: FileHealthReport,
) -> RecoveryPairComparison {
    let pair_id = format!("{}_{}", &candidate_a.sha256[..8], &candidate_b.sha256[..8]);
    let base_name = candidate_a.file_name.clone();

    let score_diff = candidate_a.health_score as i32 - candidate_b.health_score as i32;

    let (recommendation, explanation) = if score_diff > 15 {
        (
            HealthRecommendation::KeepA,
            format!(
                "Candidate A ({}) is significantly healthier (Score: {} vs {}). Candidate B contains headers or sector damage.",
                candidate_a.file_path.display(),
                candidate_a.health_score,
                candidate_b.health_score
            ),
        )
    } else if score_diff < -15 {
        (
            HealthRecommendation::KeepB,
            format!(
                "Candidate B ({}) is significantly healthier (Score: {} vs {}). Candidate A contains headers or sector damage.",
                candidate_b.file_path.display(),
                candidate_b.health_score,
                candidate_a.health_score
            ),
        )
    } else if candidate_a.health_score >= 80 && candidate_b.health_score >= 80 {
        (
            HealthRecommendation::BothHealthy,
            "Both candidate copies appear intact and free of sector holes. Manual review recommended.".into(),
        )
    } else if candidate_a.health_score < 40 && candidate_b.health_score < 40 {
        (
            HealthRecommendation::BothCorrupted,
            "Both candidate copies exhibit severe header or sector damage.".into(),
        )
    } else {
        (
            HealthRecommendation::Inconclusive,
            format!(
                "Health scores are close (A: {}, B: {}). Further inspection or media playback required.",
                candidate_a.health_score, candidate_b.health_score
            ),
        )
    };

    RecoveryPairComparison {
        pair_id,
        base_name,
        candidate_a,
        candidate_b,
        recommendation,
        explanation,
    }
}

/// Identifies conflicting recovery file candidates across a set of inspected files.
/// Matches files with identical filename or same stem + extension and similar size (+/- 10%),
/// but distinct SHA-256 hashes.
pub fn find_recovery_pairs(reports: Vec<FileHealthReport>) -> Vec<RecoveryPairComparison> {
    let mut pairs = Vec::new();
    let n = reports.len();

    for i in 0..n {
        for j in (i + 1)..n {
            let a = &reports[i];
            let b = &reports[j];

            // If hashes match, they are exact duplicates, not corrupted collision pairs
            if a.sha256 == b.sha256 {
                continue;
            }

            let same_name = a.file_name.to_lowercase() == b.file_name.to_lowercase();
            let size_diff = (a.file_size as i64 - b.file_size as i64).abs();
            let max_size = a.file_size.max(b.file_size);

            let similar_size = if max_size > 0 {
                (size_diff as f64 / max_size as f64) <= 0.10 // within 10%
            } else {
                true
            };

            if same_name || (similar_size && has_same_stem_or_ext(&a.file_name, &b.file_name)) {
                pairs.push(compare_recovery_pair(a.clone(), b.clone()));
            }
        }
    }

    pairs
}

fn has_same_stem_or_ext(name_a: &str, name_b: &str) -> bool {
    let path_a = Path::new(name_a);
    let path_b = Path::new(name_b);

    let ext_a = path_a.extension().and_then(|s| s.to_str()).unwrap_or("");
    let ext_b = path_b.extension().and_then(|s| s.to_str()).unwrap_or("");

    if ext_a.is_empty() || ext_b.is_empty() || ext_a.to_lowercase() != ext_b.to_lowercase() {
        return false;
    }

    let stem_a = path_a.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let stem_b = path_b.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    // Detect recovery suffixes like file_1.jpg vs file.jpg or file_recovered.jpg vs file.jpg
    stem_a.starts_with(stem_b) || stem_b.starts_with(stem_a)
}

/// A proposal to rectify a misidentified or missing file extension based on magic byte detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionRectificationProposal {
    pub current_path: String,
    pub file_name: String,
    pub current_extension: String,
    pub detected_format: String,
    pub suggested_extension: String,
    pub suggested_new_path: String,
    pub confidence_percentage: f32,
    pub reason: String,
}

/// Helper mapping detected format names to standard file extensions.
pub fn standard_extension_for_format(format_name: &str) -> Option<&'static str> {
    match format_name {
        "JPEG" => Some("jpg"),
        "PNG" => Some("png"),
        "GIF" => Some("gif"),
        "PDF" => Some("pdf"),
        "7-Zip" => Some("7z"),
        "MP4/ISO Media" => Some("mp4"),
        "ZIP/OfficeArchive" => Some("zip"),
        _ => None,
    }
}

/// Scans a directory recursively looking for files whose actual content/magic-bytes
/// disagree with their current extension (or have generic extensions like .bin, .tmp, or no extension).
pub fn scan_for_extension_rectifications(root: &Path) -> Result<Vec<ExtensionRectificationProposal>, String> {
    if !root.exists() || !root.is_dir() {
        return Err(format!("Directory does not exist: {}", root.display()));
    }

    let mut proposals = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "node_modules" && !crate::privacy::is_sensitive_path(&path) {
                    stack.push(path);
                }
                continue;
            }

            if !path.is_file() || crate::privacy::is_sensitive_path(&path) {
                continue;
            }

            let file_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            // Read first 32 bytes for magic bytes
            let mut file = match File::open(&path) {
                Ok(f) => f,
                Err(_) => continue,
            };

            let mut header = [0u8; 32];
            let bytes_read = match file.read(&mut header) {
                Ok(b) if b > 0 => b,
                _ => continue,
            };

            let magic_status = check_magic_bytes(&path, &header[..bytes_read]);

            if let MagicByteStatus::Mismatch { detected_format, expected_format } = magic_status {
                if let Some(target_ext) = standard_extension_for_format(&detected_format) {
                    // If the file is already using an alias (e.g. .jpeg vs .jpg), ignore
                    let cur_ext = expected_format.to_lowercase();
                    if (cur_ext == "jpeg" && target_ext == "jpg") || (cur_ext == "jpg" && target_ext == "jpeg") {
                        continue;
                    }

                    let current_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&file_name);
                    let new_file_name = format!("{}.{}", current_stem, target_ext);
                    let parent_dir = path.parent().unwrap_or(root);
                    let suggested_path = parent_dir.join(&new_file_name);

                    proposals.push(ExtensionRectificationProposal {
                        current_path: path.to_string_lossy().to_string(),
                        file_name: file_name.clone(),
                        current_extension: cur_ext.clone(),
                        detected_format: detected_format.clone(),
                        suggested_extension: target_ext.to_string(),
                        suggested_new_path: suggested_path.to_string_lossy().to_string(),
                        confidence_percentage: 99.5,
                        reason: format!(
                            "File signature matches {} but extension is '{}'. Recommended extension: .{}",
                            detected_format, cur_ext, target_ext
                        ),
                    });
                }
            }
        }
    }

    Ok(proposals)
}

/// Result of an individual extension rectification attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RectificationResultItem {
    pub original_path: String,
    pub rectified_path: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Safely applies extension rectifications, ensuring collision-free renaming.
pub fn execute_extension_rectifications(
    items_to_rename: Vec<(String, String)>,
) -> Result<Vec<RectificationResultItem>, String> {
    let mut results = Vec::new();

    for (current_path_str, suggested_path_str) in items_to_rename {
        let current_path = PathBuf::from(&current_path_str);
        let target_path = PathBuf::from(&suggested_path_str);

        if !current_path.exists() {
            results.push(RectificationResultItem {
                original_path: current_path_str,
                rectified_path: suggested_path_str,
                success: false,
                error: Some("Source file no longer exists".into()),
            });
            continue;
        }

        let final_target = crate::triage::resolve_collision_path(&target_path);

        match std::fs::rename(&current_path, &final_target) {
            Ok(_) => {
                results.push(RectificationResultItem {
                    original_path: current_path_str,
                    rectified_path: final_target.to_string_lossy().to_string(),
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                results.push(RectificationResultItem {
                    original_path: current_path_str,
                    rectified_path: suggested_path_str,
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok(results)
}
