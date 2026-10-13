//! Multi-copy block stitching and file reconstruction.
//!
//! Reconstructs a clean file from two or more damaged copies (e.g. from bad disk sectors)
//! by analyzing 4KB block sectors and combining healthy regions.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub const SECTOR_CHUNK_SIZE: usize = 4096;

/// Classification of a specific block during multi-copy analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockSource {
    /// Chunk taken from Candidate A because B has a zero-hole.
    SourceA,
    /// Chunk taken from Candidate B because A has a zero-hole.
    SourceB,
    /// Chunk is identical in both files.
    IdenticalBoth,
    /// Chunk is zeroed out in both candidates (unrecoverable without parity).
    UnrecoverableNull,
    /// Both chunks contain non-zero data, but the bytes differ.
    ConflictDifferentData,
}

/// Sector-by-sector map of reconstructed chunks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconstructionBlockMap {
    pub chunk_size: usize,
    pub total_chunks: usize,
    pub chunks_from_a: usize,
    pub chunks_from_b: usize,
    pub identical_chunks: usize,
    pub unrecoverable_chunks: usize,
    pub conflicting_chunks: usize,
    pub block_sources: Vec<BlockSource>,
}

/// Plan for reconstructing a healthy file from two damaged candidates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconstructionPlan {
    pub candidate_a_path: String,
    pub candidate_b_path: String,
    pub output_path: String,
    pub file_size: u64,
    pub block_map: ReconstructionBlockMap,
    pub potential_recovery_percentage: f32,
    pub can_achieve_full_repair: bool,
}

/// Result of executing a reconstruction operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconstructionResult {
    pub output_path: String,
    pub bytes_written: u64,
    pub sha256: String,
    pub resulting_health_score: u8,
    pub remaining_null_holes: usize,
}

/// Inspects two files to produce a block-by-block reconstruction plan.
pub fn analyze_reconstruction_plan(
    path_a: &Path,
    path_b: &Path,
) -> Result<ReconstructionPlan, String> {
    if !path_a.exists() {
        return Err(format!("Candidate A does not exist: {}", path_a.display()));
    }
    if !path_b.exists() {
        return Err(format!("Candidate B does not exist: {}", path_b.display()));
    }

    let meta_a = std::fs::metadata(path_a).map_err(|e| e.to_string())?;
    let meta_b = std::fs::metadata(path_b).map_err(|e| e.to_string())?;

    // Determine target file size
    let target_size = meta_a.len().max(meta_b.len());
    if target_size == 0 {
        return Err("Both candidate files are 0 bytes".into());
    }

    let mut file_a = BufReader::new(File::open(path_a).map_err(|e| e.to_string())?);
    let mut file_b = BufReader::new(File::open(path_b).map_err(|e| e.to_string())?);

    let mut buf_a = vec![0u8; SECTOR_CHUNK_SIZE];
    let mut buf_b = vec![0u8; SECTOR_CHUNK_SIZE];

    let mut block_sources = Vec::new();
    let mut chunks_from_a = 0;
    let mut chunks_from_b = 0;
    let mut identical_chunks = 0;
    let mut unrecoverable_chunks = 0;
    let mut conflicting_chunks = 0;

    let mut total_chunks = 0;

    loop {
        let read_a = file_a.read(&mut buf_a).map_err(|e| e.to_string())?;
        let read_b = file_b.read(&mut buf_b).map_err(|e| e.to_string())?;

        if read_a == 0 && read_b == 0 {
            break;
        }

        total_chunks += 1;

        // Zero out rest of buffer if read was partial
        if read_a < SECTOR_CHUNK_SIZE {
            buf_a[read_a..].fill(0);
        }
        if read_b < SECTOR_CHUNK_SIZE {
            buf_b[read_b..].fill(0);
        }

        let a_is_null = buf_a.iter().all(|&b| b == 0);
        let b_is_null = buf_b.iter().all(|&b| b == 0);

        if a_is_null && b_is_null {
            unrecoverable_chunks += 1;
            block_sources.push(BlockSource::UnrecoverableNull);
        } else if a_is_null && !b_is_null {
            chunks_from_b += 1;
            block_sources.push(BlockSource::SourceB);
        } else if !a_is_null && b_is_null {
            chunks_from_a += 1;
            block_sources.push(BlockSource::SourceA);
        } else if buf_a == buf_b {
            identical_chunks += 1;
            block_sources.push(BlockSource::IdenticalBoth);
        } else {
            conflicting_chunks += 1;
            block_sources.push(BlockSource::ConflictDifferentData);
        }
    }

    let recoverable_chunks = chunks_from_a + chunks_from_b + identical_chunks;
    let potential_recovery_percentage = if total_chunks > 0 {
        (recoverable_chunks as f32 / total_chunks as f32) * 100.0
    } else {
        0.0
    };

    let can_achieve_full_repair = unrecoverable_chunks == 0 && conflicting_chunks == 0;

    // Suggest collision-safe reconstructed output path
    let parent = path_a.parent().unwrap_or_else(|| Path::new("."));
    let stem = path_a.file_stem().and_then(|s| s.to_str()).unwrap_or("recovered");
    let ext = path_a.extension().and_then(|s| s.to_str()).unwrap_or("");
    let rec_name = if ext.is_empty() {
        format!("{}.reconstructed", stem)
    } else {
        format!("{}.reconstructed.{}", stem, ext)
    };
    let output_path = crate::triage::resolve_collision_path(&parent.join(rec_name))
        .to_string_lossy()
        .to_string();

    Ok(ReconstructionPlan {
        candidate_a_path: path_a.to_string_lossy().to_string(),
        candidate_b_path: path_b.to_string_lossy().to_string(),
        output_path,
        file_size: target_size,
        block_map: ReconstructionBlockMap {
            chunk_size: SECTOR_CHUNK_SIZE,
            total_chunks,
            chunks_from_a,
            chunks_from_b,
            identical_chunks,
            unrecoverable_chunks,
            conflicting_chunks,
            block_sources,
        },
        potential_recovery_percentage,
        can_achieve_full_repair,
    })
}

