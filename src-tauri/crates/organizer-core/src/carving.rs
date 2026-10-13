//! Deep carving & autonomous metadata-driven file renaming.
//!
//! Inspects recovery files, dumps, and orphaned binaries (.bin, .dat, .tmp, generic names),
//! identifies true format signatures from magic bytes, extracts internal metadata
//! (EXIF DateTimeOriginal, Camera Model, ID3v2 tags, PDF creation dates/titles),
//! and proposes structured, collision-safe canonical file names.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CarvedFileProposal {
    pub source_path: String,
    pub original_name: String,
    pub detected_mime: String,
    pub detected_extension: String,
    pub extracted_date: Option<String>,
    pub extracted_camera_or_artist: Option<String>,
    pub extracted_title: Option<String>,
    pub proposed_name: String,
    pub proposed_path: String,
    pub confidence_score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarveExecutionResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct CarvedMetadata {
    pub mime: String,
    pub extension: String,
    pub date: Option<String>,
    pub camera_or_artist: Option<String>,
    pub title: Option<String>,
    pub confidence: u8,
}

/// Sniff file content buffer (up to 64KB) and return identified format and metadata.
pub fn sniff_metadata(buf: &[u8]) -> Option<CarvedMetadata> {
    if buf.len() < 8 {
        return None;
    }

    // 1. JPEG
    if buf.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let (date, model) = parse_jpeg_exif(buf);
        return Some(CarvedMetadata {
            mime: "image/jpeg".to_string(),
            extension: "jpg".to_string(),
            date,
            camera_or_artist: model,
            title: None,
            confidence: 95,
        });
    }

    // 2. PNG
    if buf.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(CarvedMetadata {
            mime: "image/png".to_string(),
            extension: "png".to_string(),
            date: None,
            camera_or_artist: None,
            title: None,
            confidence: 95,
        });
    }

    // 3. GIF
    if buf.starts_with(b"GIF87a") || buf.starts_with(b"GIF89a") {
        return Some(CarvedMetadata {
            mime: "image/gif".to_string(),
            extension: "gif".to_string(),
            date: None,
            camera_or_artist: None,
            title: None,
            confidence: 90,
        });
    }

    // 4. WebP: "RIFF" .... "WEBP"
    if buf.starts_with(b"RIFF") && buf.len() >= 12 && &buf[8..12] == b"WEBP" {
        return Some(CarvedMetadata {
            mime: "image/webp".to_string(),
            extension: "webp".to_string(),
            date: None,
            camera_or_artist: None,
            title: None,
            confidence: 90,
        });
    }

    // 5. MP3 / ID3v2
    if buf.starts_with(b"ID3") {
        let (artist, title, year) = parse_id3v2(buf);
        return Some(CarvedMetadata {
            mime: "audio/mpeg".to_string(),
            extension: "mp3".to_string(),
            date: year,
            camera_or_artist: artist,
            title,
            confidence: 95,
        });
    }

    // 6. MP4 / MOV: ....ftyp
    if buf.len() >= 12 && &buf[4..8] == b"ftyp" {
        return Some(CarvedMetadata {
            mime: "video/mp4".to_string(),
            extension: "mp4".to_string(),
            date: None,
            camera_or_artist: None,
            title: None,
            confidence: 90,
        });
    }

    // 7. PDF: %PDF-
    if buf.starts_with(b"%PDF-") {
        let (date, title) = parse_pdf_metadata(buf);
        return Some(CarvedMetadata {
            mime: "application/pdf".to_string(),
            extension: "pdf".to_string(),
            date,
            camera_or_artist: None,
            title,
            confidence: 95,
        });
    }

    // 8. ZIP / Office formats: PK\x03\x04
    if buf.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        let (ext, mime) = if contains_subslice(buf, b"word/") {
            ("docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        } else if contains_subslice(buf, b"xl/") {
            ("xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        } else if contains_subslice(buf, b"ppt/") {
            ("pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation")
        } else {
            ("zip", "application/zip")
        };

        return Some(CarvedMetadata {
            mime: mime.to_string(),
            extension: ext.to_string(),
            date: None,
            camera_or_artist: None,
            title: None,
            confidence: 85,
        });
    }

    // 9. SQLite format 3
    if buf.starts_with(b"SQLite format 3\0") {
        return Some(CarvedMetadata {
            mime: "application/vnd.sqlite3".to_string(),
            extension: "sqlite".to_string(),
            date: None,
            camera_or_artist: None,
            title: None,
            confidence: 95,
        });
    }

    None
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ----------------------------------------------------------------------
// EXIF Parser for JPEG (Pure Rust, Little & Big Endian)
// ----------------------------------------------------------------------

pub fn parse_jpeg_exif(buf: &[u8]) -> (Option<String>, Option<String>) {
    let mut idx = 2; // skip SOI (0xFF, 0xD8)
    while idx + 4 < buf.len() {
        if buf[idx] != 0xFF {
            idx += 1;
            continue;
        }
        let marker = buf[idx + 1];
        if marker == 0xDA || marker == 0xD9 {
            // SOS (Start of Scan) or EOI (End of Image)
            break;
        }

        let length = ((buf[idx + 2] as usize) << 8) | (buf[idx + 3] as usize);
        if idx + 2 + length > buf.len() {
            break;
        }

        // APP1 marker (0xE1) with Exif header
        if marker == 0xE1 && idx + 10 <= buf.len() && &buf[idx + 4..idx + 10] == b"Exif\0\0" {
            let tiff_data = &buf[idx + 10..idx + 2 + length];
            return parse_tiff_exif(tiff_data);
        }

        idx += 2 + length;
    }
    (None, None)
}

fn parse_tiff_exif(tiff: &[u8]) -> (Option<String>, Option<String>) {
    parse_tiff_exif_inner(tiff).unwrap_or((None, None))
}

fn parse_tiff_exif_inner(tiff: &[u8]) -> Option<(Option<String>, Option<String>)> {
    if tiff.len() < 8 {
        return None;
    }

    let is_little_endian = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };

    let read_u16 = |offset: usize| -> Option<u16> {
        if offset + 2 > tiff.len() { return None; }
        let bytes = [tiff[offset], tiff[offset + 1]];
        Some(if is_little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    };

    let read_u32 = |offset: usize| -> Option<u32> {
        if offset + 4 > tiff.len() { return None; }
        let bytes = [tiff[offset], tiff[offset + 1], tiff[offset + 2], tiff[offset + 3]];
        Some(if is_little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };

    let magic = read_u16(2)?;
    if magic != 42 {
        return None;
    }

    let ifd0_offset = read_u32(4)? as usize;
    if ifd0_offset >= tiff.len() {
        return None;
    }

    let mut model: Option<String> = None;
    let mut date: Option<String> = None;
    let mut exif_subifd_offset: Option<usize> = None;

    let num_entries = read_u16(ifd0_offset)? as usize;
    let mut entry_offset = ifd0_offset + 2;

    for _ in 0..num_entries {
        if entry_offset + 12 > tiff.len() { break; }
        let tag = read_u16(entry_offset).unwrap_or(0);
        let count = read_u32(entry_offset + 4).unwrap_or(0) as usize;
        let value_or_offset = read_u32(entry_offset + 8).unwrap_or(0) as usize;

        match tag {
            0x0110 => {
                // Model
                model = read_tiff_ascii(tiff, count, value_or_offset);
            }
            0x0132 => {
                // DateTime in IFD0
                if date.is_none() {
                    date = read_tiff_ascii(tiff, count, value_or_offset);
                }
            }
            0x8769 => {
                // ExifIFD sub-directory
                exif_subifd_offset = Some(value_or_offset);
            }
            _ => {}
        }
        entry_offset += 12;
    }

    // Traverse ExifIFD for DateTimeOriginal (0x9003)
    if let Some(subifd) = exif_subifd_offset {
        if subifd + 2 <= tiff.len() {
            if let Some(sub_entries) = read_u16(subifd) {
                let mut sub_offset = subifd + 2;
                for _ in 0..sub_entries {
                    if sub_offset + 12 > tiff.len() { break; }
                    let tag = read_u16(sub_offset).unwrap_or(0);
                    let count = read_u32(sub_offset + 4).unwrap_or(0) as usize;
                    let val_offset = read_u32(sub_offset + 8).unwrap_or(0) as usize;

                    if tag == 0x9003 {
                        // DateTimeOriginal
                        if let Some(dto) = read_tiff_ascii(tiff, count, val_offset) {
                            date = Some(dto);
                            break;
                        }
                    }
                    sub_offset += 12;
                }
            }
        }
    }

    let cleaned_date = date.map(|d| format_exif_date(&d));
    let cleaned_model = model.map(|m| sanitize_identifier(&m));

    Some((cleaned_date, cleaned_model))
}

fn read_tiff_ascii(tiff: &[u8], count: usize, offset_or_val: usize) -> Option<String> {
    if count == 0 { return None; }
    let raw = if count <= 4 {
        &tiff[offset_or_val.min(tiff.len())..]
    } else {
        if offset_or_val + count > tiff.len() { return None; }
        &tiff[offset_or_val..offset_or_val + count]
    };

    let s = String::from_utf8_lossy(raw);
    let trimmed = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn format_exif_date(raw: &str) -> String {
    // Standard EXIF format: "YYYY:MM:DD HH:MM:SS" -> "YYYY-MM-DD_HH-MM-SS"
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() >= 2 {
        let date_part = parts[0].replace(':', "-");
        let time_part = parts[1].replace(':', "-");
        format!("{date_part}_{time_part}")
    } else {
        raw.replace(':', "-").replace(' ', "_")
    }
}

// ----------------------------------------------------------------------
// ID3v2 Parser for MP3
// ----------------------------------------------------------------------

pub fn parse_id3v2(buf: &[u8]) -> (Option<String>, Option<String>, Option<String>) {
    if buf.len() < 10 || !buf.starts_with(b"ID3") {
        return (None, None, None);
    }

    let tag_size = ((buf[6] as usize & 0x7F) << 21)
        | ((buf[7] as usize & 0x7F) << 14)
        | ((buf[8] as usize & 0x7F) << 7)
        | (buf[9] as usize & 0x7F);

    let max_pos = (10 + tag_size).min(buf.len());
    let mut pos = 10;
    let mut artist: Option<String> = None;
    let mut title: Option<String> = None;
    let mut year: Option<String> = None;

    while pos + 10 < max_pos {
        let frame_id = &buf[pos..pos + 4];
        if frame_id[0] == 0 { break; } // padding reached

        let frame_size = ((buf[pos + 4] as usize) << 24)
            | ((buf[pos + 5] as usize) << 16)
            | ((buf[pos + 6] as usize) << 8)
            | (buf[pos + 7] as usize);

        if frame_size == 0 || pos + 10 + frame_size > max_pos {
            break;
        }

        let frame_data = &buf[pos + 10..pos + 10 + frame_size];
        let text = extract_id3_text(frame_data);

        if frame_id == b"TIT2" {
            title = text;
        } else if frame_id == b"TPE1" {
            artist = text;
        } else if frame_id == b"TYER" || frame_id == b"TDRC" {
            year = text;
        }

        pos += 10 + frame_size;
    }

    (artist.map(|a| sanitize_identifier(&a)), title.map(|t| sanitize_identifier(&t)), year)
}

fn extract_id3_text(data: &[u8]) -> Option<String> {
    if data.is_empty() { return None; }
    // byte 0 is encoding: 0 = ISO-8859-1, 3 = UTF-8
    let s = String::from_utf8_lossy(&data[1..]);
    let trimmed = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

// ----------------------------------------------------------------------
// PDF Metadata Sniffer (/Title, /CreationDate)
// ----------------------------------------------------------------------

pub fn parse_pdf_metadata(buf: &[u8]) -> (Option<String>, Option<String>) {
    let text = String::from_utf8_lossy(buf);

    let title = if let Some(idx) = text.find("/Title") {
        extract_pdf_literal_string(&text[idx + 6..])
    } else {
        None
    };

    let date = if let Some(idx) = text.find("/CreationDate") {
        extract_pdf_date(&text[idx + 13..])
    } else {
        None
    };

    (date, title.map(|t| sanitize_identifier(&t)))
}

fn extract_pdf_literal_string(slice: &str) -> Option<String> {
    let start = slice.find('(')?;
    let end = slice[start + 1..].find(')')?;
    let val = slice[start + 1..start + 1 + end].trim();
    if val.is_empty() { None } else { Some(val.to_string()) }
}

fn extract_pdf_date(slice: &str) -> Option<String> {
    // /CreationDate (D:YYYYMMDDHHmmSS...)
    let literal = extract_pdf_literal_string(slice)?;
    let clean = literal.trim_start_matches("D:");
    if clean.len() >= 8 {
        let yyyy = &clean[0..4];
        let mm = &clean[4..6];
        let dd = &clean[6..8];
        Some(format!("{yyyy}-{mm}-{dd}"))
    } else {
        None
    }
}

// ----------------------------------------------------------------------
// Canonical Name Formatter & Collision Resolver
// ----------------------------------------------------------------------

fn sanitize_identifier(s: &str) -> String {
    let mut res = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
            res.push(c);
        } else {
            res.push('_');
        }
    }
    let collapsed: String = res.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches(|c| c == '_' || c == ' ').to_string()
}

pub fn generate_canonical_name(
    meta: &CarvedMetadata,
    original_stem: &str,
    hash_prefix: &str,
) -> String {
    let ext = &meta.extension;

    // 1. Photos
    if matches!(ext.as_str(), "jpg" | "png" | "gif" | "webp") {
        if let Some(date) = &meta.date {
            if let Some(cam) = &meta.camera_or_artist {
                return format!("IMG_{date}_{cam}.{ext}");
            }
            return format!("IMG_{date}.{ext}");
        }
    }

    // 2. Audio
    if ext == "mp3" {
        if let (Some(artist), Some(title)) = (&meta.camera_or_artist, &meta.title) {
            return format!("{artist} - {title}.{ext}");
        } else if let Some(title) = &meta.title {
            return format!("{title}.{ext}");
        }
    }

    // 3. Documents
    if ext == "pdf" || ext == "docx" || ext == "xlsx" || ext == "pptx" {
        if let (Some(title), Some(date)) = (&meta.title, &meta.date) {
            return format!("{title}_{date}.{ext}");
        } else if let Some(title) = &meta.title {
            return format!("{title}.{ext}");
        }
    }

    // 4. Fallback for unrecognized or unnamed files
    if !original_stem.is_empty() && !is_generic_recovery_name(original_stem) {
        format!("{original_stem}.{ext}")
    } else {
        format!("RECOVERED_{hash_prefix}.{ext}")
    }
}

pub fn is_generic_recovery_name(stem: &str) -> bool {
    let s = stem.to_lowercase();
    s.starts_with("recovered_")
        || s.starts_with("file_")
        || (s.starts_with('f') && s.chars().skip(1).all(|c| c.is_ascii_digit()))
        || (s.starts_with("img_") && s.chars().skip(4).all(|c| c.is_ascii_digit()))
        || (s.starts_with("dcim_") && s.chars().skip(5).all(|c| c.is_ascii_digit()))
        || s == "unknown"
        || s == "binary"
        || s == "data"
}

/// Resolves unique filename by appending _1, _2 if destination already exists.
pub fn resolve_collision_safe_path(parent_dir: &Path, proposed_name: &str) -> PathBuf {
    let mut candidate = parent_dir.join(proposed_name);
    if !candidate.exists() {
        return candidate;
    }

    let p = Path::new(proposed_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

    let mut counter = 1usize;
    loop {
        let new_name = if ext.is_empty() {
            format!("{stem}_{counter}")
        } else {
            format!("{stem}_{counter}.{ext}")
        };
        candidate = parent_dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

// ----------------------------------------------------------------------
// Directory Scanning & Execution Engine
// ----------------------------------------------------------------------

pub fn scan_carving_proposals(folder_path: &Path) -> Result<Vec<CarvedFileProposal>, String> {
    if !folder_path.is_dir() {
        return Err(format!("path is not a directory: {}", folder_path.display()));
    }

    let entries = fs::read_dir(folder_path)
        .map_err(|e| format!("unable to read directory {}: {e}", folder_path.display()))?;

    let mut proposals = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let original_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let mut file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let mut buffer = vec![0u8; 65536];
        let bytes_read = match file.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => continue,
        };
        buffer.truncate(bytes_read);

        if let Some(meta) = sniff_metadata(&buffer) {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let current_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

            // Hash prefix (first 8 chars of sha256 or truncated size)
            let hash_prefix = format!("{:08x}", bytes_read);
            let canonical_name = generate_canonical_name(&meta, stem, &hash_prefix);

            // Propose rename if extension is missing/wrong or name is generic
            let is_generic = is_generic_recovery_name(stem);
            let ext_mismatch = current_ext != meta.extension;

            if is_generic || ext_mismatch || canonical_name != original_name {
                let parent = path.parent().unwrap_or_else(|| Path::new("."));
                let safe_path = resolve_collision_safe_path(parent, &canonical_name);
                let final_name = safe_path.file_name().and_then(|n| n.to_str()).unwrap_or(&canonical_name).to_string();

                proposals.push(CarvedFileProposal {
                    source_path: path.to_string_lossy().to_string(),
                    original_name,
                    detected_mime: meta.mime,
                    detected_extension: meta.extension,
                    extracted_date: meta.date,
                    extracted_camera_or_artist: meta.camera_or_artist,
                    extracted_title: meta.title,
                    proposed_name: final_name,
                    proposed_path: safe_path.to_string_lossy().to_string(),
                    confidence_score: meta.confidence,
                });
            }
        }
    }

    Ok(proposals)
}

pub fn execute_carve_renames(items: &[(String, String)]) -> CarveExecutionResult {
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();

    for (src_str, dst_str) in items {
        let src = Path::new(src_str);
        let dst = Path::new(dst_str);

        if !src.exists() {
            failed += 1;
            errors.push((src_str.clone(), "source file does not exist".to_string()));
            continue;
        }

        if dst.exists() {
            failed += 1;
            errors.push((src_str.clone(), "destination file already exists".to_string()));
            continue;
        }

        match fs::rename(src, dst) {
            Ok(()) => {
                succeeded += 1;
            }
            Err(e) => {
                // Try copy + delete if cross-device
                if fs::copy(src, dst).is_ok() {
                    if fs::remove_file(src).is_ok() {
                        succeeded += 1;
                        continue;
                    }
                    let _ = fs::remove_file(dst);
                }
                failed += 1;
                errors.push((src_str.clone(), e.to_string()));
            }
        }
    }

    CarveExecutionResult {
        succeeded,
        failed,
        errors,
    }
}
