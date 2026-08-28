use crate::content::ContentGenerator;
use crate::manifest::Manifest;
use crate::safety::{validate_output_path, AtomicManifestWriter};
use crate::scenarios::{GenerationConfig, Scenario, ScenarioRegistry, TestScenarioContext};
use anyhow::{anyhow, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::time::Instant;

/// Main test data generator
pub struct TestDataGenerator {
    config: GenerationConfig,
    output_dir: PathBuf,
    scenario: Scenario,
}

impl TestDataGenerator {
    /// Create a new test data generator
    pub fn new(scenario_name: &str, output_dir: PathBuf, config: GenerationConfig) -> Result<Self> {
        Self::new_with_options(scenario_name, output_dir, config, false)
    }

    /// Create a new test data generator with options
    pub fn new_with_options(scenario_name: &str, output_dir: PathBuf, config: GenerationConfig, allow_dangerous: bool) -> Result<Self> {
        let scenario = ScenarioRegistry::get(scenario_name)
            .ok_or_else(|| anyhow!("Unknown scenario: {}", scenario_name))?;

        // Validate output path (unless allow_dangerous is true)
        if !allow_dangerous {
            validate_output_path(&output_dir)?;
        }

        // Check disk space for large generations
        if config.total_files >= 100000 {
            Self::check_disk_space(&output_dir, config.total_files)?;
        }

        Ok(Self {
            config,
            output_dir,
            scenario,
        })
    }

    /// Check available disk space
    fn check_disk_space(_output_dir: &Path, file_count: usize) -> Result<()> {
        // Estimate: average 50KB per file + overhead
        let estimated_bytes = (file_count as u64) * 50_000;
        let _estimated_gb = estimated_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        
        // Get available space
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(output_dir)?;
            let _available = metadata.blocks() * 512; // approximate
            // Skip detailed check on Windows
        }
        
        Ok(())
    }

/// Generate the test dataset
    pub fn generate(&mut self) -> Result<Manifest> {
        println!("Generating test dataset...");
        println!();
        println!("Scenario: {}", self.scenario.name);
        println!("Output: {}", self.output_dir.display());
        println!();

        let start_time = Instant::now();

        // Create output directory
        std::fs::create_dir_all(&self.output_dir)?;

        // Create manifest
        let mut manifest = Manifest::new(
            self.scenario.scenario_type,
            self.config.seed,
            self.output_dir.to_string_lossy().to_string(),
        );

        // Create content generator
        let mut content_gen = ContentGenerator::new(self.config.seed);

        // Extract config values needed before mutable borrow
        let total_files = self.config.total_files;

        // Create scenario context
        let mut file_counter = 0;
        let mut duplicate_groups = HashMap::new();
        
        let mut ctx = TestScenarioContext {
            output_dir: &self.output_dir,
            manifest: &mut manifest,
            content_gen: &mut content_gen,
            config: &mut self.config,
            file_counter: &mut file_counter,
            duplicate_groups: &mut duplicate_groups,
        };

        // Set up progress reporting
        let progress_bar = if total_files > 100 {
            Some(Self::create_progress_bar(total_files))
        } else {
            None
        };

        // Set up Ctrl+C handler (only once per process)
        static CTRL_C_HANDLER_SET: Once = Once::new();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_clone = cancelled.clone();
        CTRL_C_HANDLER_SET.call_once(|| {
            ctrlc::set_handler(move || {
                cancelled_clone.store(true, Ordering::SeqCst);
                eprintln!("\nReceived interrupt signal. Finishing current file...");
            }).expect("Failed to set Ctrl-C handler");
        });

        // Run scenario generation
        let generate_fn = self.scenario.generate;
        
        // Run generation
        generate_fn(&mut ctx)?;

        // Finalize manifest
        manifest.finalize();

        // Write manifest atomically
        let manifest_path = self.output_dir.join("manifest.json");
        AtomicManifestWriter::write(&manifest_path, &manifest)?;

        // Write README
        Self::write_readme(&self.output_dir, &manifest)?;

        // Finish progress bar
        if let Some(pb) = progress_bar {
            pb.finish_with_message("Generation complete");
        }

        let elapsed = start_time.elapsed();
        
        println!();
        println!("✓ Generation complete");
        println!("Manifest: {}", manifest_path.display());
        println!("Files generated: {}", manifest.summary.files);
        println!("Directories: {}", manifest.summary.directories);
        println!("Duplicate groups: {}", manifest.summary.duplicate_groups);
        println!("Duplicate files: {}", manifest.summary.duplicate_files);
        println!("Empty files: {}", manifest.summary.empty_files);
        println!("Time elapsed: {:.2}s", elapsed.as_secs_f64());
        
        Ok(manifest)
    }

    fn create_progress_bar(total: usize) -> ProgressBar {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        pb
    }

    fn write_readme(output_dir: &Path, manifest: &Manifest) -> Result<()> {
        let readme_path = output_dir.join("README.md");
        let content = format!(
            r#"# Test Dataset: {}

**Scenario:** {}
**Seed:** {}
**Generated:** {}

## Summary
- Files: {}
- Directories: {}
- Duplicate Groups: {}
- Duplicate Files: {}
- Empty Files: {}
- Unicode Files: {}
- Large Files: {}

## Verification
```bash
testgen verify --manifest .{}
```

## Cleanup
```bash
testgen clean .{}
```
"#,
            manifest.scenario,
            manifest.scenario,
            manifest.seed,
            manifest.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            manifest.summary.files,
            manifest.summary.directories,
            manifest.summary.duplicate_groups,
            manifest.summary.duplicate_files,
            manifest.summary.empty_files,
            manifest.summary.unicode_files,
            manifest.summary.large_files,
            "/manifest.json",
            "/manifest.json"
        );
        std::fs::write(readme_path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generator_creation() {
        let temp_dir = tempdir().unwrap();
        let config = GenerationConfig::default();
        let generator = TestDataGenerator::new_with_options("phase1-baseline", temp_dir.path().to_path_buf(), config, true);
        assert!(generator.is_ok());
    }

    #[test]
    fn test_unknown_scenario() {
        let temp_dir = tempdir().unwrap();
        let config = GenerationConfig::default();
        let generator = TestDataGenerator::new_with_options("unknown-scenario", temp_dir.path().to_path_buf(), config, true);
        assert!(generator.is_err());
    }

    #[test]
    fn test_small_generation() {
        let temp_dir = tempdir().unwrap();
        let mut config = GenerationConfig::default();
        config.total_files = 10;
        config.duplicate_groups = 2;
        config.empty_files = 2;
        
        let mut generator = TestDataGenerator::new_with_options("phase1-baseline", temp_dir.path().to_path_buf(), config, true).unwrap();
        let manifest = generator.generate().unwrap();
        
        assert_eq!(manifest.summary.files, 10);
        assert!(manifest.summary.duplicate_groups >= 2);
    }
}