/// Executes block stitching according to the reconstruction plan.
pub fn execute_reconstruction(
    plan: &ReconstructionPlan,
) -> Result<ReconstructionResult, String> {
    let path_a = PathBuf::from(&plan.candidate_a_path);
    let path_b = PathBuf::from(&plan.candidate_b_path);
    let out_path = PathBuf::from(&plan.output_path);

    if !path_a.exists() {
        return Err(format!("Source A not found: {}", path_a.display()));
    }
    if !path_b.exists() {
        return Err(format!("Source B not found: {}", path_b.display()));
    }

    let mut file_a = BufReader::new(File::open(&path_a).map_err(|e| e.to_string())?);
    let mut file_b = BufReader::new(File::open(&path_b).map_err(|e| e.to_string())?);
    let mut out_file = BufWriter::new(File::create(&out_path).map_err(|e| e.to_string())?);

    let mut buf_a = vec![0u8; SECTOR_CHUNK_SIZE];
    let mut buf_b = vec![0u8; SECTOR_CHUNK_SIZE];

    let mut bytes_written: u64 = 0;
    let mut remaining_bytes = plan.file_size;

    for &source in &plan.block_map.block_sources {
        let read_a = file_a.read(&mut buf_a).map_err(|e| e.to_string())?;
        let _read_b = file_b.read(&mut buf_b).map_err(|e| e.to_string())?;

        let to_write = (SECTOR_CHUNK_SIZE as u64).min(remaining_bytes) as usize;
        if to_write == 0 {
            break;
        }

        let chunk_to_write = match source {
            BlockSource::SourceA | BlockSource::IdenticalBoth => &buf_a[..to_write],
            BlockSource::SourceB => &buf_b[..to_write],
            BlockSource::ConflictDifferentData => {
                // By default, prefer Candidate A for conflicted non-zero bytes
                if read_a > 0 { &buf_a[..to_write] } else { &buf_b[..to_write] }
            }
            BlockSource::UnrecoverableNull => {
                // Write null block
                &buf_a[..to_write]
            }
        };

        out_file.write_all(chunk_to_write).map_err(|e| e.to_string())?;
        bytes_written += to_write as u64;
        remaining_bytes -= to_write as u64;
    }

    out_file.flush().map_err(|e| e.to_string())?;
    drop(out_file);

    // Compute health evaluation on the newly synthesized file
    let health_report = crate::health::evaluate_file_health(&out_path)
        .map_err(|e| format!("Failed to evaluate health of synthesized file: {}", e))?;

    Ok(ReconstructionResult {
        output_path: plan.output_path.clone(),
        bytes_written,
        sha256: health_report.sha256,
        resulting_health_score: health_report.health_score,
        remaining_null_holes: plan.block_map.unrecoverable_chunks,
    })
}
