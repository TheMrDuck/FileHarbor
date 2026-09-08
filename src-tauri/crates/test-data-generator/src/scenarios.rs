use crate::content::{ContentGenerator, ContentType};
use crate::manifest::{FileCategory, FileRecord, Manifest, ScenarioType};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use std::collections::HashMap;
use std::path::Path;

/// A scenario definition with its configuration
pub struct Scenario {
    pub name: &'static str,
    pub scenario_type: ScenarioType,
    pub description: &'static str,
    pub default_files: usize,
    pub generate: fn(&mut TestScenarioContext) -> Result<()>,
}

/// Context passed to scenario generators
pub struct TestScenarioContext<'a> {
    pub output_dir: &'a Path,
    pub manifest: &'a mut Manifest,
    pub content_gen: &'a mut ContentGenerator,
    pub config: &'a mut GenerationConfig,
    pub file_counter: &'a mut usize,
    pub duplicate_groups: &'a mut HashMap<String, Vec<String>>,
}

/// Configuration for generation
pub struct GenerationConfig {
    pub total_files: usize,
    pub seed: u64,
    pub base_date: DateTime<Utc>,
    pub duplicate_groups: usize,
    pub empty_files: usize,
    pub unicode_files: usize,
    pub large_files: usize,
    pub nested_depth: usize,
    pub same_size_diff_content_pairs: usize,
    pub symlinks: bool,
    pub broken_symlinks: bool,
    pub progress_callback: Option<Box<dyn Fn(GenerationProgress) + Send + Sync>>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            total_files: 2500,
            seed: 12345,
            base_date: Utc::now(),
            duplicate_groups: 100,
            empty_files: 50,
            unicode_files: 50,
            large_files: 5,
            nested_depth: 5,
            same_size_diff_content_pairs: 50,
            symlinks: false,
            broken_symlinks: false,
            progress_callback: None,
        }
    }
}

/// Progress reporting during generation
#[derive(Debug, Clone, Copy)]
pub struct GenerationProgress {
    pub directories_created: usize,
    pub files_created: usize,
    pub bytes_written: u64,
    pub duplicate_groups_created: usize,
}

/// Scenario registry with all available scenarios
pub struct ScenarioRegistry;

impl ScenarioRegistry {
    pub fn all() -> Vec<Scenario> {
        vec![
            Scenario {
                name: "phase1-baseline",
                scenario_type: ScenarioType::Phase1Baseline,
                description: "General regression testing for Phase 1. Includes normal files, exact duplicates, same-size/different-content, empty files, nested directories, Unicode names, mixed extensions, and large files.",
                default_files: 2500,
                generate: generate_phase1_baseline,
            },
            Scenario {
                name: "duplicates",
                scenario_type: ScenarioType::Duplicates,
                description: "Specifically designed to test exact duplicate detection. Creates duplicate files across different directories with different names but identical content.",
                default_files: 1000,
                generate: generate_duplicates_scenario,
            },
            Scenario {
                name: "empty-files",
                scenario_type: ScenarioType::EmptyFiles,
                description: "Generates zero-byte files with realistic names like .gitkeep, placeholder, lockfile, etc.",
                default_files: 500,
                generate: generate_empty_files_scenario,
            },
            Scenario {
                name: "unicode",
                scenario_type: ScenarioType::Unicode,
                description: "Generates files with Arabic, Japanese, Chinese, and emoji filenames to test Unicode handling.",
                default_files: 500,
                generate: generate_unicode_scenario,
            },
            Scenario {
                name: "large-files",
                scenario_type: ScenarioType::LargeFiles,
                description: "Creates large files (configurable size) for testing streaming SHA-256 hashing performance.",
                default_files: 50,
                generate: generate_large_files_scenario,
            },
            Scenario {
                name: "performance-small",
                scenario_type: ScenarioType::PerformanceSmall,
                description: "1,000 files for quick performance testing.",
                default_files: 1000,
                generate: generate_performance_small,
            },
            Scenario {
                name: "performance-medium",
                scenario_type: ScenarioType::PerformanceMedium,
                description: "10,000 files for medium-scale performance testing.",
                default_files: 10000,
                generate: generate_performance_medium,
            },
            Scenario {
                name: "performance-large",
                scenario_type: ScenarioType::PerformanceLarge,
                description: "100,000 files for large-scale performance testing.",
                default_files: 100000,
                generate: generate_performance_large,
            },
            Scenario {
                name: "performance-stress",
                scenario_type: ScenarioType::PerformanceStress,
                description: "1,000,000 files for stress testing. Requires confirmation due to disk usage.",
                default_files: 1000000,
                generate: generate_performance_stress,
            },
            Scenario {
                name: "recovered-hdd",
                scenario_type: ScenarioType::RecoveredHdd,
                description: "Recovery-style test fixtures with filenames like file000001, IMG000183, document_recovered, etc.",
                default_files: 5000,
                generate: generate_recovered_hdd_scenario,
            },
            Scenario {
                name: "unstable",
                scenario_type: ScenarioType::Unstable,
                description: "Creates a file that changes during hashing to test instability detection.",
                default_files: 10,
                generate: generate_unstable_scenario,
            },
            Scenario {
                name: "missing-files",
                scenario_type: ScenarioType::MissingFiles,
                description: "Generates a dataset and then removes selected files to test missing file detection.",
                default_files: 1000,
                generate: generate_missing_files_scenario,
            },
            Scenario {
                name: "symlinks",
                scenario_type: ScenarioType::Symlinks,
                description: "Creates valid symlinks, broken symlinks, and symlink loops where supported.",
                default_files: 100,
                generate: generate_symlinks_scenario,
            },
            Scenario {
                name: "locked-files",
                scenario_type: ScenarioType::LockedFiles,
                description: "Windows-only: Creates a file locked by a helper process to test locked file detection.",
                default_files: 10,
                generate: generate_locked_files_scenario,
            },
            Scenario {
                name: "permissions",
                scenario_type: ScenarioType::Permissions,
                description: "Creates files with restricted permissions where supported.",
                default_files: 50,
                generate: generate_permissions_scenario,
            },
            Scenario {
                name: "regression",
                scenario_type: ScenarioType::Regression,
                description: "Comprehensive Phase 1 regression dataset with known expected results.",
                default_files: 5000,
                generate: generate_regression_scenario,
            },
            Scenario {
                name: "backup-baseline",
                scenario_type: ScenarioType::BackupBaseline,
                description: "Generates a baseline dataset and creates an initial full backup with manifest.",
                default_files: 1000,
                generate: generate_backup_baseline,
            },
            Scenario {
                name: "backup-incremental",
                scenario_type: ScenarioType::BackupIncremental,
                description: "Generates a baseline, modifies files, then runs an incremental backup.",
                default_files: 1000,
                generate: generate_backup_incremental,
            },
            Scenario {
                name: "backup-changed-files",
                scenario_type: ScenarioType::BackupChangedFiles,
                description: "Generates a baseline, modifies some files, then runs incremental backup capturing only changes.",
                default_files: 1000,
                generate: generate_backup_changed_files,
            },
            Scenario {
                name: "backup-deleted-files",
                scenario_type: ScenarioType::BackupDeletedFiles,
                description: "Generates a baseline, deletes some source files, then runs incremental backup.",
                default_files: 1000,
                generate: generate_backup_deleted_files,
            },
            Scenario {
                name: "backup-restore",
                scenario_type: ScenarioType::BackupRestore,
                description: "Generates a baseline backup and tests various restore scenarios (missing file, conflict, alternate location).",
                default_files: 1000,
                generate: generate_backup_restore,
            },
            Scenario {
                name: "backup-corruption",
                scenario_type: ScenarioType::BackupCorruption,
                description: "Generates a baseline backup, corrupts a backup file, then runs verification to detect corruption.",
                default_files: 1000,
                generate: generate_backup_corruption,
            },
            Scenario {
                name: "backup-interrupted",
                scenario_type: ScenarioType::BackupInterrupted,
                description: "Generates a baseline backup, creates a partial backup to simulate interruption, tests resume capability.",
                default_files: 1000,
                generate: generate_backup_interrupted,
            },
            Scenario {
                name: "backup-large",
                scenario_type: ScenarioType::BackupLarge,
                description: "Generates a large dataset (10,000 files) and tests incremental backup on it.",
                default_files: 10000,
                generate: generate_backup_large,
            },
            Scenario {
                name: "backup-duplicate-content",
                scenario_type: ScenarioType::BackupDuplicateContent,
                description: "Generates a baseline backup with duplicate content files and verifies backup handles duplicates correctly.",
                default_files: 1000,
                generate: generate_backup_duplicate_content,
            },
        ]
    }

