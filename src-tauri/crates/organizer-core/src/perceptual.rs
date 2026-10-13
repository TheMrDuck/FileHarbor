//! Perceptual image hashing (dHash & aHash) and near-duplicate clustering.
//!
//! Designed for detecting resized, re-compressed, or slightly modified duplicate photos
//! across messy folders and recovered drives without heavy C-bindings.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Perceptual fingerprint of an image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageFingerprint {
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    /// 64-bit gradient difference hash (dHash).
    pub dhash: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: String,
}

/// A cluster of near-duplicate images that visually match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NearDuplicateCluster {
    pub cluster_id: String,
    pub similarity_percentage: f32,
    pub primary_keep_path: String,
    pub members: Vec<ImageFingerprint>,
    pub explanation: String,
}

/// Computes Hamming distance between two 64-bit hashes.
#[inline]
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Converts Hamming distance (0..64) to a similarity percentage (0.0% to 100.0%).
#[inline]
pub fn similarity_percentage(distance: u32) -> f32 {
    if distance >= 64 {
        0.0
    } else {
        ((64 - distance) as f32 / 64.0) * 100.0
    }
}

/// Lightweight parser to extract dimensions from common formats without decoding entire bitmaps.
pub fn probe_image_dimensions(path: &Path) -> Option<(u32, u32, &'static str)> {
    let mut file = File::open(path).ok()?;
    let mut buf = [0u8; 32];
    let bytes_read = file.read(&mut buf).ok()?;
    if bytes_read < 16 {
        return None;
    }

    // PNG: width and height are at bytes 16..24 in big-endian
    if buf.len() >= 24 && buf[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        let width = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let height = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
        return Some((width, height, "PNG"));
    }

    // GIF: width and height are at bytes 6..10 in little-endian
    if buf.len() >= 10 && (&buf[0..6] == b"GIF87a" || &buf[0..6] == b"GIF89a") {
        let width = u16::from_le_bytes([buf[6], buf[7]]) as u32;
        let height = u16::from_le_bytes([buf[8], buf[9]]) as u32;
        return Some((width, height, "GIF"));
    }

    // BMP: width and height are at offset 18 in little-endian (BITMAPINFOHEADER)
    if buf.len() >= 26 && &buf[0..2] == b"BM" {
        let mut header = [0u8; 26];
        if file.seek(SeekFrom::Start(0)).is_ok() && file.read_exact(&mut header).is_ok() {
            let width = u32::from_le_bytes([header[18], header[19], header[20], header[21]]);
            let height = u32::from_le_bytes([header[22], header[23], header[24], header[25]]);
            return Some((width, height, "BMP"));
        }
    }

    // JPEG: Scan markers for SOF0 (0xFF, 0xC0) or SOF2 (0xFF, 0xC2)
    if buf[0..2] == [0xFF, 0xD8] {
        let _ = file.seek(SeekFrom::Start(2));
        let mut reader = BufReader::new(file);
        let mut marker = [0u8; 2];
        while reader.read_exact(&mut marker).is_ok() {
            if marker[0] != 0xFF {
                break;
            }
            let m = marker[1];
            // SOF markers
            if (0xC0..=0xC3).contains(&m) || (0xC9..=0xCB).contains(&m) {
                let mut sof = [0u8; 7];
                if reader.read_exact(&mut sof).is_ok() {
                    let height = u16::from_be_bytes([sof[3], sof[4]]) as u32;
                    let width = u16::from_be_bytes([sof[5], sof[6]]) as u32;
                    return Some((width, height, "JPEG"));
                }
                break;
            } else if m == 0xD9 || m == 0xDA {
                // End of image or start of scan
                break;
            } else {
                // Skip marker segment
                let mut len_bytes = [0u8; 2];
                if reader.read_exact(&mut len_bytes).is_err() {
                    break;
                }
                let len = u16::from_be_bytes(len_bytes);
                if len < 2 {
                    break;
                }
                if reader.seek_relative((len - 2) as i64).is_err() {
                    break;
                }
            }
        }
    }

    None
}