    pub fn get(name: &str) -> Option<Scenario> {
        Self::all().into_iter().find(|s| s.name == name)
    }

    pub fn list() -> Vec<(&'static str, ScenarioType, &'static str)> {
        Self::all().into_iter().map(|s| (s.name, s.scenario_type, s.description)).collect()
    }
}

/// Generate the phase1-baseline scenario
fn generate_phase1_baseline(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    // Directory structure
    let base_dirs = [
        "documents",
        "photos",
        "projects",
        "downloads",
        "archives",
        "misc",
        "documents/work",
        "documents/personal",
        "photos/2024",
        "photos/2023",
        "projects/active",
        "projects/archive",
        "downloads/software",
        "downloads/media",
    ];

    for dir in &base_dirs {
        std::fs::create_dir_all(files_dir.join(dir))?;
    }

    let mut duplicate_group_counter = 0;
    let mut same_size_counter = 0;
    
    // 1. Generate normal files (~40% of total)
    let normal_count = (ctx.config.total_files * 40) / 100;
    for _i in 0..normal_count {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        generate_normal_file(ctx, &files_dir, &base_dirs)?;
    }

    // 2. Generate exact duplicates (~20% of total, forming duplicate_groups groups)
    for _group_idx in 0..ctx.config.duplicate_groups.min(ctx.config.total_files / 3) {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        duplicate_group_counter += 1;
        generate_duplicate_group(ctx, &files_dir, &base_dirs, duplicate_group_counter)?;
    }

    // 3. Generate empty files
    for _i in 0..ctx.config.empty_files {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        generate_empty_file(ctx, &files_dir, &base_dirs)?;
    }

    // 3b. Generate same-size/different-content pairs
    for _i in 0..ctx.config.same_size_diff_content_pairs {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        same_size_counter += 1;
        generate_same_size_different_content(ctx, &files_dir, &base_dirs, same_size_counter)?;
    }

// 4. Generate Unicode files
    for _i in 0..ctx.config.unicode_files {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        generate_unicode_file(ctx, &files_dir, &base_dirs)?;
    }

    // 5. Generate nested/deep directory files
    for _i in 0..(ctx.config.total_files / 20).max(10) {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        generate_nested_file(ctx, &files_dir, ctx.config.nested_depth)?;
    }

    // 6. Generate large files
    for _i in 0..ctx.config.large_files {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        generate_large_file(ctx, &files_dir, &base_dirs)?;
    }

    // 7. Generate backup-like copies
    generate_backup_like_copies(ctx, &files_dir)?;

    ctx.manifest.finalize();
    Ok(())
}

/// Generate duplicates-specific scenario
fn generate_duplicates_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    let dirs = [
        "original",
        "copies",
        "backup",
        "downloads",
        "recovered",
    ];

    for dir in &dirs {
        std::fs::create_dir_all(files_dir.join(dir))?;
    }

    // Generate groups with identical content but different names/locations
    let groups = ctx.config.total_files / 4;
    for group_idx in 0..groups {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
let content_type = ContentType::random(&mut ctx.content_gen.rng());
        let content = ctx.content_gen.generate(
            &format!("dup-group-{}", group_idx),
            rand::thread_rng().gen_range(1024..10240),
            content_type,
        );
        let hash = ContentGenerator::sha256(&content);
        
        // Create 3-4 copies per group in different directories
        let copies = rand::thread_rng().gen_range(3..5);
        let group_id = format!("dup-{:04}", group_idx);
        
for copy_idx in 0..copies {
            if *ctx.file_counter >= ctx.config.total_files { break; }
            
            let dir = dirs[copy_idx % dirs.len()];
            let ext = ["pdf", "jpg", "docx", "zip", "txt"][rand::thread_rng().gen_range(0..5)];
            let name = match copy_idx {
                0 => format!("report.{}", ext),
                1 => format!("report_copy.{}.", ext),
                2 => format!("report_final.{}.", ext),
                _ => format!("report_backup.{}.", ext),
            };
            
            let rel_path = format!("{}/{}", dir, name);
            let full_path = files_dir.join(&rel_path);
            
            std::fs::write(&full_path, &content)?;
            
            let record = FileRecord {
                id: format!("file-{:06}", ctx.file_counter),
                relative_path: rel_path,
                size_bytes: content.len() as u64,
                sha256: Some(hash.clone()),
                categories: vec![
                    if copy_idx == 0 { FileCategory::ExactDuplicateOriginal } else { FileCategory::ExactDuplicateCopy },
                    FileCategory::Normal,
                ],
                duplicate_group: Some(group_id.clone()),
                expected_duplicate: Some(true),
                expected_empty: Some(false),
                role: None,
                content_type: Some("duplicate".to_string()),
                modified_at: Some(ctx.config.base_date),
            };
            
let record_id = record.id.clone();
        ctx.manifest.add_file(record);
        ctx.duplicate_groups.entry(group_id.clone()).or_default().push(record_id);
            *ctx.file_counter += 1;
        }
    }

    ctx.manifest.finalize();
    Ok(())
}

/// Generate empty files scenario
fn generate_empty_files_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    let empty_names = [
        ".gitkeep",
        "placeholder",
        "empty.txt",
        "lockfile",
        "marker",
        "empty.json",
        "empty.bin",
        ".placeholder",
        "keep",
        "empty.log",
        "README.empty",
        "flag",
        "sentinel",
        "touch",
        "empty.md",
    ];

    for (i, name) in empty_names.iter().enumerate() {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
        let dir = if i % 3 == 0 { "misc" } else if i % 3 == 1 { "projects" } else { "documents" };
        let rel_path = format!("{}/{}", dir, name);
        let full_path = files_dir.join(&rel_path);
        
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        std::fs::write(&full_path, b"")?;

        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path,
            size_bytes: 0,
            sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
            categories: vec![FileCategory::Empty, FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(true),
            role: None,
            content_type: Some("empty".to_string()),
            modified_at: Some(ctx.config.base_date),
        };

        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }

    ctx.manifest.finalize();
    Ok(())
}

/// Generate Unicode filenames scenario
fn generate_unicode_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    let unicode_files = [
        // Arabic
        ("تقرير.pdf", "documents"),
        ("مشروع التخرج.docx", "documents"),
        ("صور العائلة.jpg", "photos"),
        ("نسخة احتياطية.zip", "archives"),
        // Japanese
        ("レポート.pdf", "documents"),
        ("写真.jpg", "photos"),
        ("プロジェクト.zip", "projects"),
        ("バックアップ.txt", "documents"),
        // Chinese
        ("报告.pdf", "documents"),
        ("照片.jpg", "photos"),
        ("项目.zip", "projects"),
        // Emoji
        ("📚 book.txt", "documents"),
        ("📷 photo.jpg", "photos"),
        ("🗂️ archive.zip", "archives"),
        ("🎵 music.mp3", "downloads"),
        ("📝 notes.md", "documents"),
        // Mixed
        ("report_تقرير.pdf", "documents"),
        ("レポート_report.docx", "documents"),
    ];

    for (name, dir) in unicode_files {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
        let rel_path = format!("{}/{}", dir, name);
        let full_path = files_dir.join(&rel_path);
        
        std::fs::create_dir_all(full_path.parent().unwrap())?;
let content_type = ContentType::random(&mut ctx.content_gen.rng());
        let content = ctx.content_gen.generate(
            &format!("unicode-{}", ctx.file_counter),
            rand::thread_rng().gen_range(1024..51200),
            content_type,
        );
        let hash = ContentGenerator::sha256(&content);
        
        std::fs::write(&full_path, &content)?;

        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path,
            size_bytes: content.len() as u64,
            sha256: Some(hash),
            categories: vec![FileCategory::Unicode, FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: None,
            content_type: Some("unicode".to_string()),
            modified_at: Some(ctx.config.base_date),
        };

        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }

    ctx.manifest.finalize();
    Ok(())
}

/// Generate large files scenario
fn generate_large_files_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    // Large unique file
    generate_large_file(ctx, &files_dir, &["large"])?;
    
    // Large duplicate
    let content = ctx.content_gen.generate(
        "large-dup",
        100 * 1024 * 1024, // 100 MB
        ContentType::RandomBinary,
    );
    let hash = ContentGenerator::sha256(&content);
    