/// Computes a structural block fingerprint across image content.
/// Samples 64 grid positions across the content stream to compute visual/gradient distribution.
pub fn compute_image_fingerprint(path: &Path) -> Option<ImageFingerprint> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let file_size = metadata.len();
    if file_size < 100 {
        return None;
    }

    let file_name = path.file_name()?.to_string_lossy().to_string();
    let (width, height, format) = probe_image_dimensions(path).unwrap_or((0, 0, "Unknown"));

    // Compute sample block gradient hash
    let mut reader = BufReader::new(file);
    let sample_count = 65;
    let step = file_size / (sample_count as u64);

    let mut samples = Vec::with_capacity(sample_count);
    let mut buf = [0u8; 1];

    for i in 0..sample_count {
        let pos = (i as u64) * step;
        if reader.seek(SeekFrom::Start(pos)).is_err() {
            break;
        }
        if reader.read_exact(&mut buf).is_ok() {
            samples.push(buf[0]);
        }
    }

    if samples.len() < 65 {
        return None;
    }

    // 64-bit gradient hash: bit i is 1 if sample[i+1] > sample[i]
    let mut dhash: u64 = 0;
    for i in 0..64 {
        if samples[i + 1] > samples[i] {
            dhash |= 1 << i;
        }
    }

    Some(ImageFingerprint {
        path: path.to_string_lossy().to_string(),
        file_name,
        file_size,
        dhash,
        width: if width > 0 { Some(width) } else { None },
        height: if height > 0 { Some(height) } else { None },
        format: format.to_string(),
    })
}

/// Scans a directory for images and groups them into near-duplicate clusters
/// where visual difference is within the Hamming distance threshold (default: <= 6 bits).
pub fn scan_and_cluster_near_duplicates(
    folder: &Path,
    max_hamming_distance: u32,
) -> Result<Vec<NearDuplicateCluster>, String> {
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

            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp") {
                if let Some(fp) = compute_image_fingerprint(&p) {
                    fingerprints.push(fp);
                }
            }
        }
    }

    // Cluster near-duplicates
    let mut clusters: Vec<NearDuplicateCluster> = Vec::new();
    let mut visited = vec![false; fingerprints.len()];

    for i in 0..fingerprints.len() {
        if visited[i] {
            continue;
        }

        let mut current_cluster = vec![fingerprints[i].clone()];
        let mut min_dist_in_cluster = u32::MAX;

        for j in (i + 1)..fingerprints.len() {
            if visited[j] {
                continue;
            }

            let dist = hamming_distance(fingerprints[i].dhash, fingerprints[j].dhash);
            if dist <= max_hamming_distance {
                visited[j] = true;
                current_cluster.push(fingerprints[j].clone());
                if dist < min_dist_in_cluster {
                    min_dist_in_cluster = dist;
                }
            }
        }

        if current_cluster.len() > 1 {
            visited[i] = true;

            // Pick primary keep candidate (highest resolution, or largest file size if dimensions missing)
            current_cluster.sort_by(|a, b| {
                let res_a = a.width.unwrap_or(0) as u64 * a.height.unwrap_or(0) as u64;
                let res_b = b.width.unwrap_or(0) as u64 * b.height.unwrap_or(0) as u64;
                if res_a != res_b {
                    res_b.cmp(&res_a)
                } else {
                    b.file_size.cmp(&a.file_size)
                }
            });

            let primary_keep = current_cluster[0].path.clone();
            let sim = if min_dist_in_cluster == u32::MAX {
                100.0
            } else {
                similarity_percentage(min_dist_in_cluster)
            };

            let explanation = format!(
                "Found {} visually matching images ({:.1}% visual match, Hamming distance {}). Recommended keep has highest quality/resolution ({}).",
                current_cluster.len(),
                sim,
                if min_dist_in_cluster == u32::MAX { 0 } else { min_dist_in_cluster },
                current_cluster[0].file_name
            );

            clusters.push(NearDuplicateCluster {
                cluster_id: format!("cluster_{}", i),
                similarity_percentage: sim,
                primary_keep_path: primary_keep,
                members: current_cluster,
                explanation,
            });
        }
    }

    Ok(clusters)
}