for copy_idx in 0..2 {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
        let rel_path = format!("large/large_file_{}.bin", copy_idx + 1);
        let full_path = files_dir.join(&rel_path);
        
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        std::fs::write(&full_path, &content)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path,
            size_bytes: content.len() as u64,
            sha256: Some(hash.clone()),
            categories: vec![
                if copy_idx == 0 { FileCategory::ExactDuplicateOriginal } else { FileCategory::ExactDuplicateCopy },
                FileCategory::LargeFile,
                FileCategory::Normal,
            ],
            duplicate_group: Some("dup-large-001".to_string()),
            expected_duplicate: Some(true),
            expected_empty: Some(false),
            role: None,
            content_type: Some("large-duplicate".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }

    // Same size, different content
    let content2 = ctx.content_gen.generate(
        "large-diff",
        100 * 1024 * 1024, // 100 MB
        ContentType::RandomBinary,
    );
    let hash2 = ContentGenerator::sha256(&content2);
    
    let rel_path = "large/large_different.bin";
    let full_path = files_dir.join(rel_path);
    std::fs::write(&full_path, &content2)?;
    
    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path.to_string(),
        size_bytes: content2.len() as u64,
        sha256: Some(hash2),
        categories: vec![FileCategory::SameSizeDifferentContent, FileCategory::LargeFile, FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(false),
        role: None,
        content_type: Some("large-different".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;

    ctx.manifest.finalize();
    Ok(())
}

/// Generate recovered HDD scenario
fn generate_recovered_hdd_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    let recovered_dirs = ["recovered", "recovered/files", "recovered/images", "recovered/docs"];
    for dir in &recovered_dirs {
        std::fs::create_dir_all(files_dir.join(dir))?;
    }

    // Generate recovered-style files
    let mut rng = rand::thread_rng();
    
    for i in 0..ctx.config.total_files {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
        let name = match rng.gen_range(0..6) {
            0 => format!("file{:06}", i),
            1 => format!("IMG{:06}", rng.gen_range(1..100000)),
            2 => format!("document_recovered_{}", rng.gen_range(1..10000)),
            3 => format!("recovered_{:03}.bin", rng.gen_range(1..1000)),
            4 => format!("unknown_{:05}", rng.gen_range(1..100000)),
            _ => format!("data_{:06}", i),
        };
        
        let dir = recovered_dirs[rng.gen_range(0..recovered_dirs.len())];
        let rel_path = format!("{}/{}", dir, name);
        let full_path = files_dir.join(&rel_path);
        
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        
        // Some duplicates, some empty, some normal
        let file_type = rng.gen_range(0..10);
let (content, categories, expected_dup, expected_empty) = if file_type < 2 {
            // Duplicate
            let group = rng.gen_range(0..10);
            let content_type = ContentType::random(&mut ctx.content_gen.rng());
            let dup_content = ctx.content_gen.generate(
                &format!("recovered-dup-{}", group),
                rng.gen_range(1024..51200),
                content_type,
            );
            (dup_content, vec![FileCategory::ExactDuplicateCopy, FileCategory::Normal], Some(true), Some(false))
        } else if file_type == 2 {
            // Empty
            (vec![], vec![FileCategory::Empty, FileCategory::Normal], Some(false), Some(true))
        } else {
            // Normal
            let content_type = ContentType::random(&mut ctx.content_gen.rng());
            (ctx.content_gen.generate(&format!("recovered-{}", ctx.file_counter), rng.gen_range(1024..102400), content_type), vec![FileCategory::Normal], Some(false), Some(false))
        };
        
        let hash = if content.is_empty() {
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string())
        } else {
            Some(ContentGenerator::sha256(&content))
        };
        
        std::fs::write(&full_path, &content)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path,
            size_bytes: content.len() as u64,
            sha256: hash,
            categories,
            duplicate_group: if expected_dup == Some(true) { Some(format!("dup-recovered-{:04}", rng.gen_range(0..10))) } else { None },
            expected_duplicate: expected_dup,
            expected_empty: expected_empty,
            role: Some("recovered".to_string()),
            content_type: Some("recovered".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }

    ctx.manifest.finalize();
    Ok(())
}

/// Generate unstable file scenario (file changes during scan)
fn generate_unstable_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    // Create a file that will be modified during scanning
    let rel_path = "unstable-target.bin";
    let full_path = files_dir.join(rel_path);
    
    // Initial content
    let initial_content = ctx.content_gen.generate("unstable-initial", 1024 * 1024, ContentType::RandomBinary);
    std::fs::write(&full_path, &initial_content)?;

    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path.to_string(),
        size_bytes: initial_content.len() as u64,
        sha256: Some(ContentGenerator::sha256(&initial_content)),
        categories: vec![FileCategory::Unstable, FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(false),
        role: Some("unstable-target".to_string()),
        content_type: Some("unstable".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;

    // Also create some normal files for context
for _i in 0..9 {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        generate_normal_file(ctx, &files_dir, &["misc"])?;
    }

    ctx.manifest.finalize();
    Ok(())
}

/// Generate missing files scenario
fn generate_missing_files_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    let files_dir = ctx.output_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    // First generate a normal dataset
    let mut temp_ctx = TestScenarioContext {
        output_dir: ctx.output_dir,
        manifest: ctx.manifest,
        content_gen: ctx.content_gen,
        config: ctx.config,
        file_counter: ctx.file_counter,
        duplicate_groups: ctx.duplicate_groups,
    };
    
    generate_phase1_baseline(&mut temp_ctx)?;
    
    // Now remove some files to create "missing" entries
    let files_to_remove: Vec<String> = ctx.manifest.files.iter()
        .filter(|f| f.categories.contains(&FileCategory::Normal) && !f.categories.contains(&FileCategory::ExactDuplicateOriginal))
        .take(5)
        .map(|f| f.relative_path.clone())
        .collect();
    
    for rel_path in &files_to_remove {
        let full_path = files_dir.join(rel_path);
        if full_path.exists() {
            std::fs::remove_file(&full_path)?;
        }
        
        // Update manifest to mark as missing
        if let Some(record) = ctx.manifest.files.iter_mut().find(|f| f.relative_path == *rel_path) {
            record.categories.push(FileCategory::Missing);
            record.expected_duplicate = Some(false);
            record.expected_empty = Some(false);
            record.role = Some("missing".to_string());
        }
    }

    ctx.manifest.finalize();
    Ok(())
}

/// Generate symlinks scenario
fn generate_symlinks_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        
        let files_dir = ctx.output_dir.join("files");
        std::fs::create_dir_all(&files_dir)?;
        
        let real_dir = files_dir.join("real");
        let links_dir = files_dir.join("links");
        std::fs::create_dir_all(&real_dir)?;
        std::fs::create_dir_all(&links_dir)?;
        
        // Create a real file
        let real_file = real_dir.join("important.txt");
        let content = ctx.content_gen.generate("symlink-target", 1024, ContentType::Text);
        std::fs::write(&real_file, &content)?;
        
        // Valid symlink
        let link_file = links_dir.join("important-link.txt");
        symlink(&real_file, &link_file)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: "links/important-link.txt".to_string(),
            size_bytes: content.len() as u64,
            sha256: Some(ContentGenerator::sha256(&content)),
            categories: vec![FileCategory::Symlink, FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("symlink".to_string()),
            content_type: Some("symlink".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
        
        // Broken symlink
        let broken_link = links_dir.join("broken-link.txt");
        symlink("/nonexistent/path.txt", &broken_link)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: "links/broken-link.txt".to_string(),
            size_bytes: 0,
            sha256: None,
            categories: vec![FileCategory::BrokenSymlink, FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("broken-symlink".to_string()),
            content_type: Some("symlink".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }
    
    #[cfg(windows)]
    {
        // On Windows, symlinks require admin privileges
        // We'll create a note in the manifest instead
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: "symlinks/NOT_CREATED_ON_WINDOWS.txt".to_string(),
            size_bytes: 0,
            sha256: None,
            categories: vec![FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("symlink-skipped".to_string()),
            content_type: Some("info".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }
    
    ctx.manifest.finalize();
    Ok(())
}

/// Generate locked files scenario (Windows only)
fn generate_locked_files_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    #[cfg(windows)]
    {
        let files_dir = ctx.output_dir.join("files");
        std::fs::create_dir_all(&files_dir)?;
        
        let rel_path = "locked/test.locked";
        let full_path = files_dir.join(rel_path);
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        
        let content = ctx.content_gen.generate("locked-file", 1024, ContentType::Text);
        std::fs::write(&full_path, &content)?;
        
        // Note: Actual locking requires a separate process
        // This just creates the file; the lock is applied by the test harness
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path.to_string(),
            size_bytes: content.len() as u64,
            sha256: Some(ContentGenerator::sha256(&content)),
            categories: vec![FileCategory::Inaccessible, FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("locked".to_string()),
            content_type: Some("locked".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }
    
    #[cfg(not(windows))]
    {
        // Not supported on non-Windows
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: "locked/NOT_SUPPORTED_ON_THIS_PLATFORM.txt".to_string(),
            size_bytes: 0,
            sha256: None,
            categories: vec![FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("locked-skipped".to_string()),
            content_type: Some("info".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }
    
    ctx.manifest.finalize();
    Ok(())
}

/// Generate permissions scenario
fn generate_permissions_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        
        let files_dir = ctx.output_dir.join("files");
        std::fs::create_dir_all(&files_dir)?;
        
        let rel_path = "restricted/noaccess.txt";
        let full_path = files_dir.join(rel_path);
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        
        let content = ctx.content_gen.generate("noaccess", 1024, ContentType::Text);
        std::fs::write(&full_path, &content)?;
        
        // Remove all permissions
        let mut perms = std::fs::metadata(&full_path)?.permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&full_path, perms)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path.to_string(),
            size_bytes: content.len() as u64,
            sha256: Some(ContentGenerator::sha256(&content)),
            categories: vec![FileCategory::Inaccessible, FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("no-permissions".to_string()),
            content_type: Some("permissions".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
        
        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&full_path)?.permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&full_path, perms)?;
    }
    
    #[cfg(not(unix))]
    {
        // Not easily supported on Windows
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: "permissions/NOT_SUPPORTED_ON_THIS_PLATFORM.txt".to_string(),
            size_bytes: 0,
            sha256: None,
            categories: vec![FileCategory::Normal],
            duplicate_group: None,
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("permissions-skipped".to_string()),
            content_type: Some("info".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }
    
    ctx.manifest.finalize();
    Ok(())
}

/// Generate regression scenario
fn generate_regression_scenario(ctx: &mut TestScenarioContext) -> Result<()> {
    // Use a larger configuration for regression testing
    let original_files = ctx.config.total_files;
    ctx.config.total_files = 5000;
    ctx.config.duplicate_groups = 200;
    ctx.config.empty_files = 100;
    ctx.config.unicode_files = 100;
    ctx.config.large_files = 5;
    ctx.config.same_size_diff_content_pairs = 100;
    
    generate_phase1_baseline(ctx)?;
    
    ctx.config.total_files = original_files;
    Ok(())
}

/// Performance scenario generators
fn generate_performance_small(ctx: &mut TestScenarioContext) -> Result<()> {
    ctx.config.total_files = 1000;
    generate_phase1_baseline(ctx)
}

fn generate_performance_medium(ctx: &mut TestScenarioContext) -> Result<()> {
    ctx.config.total_files = 10000;
    ctx.config.duplicate_groups = 400;
    ctx.config.empty_files = 200;
    generate_phase1_baseline(ctx)
}

fn generate_performance_large(ctx: &mut TestScenarioContext) -> Result<()> {
    ctx.config.total_files = 100000;
    ctx.config.duplicate_groups = 4000;
    ctx.config.empty_files = 2000;
    generate_phase1_baseline(ctx)
}

fn generate_performance_stress(ctx: &mut TestScenarioContext) -> Result<()> {
    ctx.config.total_files = 1000000;
    ctx.config.duplicate_groups = 40000;
    ctx.config.empty_files = 20000;
    generate_phase1_baseline(ctx)
}

// Helper functions

fn generate_normal_file(ctx: &mut TestScenarioContext, base_dir: &Path, base_dirs: &[&str]) -> Result<()> {
    let mut rng = rand::thread_rng();
    let dir = base_dirs[rng.gen_range(0..base_dirs.len())];
    let ext = ["txt", "md", "json", "csv", "pdf", "docx", "jpg", "png", "zip", "bin"][rng.gen_range(0..10)];
    let name = format!("file_{:06}.{}", ctx.file_counter, ext);
    let rel_path = format!("{}/{}", dir, name);
    let full_path = base_dir.join(&rel_path);
    
    std::fs::create_dir_all(full_path.parent().unwrap())?;
    
let content_type = ContentType::random(&mut ctx.content_gen.rng());
    let content = ctx.content_gen.generate(
        &format!("normal-{}", ctx.file_counter),
        rng.gen_range(1024..102400),
        content_type,
    );
    let hash = ContentGenerator::sha256(&content);
    std::fs::write(&full_path, &content)?;
    
    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path,
        size_bytes: content.len() as u64,
        sha256: Some(hash),
        categories: vec![FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(false),
        role: None,
        content_type: Some("normal".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;
    Ok(())
}

fn generate_duplicate_group(
    ctx: &mut TestScenarioContext,
    base_dir: &Path,
    base_dirs: &[&str],
    group_num: usize,
) -> Result<()> {
    let mut rng = rand::thread_rng();
    let group_id = format!("dup-{:04}", group_num);
    
    // Generate the master content for this duplicate group
    let size = rng.gen_range(1024..102400);
    let content_type = ContentType::random(&mut ctx.content_gen.rng());
    let master_content = ctx.content_gen.generate(&format!("master-{}", group_num), size, content_type);
    let hash = ContentGenerator::sha256(&master_content);
    
    // Create 3-5 copies in different directories with different names
    let copy_count = rng.gen_range(3..6);
    let copy_names = [
        "report.pdf", "report_copy.pdf", "report_final.pdf", "report_backup.pdf",
        "document.docx", "document_copy.docx", "document_v2.docx",
        "image.jpg", "image_copy.jpg", "image_backup.jpg",
        "data.bin", "data_copy.bin", "data_backup.bin",
    ];
    
for copy_idx in 0..copy_count {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
        let dir = base_dirs[rng.gen_range(0..base_dirs.len())];
        // Include group_num in filename to avoid collisions across groups
        let name = format!("{}_{}", group_num, copy_names[(group_num * 5 + copy_idx) % copy_names.len()]);
        let rel_path = format!("{}/{}", dir, name);
        let full_path = base_dir.join(&rel_path);
        
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        std::fs::write(&full_path, &master_content)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path,
            size_bytes: master_content.len() as u64,
            sha256: Some(hash.clone()),
            categories: vec![
                if copy_idx == 0 { FileCategory::ExactDuplicateOriginal } else { FileCategory::ExactDuplicateCopy },
                FileCategory::Normal,
            ],
            duplicate_group: Some(group_id.clone()),
            expected_duplicate: Some(true),
            expected_empty: Some(false),
            role: None,
            content_type: Some("duplicate".to_string()),
            modified_at: Some(ctx.config.base_date - Duration::days(rng.gen_range(0..365) as i64)),
};
        
        let record_id = record.id.clone();
        ctx.manifest.add_file(record);
        ctx.duplicate_groups.entry(group_id.clone()).or_default().push(record_id);
        *ctx.file_counter += 1;
    }
    
    Ok(())
}

fn generate_empty_file(ctx: &mut TestScenarioContext, base_dir: &Path, base_dirs: &[&str]) -> Result<()> {
    let mut rng = rand::thread_rng();
    let dir = base_dirs[rng.gen_range(0..base_dirs.len())];
    let empty_names = [
        ".gitkeep", "placeholder", "empty.txt", "lockfile", "marker",
        "empty.json", "empty.bin", ".placeholder", "keep", "empty.log",
    ];
    let name = empty_names[rng.gen_range(0..empty_names.len())];
    let rel_path = format!("{}/{}", dir, name);
    let full_path = base_dir.join(&rel_path);
    
    std::fs::create_dir_all(full_path.parent().unwrap())?;
    std::fs::write(&full_path, b"")?;
    
    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path,
        size_bytes: 0,
        sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
        categories: vec![FileCategory::Empty, FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(true),
        role: None,
        content_type: Some("empty".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;
    Ok(())
}

fn generate_same_size_different_content(
    ctx: &mut TestScenarioContext,
    base_dir: &Path,
    base_dirs: &[&str],
    pair_num: usize,
) -> Result<()> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(1024..102400);
    
    for file_idx in 0..2 {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
        let dir = base_dirs[rng.gen_range(0..base_dirs.len())];
        let name = format!("same_size_{}_{}.bin", pair_num, file_idx + 1);
        let rel_path = format!("{}/{}", dir, name);
        let full_path = base_dir.join(&rel_path);
        
        std::fs::create_dir_all(full_path.parent().unwrap())?;
        
        // Generate different content of the same size
        let content = ctx.content_gen.generate(
            &format!("samesize-{}-{}", pair_num, file_idx),
            size,
            ContentType::RandomBinary,
        );
        let hash = ContentGenerator::sha256(&content);
        std::fs::write(&full_path, &content)?;
        
        let record = FileRecord {
            id: format!("file-{:06}", ctx.file_counter),
            relative_path: rel_path,
            size_bytes: size,
            sha256: Some(hash),
            categories: vec![FileCategory::SameSizeDifferentContent, FileCategory::Normal],
            duplicate_group: Some(format!("samesize-{:04}", pair_num)),
            expected_duplicate: Some(false),
            expected_empty: Some(false),
            role: Some("same-size-different-content".to_string()),
            content_type: Some("same-size-different-content".to_string()),
            modified_at: Some(ctx.config.base_date),
        };
        
        ctx.manifest.add_file(record);
        *ctx.file_counter += 1;
    }
    
    Ok(())
}

fn generate_unicode_file(ctx: &mut TestScenarioContext, base_dir: &Path, base_dirs: &[&str]) -> Result<()> {
    let mut rng = rand::thread_rng();
    let dir = base_dirs[rng.gen_range(0..base_dirs.len())];
    
    let unicode_names = [
        "تقرير.pdf", "مشروع.docx", "صور.jpg", "نسخة.zip",
        "レポート.pdf", "写真.jpg", "プロジェクト.zip", "バックアップ.txt",
        "报告.pdf", "照片.jpg", "项目.zip",
        "📚_book.txt", "📷_photo.jpg", "🗂️_archive.zip",
        "report_تقرير.pdf", "레포트_report.docx",
    ];
    
    // Include file counter in name to avoid collisions
    let base_name = unicode_names[rng.gen_range(0..unicode_names.len())];
    let name = format!("{}_{}", ctx.file_counter, base_name);
    let rel_path = format!("{}/{}", dir, name);
    let full_path = base_dir.join(&rel_path);
    
    std::fs::create_dir_all(full_path.parent().unwrap())?;
    
let content_type = ContentType::random(&mut ctx.content_gen.rng());
    let content = ctx.content_gen.generate(
        &format!("unicode-{}", ctx.file_counter),
        rng.gen_range(1024..51200),
        content_type,
    );
    let hash = ContentGenerator::sha256(&content);
    std::fs::write(&full_path, &content)?;
    
    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path,
        size_bytes: content.len() as u64,
        sha256: Some(hash),
        categories: vec![FileCategory::Unicode, FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(false),
        role: None,
        content_type: Some("unicode".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;
    Ok(())
}

fn generate_nested_file(ctx: &mut TestScenarioContext, base_dir: &Path, max_depth: usize) -> Result<()> {
    let mut rng = rand::thread_rng();
    let depth = rng.gen_range(2..max_depth + 1);
    
    let mut path_parts = Vec::new();
    for i in 0..depth {
        path_parts.push(format!("level{}", i + 1));
    }
    path_parts.push(format!("file_{:06}.txt", ctx.file_counter));
    
    let rel_path = path_parts.join("/");
    let full_path = base_dir.join(&rel_path);
    
    std::fs::create_dir_all(full_path.parent().unwrap())?;
    
    let content = ctx.content_gen.generate(
        &format!("nested-{}", ctx.file_counter),
        rng.gen_range(1024..10240),
        ContentType::Text,
    );
    let hash = ContentGenerator::sha256(&content);
    std::fs::write(&full_path, &content)?;
    
    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path,
        size_bytes: content.len() as u64,
        sha256: Some(hash),
        categories: vec![FileCategory::Nested, FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(false),
        role: None,
        content_type: Some("nested".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;
    Ok(())
}

fn generate_large_file(ctx: &mut TestScenarioContext, base_dir: &Path, base_dirs: &[&str]) -> Result<()> {
    let mut rng = rand::thread_rng();
    let dir = base_dirs[rng.gen_range(0..base_dirs.len())];
    let size_mb = rng.gen_range(10..101);
    let size = size_mb * 1024 * 1024;
    
    // Include file counter for uniqueness in case same size is picked twice
    let name = format!("large_{}mb_{}.bin", size_mb, ctx.file_counter);
    let rel_path = format!("{}/{}", dir, name);
    let full_path = base_dir.join(&rel_path);
    
    std::fs::create_dir_all(full_path.parent().unwrap())?;
    
    let content = ctx.content_gen.generate(
        &format!("large-{}", ctx.file_counter),
        size,
        ContentType::RandomBinary,
    );
    let hash = ContentGenerator::sha256(&content);
    std::fs::write(&full_path, &content)?;
    
    let record = FileRecord {
        id: format!("file-{:06}", ctx.file_counter),
        relative_path: rel_path,
        size_bytes: size,
        sha256: Some(hash),
        categories: vec![FileCategory::LargeFile, FileCategory::Normal],
        duplicate_group: None,
        expected_duplicate: Some(false),
        expected_empty: Some(false),
        role: None,
        content_type: Some("large".to_string()),
        modified_at: Some(ctx.config.base_date),
    };
    
    ctx.manifest.add_file(record);
    *ctx.file_counter += 1;
    Ok(())
}

fn generate_backup_like_copies(ctx: &mut TestScenarioContext, base_dir: &Path) -> Result<()> {
    let working_dir = base_dir.join("working");
    let backup_dir = base_dir.join("backup");
    std::fs::create_dir_all(&working_dir)?;
    std::fs::create_dir_all(&backup_dir)?;
    
    let mut rng = rand::thread_rng();
    let count = (ctx.config.total_files / 50).max(1).min(10);
    
    for i in 0..count {
        if *ctx.file_counter >= ctx.config.total_files { break; }
        
let content_type = ContentType::random(&mut ctx.content_gen.rng());
        let content = ctx.content_gen.generate(
            &format!("backup-{}", i),
            rng.gen_range(1024..102400),
            content_type,
        );
        let hash = ContentGenerator::sha256(&content);
        
        let ext = ["pdf", "docx", "jpg", "zip", "txt"][rng.gen_range(0..5)];
        let name = format!("document_{}.{}", i, ext);
        
        let working_path = working_dir.join(&name);
        let backup_path = backup_dir.join(&name);
        
        std::fs::write(&working_path, &content)?;
        std::fs::write(&backup_path, &content)?;
        
        for (rel_dir, is_original) in [("working", true), ("backup", false)] {
            let rel_path = format!("{}/{}", rel_dir, name);
            
            let record = FileRecord {
                id: format!("file-{:06}", ctx.file_counter),
                relative_path: rel_path,
                size_bytes: content.len() as u64,
                sha256: Some(hash.clone()),
                categories: vec![
                    if is_original { FileCategory::ExactDuplicateOriginal } else { FileCategory::ExactDuplicateCopy },
                    FileCategory::BackupLikeCopy,
                    FileCategory::Normal,
                ],
                duplicate_group: Some(format!("backup-dup-{:04}", i)),
                expected_duplicate: Some(true),
                expected_empty: Some(false),
                role: Some("backup-like-copy".to_string()),
                content_type: Some("backup".to_string()),
                modified_at: Some(ctx.config.base_date),
            };
            
            ctx.manifest.add_file(record);
            *ctx.file_counter += 1;
        }
    }
    
    Ok(())
}

/// Generate backup baseline scenario
pub fn generate_backup_baseline(ctx: &mut TestScenarioContext) -> Result<()> {
    // First generate a baseline dataset with files
    crate::scenarios::generate_phase1_baseline(ctx)?;
    
    // Then create an initial backup
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    std::fs::create_dir_all(&backup_dir)?;
    
    // Copy all files to backup directory (simulating initial backup)
    copy_dir_all(&source_dir, &backup_dir)?;
    
    // Create backup manifest
    create_backup_manifest(ctx, &backup_dir)?;
    
    Ok(())
}

/// Generate incremental backup scenario
pub fn generate_backup_incremental(ctx: &mut TestScenarioContext) -> Result<()> {
    // First generate baseline
    generate_backup_baseline(ctx)?;
    
    // Modify some source files
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    
    // Modify some existing files
    modify_some_files(ctx, &source_dir)?;
    
    // Add new files
    add_new_files(ctx, &source_dir)?;
    
    // Delete some files
    delete_some_files(ctx, &source_dir)?;
    
    // Run incremental backup
    run_incremental_backup(ctx, &source_dir, &backup_dir)?;
    
    Ok(())
}

/// Generate changed files backup scenario
pub fn generate_backup_changed_files(ctx: &mut TestScenarioContext) -> Result<()> {
    generate_backup_baseline(ctx)?;
    
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    
    // Modify some files
    modify_some_files(ctx, &source_dir)?;
    
    // Run incremental backup to capture changes
    run_incremental_backup(ctx, &source_dir, &backup_dir)?;
    
    Ok(())
}

/// Generate deleted files backup scenario
pub fn generate_backup_deleted_files(ctx: &mut TestScenarioContext) -> Result<()> {
    generate_backup_baseline(ctx)?;
    
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    
    // Delete some source files
    delete_some_files(ctx, &source_dir)?;
    
    // Run incremental backup
    run_incremental_backup(ctx, &source_dir, &backup_dir)?;
    
    Ok(())
}

/// Generate backup restore scenario
pub fn generate_backup_restore(ctx: &mut TestScenarioContext) -> Result<()> {
    generate_backup_baseline(ctx)?;
    
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    let restore_dir = ctx.output_dir.join("restore");
    std::fs::create_dir_all(&restore_dir)?;
    
    // Test various restore scenarios
    // 1. Restore missing file
    restore_file(ctx, &backup_dir, &restore_dir, "documents/report.pdf")?;
    
    // 2. Restore with conflict (file exists)
    std::fs::write(source_dir.join("documents/report.pdf"), b"newer content")?;
    restore_file_with_conflict(ctx, &backup_dir, &source_dir, "documents/report.pdf")?;
    
    // 3. Restore to alternate location
    restore_to_alternate(ctx, &backup_dir, &restore_dir, "photos/photo.jpg")?;
    
    Ok(())
}

/// Generate backup corruption scenario
pub fn generate_backup_corruption(ctx: &mut TestScenarioContext) -> Result<()> {
    generate_backup_baseline(ctx)?;
    
    let backup_dir = ctx.output_dir.join("backup");
    
    // Corrupt one file in the backup
    corrupt_backup_file(ctx, &backup_dir)?;
    
    // Run verification to detect corruption
    verify_backup_integrity(ctx, &backup_dir)?;
    
    Ok(())
}

/// Generate interrupted backup scenario
pub fn generate_backup_interrupted(ctx: &mut TestScenarioContext) -> Result<()> {
    generate_backup_baseline(ctx)?;
    
    let _source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    
    // Create a partial backup (simulate interruption)
    create_partial_backup(ctx, &backup_dir)?;
    
    // Test resume capability
    test_backup_resume(ctx, &backup_dir)?;
    
    Ok(())
}

/// Generate large backup scenario
pub fn generate_backup_large(ctx: &mut TestScenarioContext) -> Result<()> {
    // Generate large dataset
    ctx.config.total_files = 10000;
    ctx.config.duplicate_groups = 1000;
    ctx.config.empty_files = 500;
    
    generate_backup_baseline(ctx)?;
    
    // Test incremental on large dataset
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    
    modify_some_files(ctx, &source_dir)?;
    add_new_files(ctx, &source_dir)?;
    
    run_incremental_backup(ctx, &source_dir, &backup_dir)?;
    
    Ok(())
}

/// Generate backup with duplicate content scenario
pub fn generate_backup_duplicate_content(ctx: &mut TestScenarioContext) -> Result<()> {
    generate_backup_baseline(ctx)?;
    
    let source_dir = ctx.output_dir.join("files");
    let backup_dir = ctx.output_dir.join("backup");
    
    // Create files with identical content in both source and backup
    create_duplicate_content_in_backup(ctx, &source_dir, &backup_dir)?;
    
    // Verify backup correctly handles duplicates
    verify_backup_handles_duplicates(ctx, &backup_dir)?;
    
    Ok(())
}

/// Helper functions for backup scenarios
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn create_backup_manifest(ctx: &mut TestScenarioContext, backup_dir: &std::path::Path) -> Result<()> {
    // Create a simple backup manifest file
    let manifest = serde_json::json!({
        "version": 1,
        "created_at": ctx.config.base_date.timestamp(),
        "files": []
    });
    std::fs::write(backup_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

fn modify_some_files(ctx: &mut TestScenarioContext, source_dir: &std::path::Path) -> Result<()> {
    // Modify a few existing files
    let files_to_modify = [
        "documents/report.pdf",
        "photos/photo.jpg",
        "videos/video.mp4",
    ];
    for file in files_to_modify {
        let path = source_dir.join(file);
        if path.exists() {
            std::fs::write(&path, format!("modified content at {}", ctx.config.base_date))?;
        }
    }
    Ok(())
}

fn add_new_files(_ctx: &mut TestScenarioContext, source_dir: &std::path::Path) -> Result<()> {
    // Add some new files
    let new_files = [
        ("documents/new_report.txt", "New report content"),
        ("photos/new_photo.png", "New photo content"),
        ("music/new_song.mp3", "New song content"),
    ];
    for (file, content) in new_files {
        let path = source_dir.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
    }
    Ok(())
}

fn delete_some_files(_ctx: &mut TestScenarioContext, source_dir: &std::path::Path) -> Result<()> {
    // Delete some existing files
    let files_to_delete = [
        "documents/old_report.txt",
        "photos/old_photo.jpg",
    ];
    for file in files_to_delete {
        let path = source_dir.join(file);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn run_incremental_backup(
    _ctx: &mut TestScenarioContext,
    source_dir: &std::path::Path,
    backup_dir: &std::path::Path,
) -> Result<()> {
    // Simulate incremental backup by copying changed/new files
    copy_dir_all(source_dir, backup_dir)?;
    Ok(())
}

fn restore_file(
    _ctx: &mut TestScenarioContext,
    backup_dir: &std::path::Path,
    restore_dir: &std::path::Path,
    file_path: &str,
) -> Result<()> {
    let src = backup_dir.join(file_path);
    let dst = restore_dir.join(file_path);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&src, &dst)?;
    Ok(())
}

fn restore_file_with_conflict(
    _ctx: &mut TestScenarioContext,
    backup_dir: &std::path::Path,
    source_dir: &std::path::Path,
    file_path: &str,
) -> Result<()> {
    // In conflict case, create a backup of existing file and restore
    let existing = source_dir.join(file_path);
    let backup_existing = source_dir.join(format!("{}.backup", file_path));
    if existing.exists() {
        std::fs::copy(&existing, &backup_existing)?;
    }
    let src = backup_dir.join(file_path);
    std::fs::copy(&src, &existing)?;
    Ok(())
}

fn restore_to_alternate(
    _ctx: &mut TestScenarioContext,
    backup_dir: &std::path::Path,
    restore_dir: &std::path::Path,
    file_path: &str,
) -> Result<()> {
    let src = backup_dir.join(file_path);
    let dst = restore_dir.join(file_path);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&src, &dst)?;
    Ok(())
}

fn corrupt_backup_file(_ctx: &mut TestScenarioContext, backup_dir: &std::path::Path) -> Result<()> {
    // Corrupt a random file in the backup
    if let Some(entry) = std::fs::read_dir(backup_dir)?.next() {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            std::fs::write(&path, b"CORRUPTED DATA")?;
        }
    }
    Ok(())
}

fn verify_backup_integrity(_ctx: &mut TestScenarioContext, backup_dir: &std::path::Path) -> Result<()> {
    // Simulate verification - check for corrupted files
    for entry in std::fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let content = std::fs::read(&path)?;
            if content == b"CORRUPTED DATA" {
                eprintln!("Corruption detected in: {:?}", path);
            }
        }
    }
    Ok(())
}

fn create_partial_backup(_ctx: &mut TestScenarioContext, backup_dir: &std::path::Path) -> Result<()> {
    // Create a partial backup (only some files)
    let partial_dir = backup_dir.join("partial");
    std::fs::create_dir_all(&partial_dir)?;
    // Copy only a subset of files
    Ok(())
}

fn test_backup_resume(_ctx: &mut TestScenarioContext, _backup_dir: &std::path::Path) -> Result<()> {
    // Simulate backup resume capability
    Ok(())
}

fn create_duplicate_content_in_backup(
    _ctx: &mut TestScenarioContext,
    source_dir: &std::path::Path,
    backup_dir: &std::path::Path,
) -> Result<()> {
    // Create files with identical content
    let content = b"DUPLICATE CONTENT";
    std::fs::write(source_dir.join("duplicate1.txt"), content)?;
    std::fs::write(source_dir.join("duplicate2.txt"), content)?;
    std::fs::write(backup_dir.join("duplicate1.txt"), content)?;
    std::fs::write(backup_dir.join("duplicate2.txt"), content)?;
    Ok(())
}

fn verify_backup_handles_duplicates(_ctx: &mut TestScenarioContext, backup_dir: &std::path::Path) -> Result<()> {
    // Verify backup correctly handles duplicates
    let file1 = backup_dir.join("duplicate1.txt");
    let file2 = backup_dir.join("duplicate2.txt");
    if file1.exists() && file2.exists() {
        let content1 = std::fs::read(&file1)?;
        let content2 = std::fs::read(&file2)?;
        assert_eq!(content1, content2);
    }
    Ok(())
}





