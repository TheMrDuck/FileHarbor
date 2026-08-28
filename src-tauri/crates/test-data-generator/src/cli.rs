use crate::generator::TestDataGenerator;
use crate::manifest::ScenarioType;
use crate::scenarios::GenerationConfig;
use crate::verify::verify_dataset;
use crate::safety::SafeCleanup;
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Test Data Generator for FileHarbor
#[derive(Parser)]
#[command(name = "testgen")]
#[command(about = "Generate deterministic test datasets for FileHarbor QA testing")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a test dataset
    Generate {
        /// Scenario to generate
        #[arg(short, long)]
        scenario: String,
        
        /// Output directory (required)
        #[arg(short, long)]
        output: PathBuf,
        
        /// Number of files to generate
        #[arg(long, default_value_t = 2500)]
        files: usize,
        
        /// Random seed for deterministic generation
        #[arg(long, default_value_t = 12345)]
        seed: u64,
        
        /// Number of duplicate groups
        #[arg(long, default_value_t = 100)]
        duplicate_groups: usize,
        
        /// Number of empty files
        #[arg(long, default_value_t = 50)]
        empty_files: usize,
        
        /// Number of Unicode files
        #[arg(long, default_value_t = 50)]
        unicode_files: usize,
        
        /// Number of large files
        #[arg(long, default_value_t = 5)]
        large_files: usize,
        
        /// Maximum nested directory depth
        #[arg(long, default_value_t = 5)]
        nested_depth: usize,
        
        /// Number of same-size/different-content pairs
        #[arg(long, default_value_t = 50)]
        same_size_diff_content: usize,
        
        /// Enable symlink generation
        #[arg(long)]
        symlinks: bool,
        
        /// Enable broken symlink generation
        #[arg(long)]
        broken_symlinks: bool,
        
        /// Skip confirmation for large datasets
        #[arg(long)]
        yes: bool,
    },
    
    /// List all available scenarios
    ListScenarios,
    
    /// Describe a scenario in detail
    Describe {
        /// Scenario name
        scenario: String,
    },
    
    /// Verify a generated dataset against its manifest
    Verify {
        /// Path to manifest.json
        #[arg(short, long)]
        manifest: PathBuf,
    },
    
    /// Clean up a generated test dataset
    Clean {
        /// Path to dataset directory
        path: PathBuf,
        
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
    
    /// Generate a comprehensive regression dataset
    Regression {
        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
        
        /// Number of files (default: 5000)
        #[arg(long, default_value_t = 5000)]
        files: usize,
        
        /// Random seed
        #[arg(long, default_value_t = 12345)]
        seed: u64,
        
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
}

impl Cli {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Commands::Generate { 
                scenario, output, files, seed, duplicate_groups, empty_files, 
                unicode_files, large_files, nested_depth, same_size_diff_content,
                symlinks: _, broken_symlinks: _, yes 
            } => {
                let config = GenerationConfig {
                    total_files: *files,
                    seed: *seed,
                    duplicate_groups: *duplicate_groups,
                    empty_files: *empty_files,
                    unicode_files: *unicode_files,
                    large_files: *large_files,
                    nested_depth: *nested_depth,
                    same_size_diff_content_pairs: *same_size_diff_content,
                    ..Default::default()
                };
                
                // Check for large dataset confirmation
                if *files >= 100000 && !yes {
                    Self::confirm_large_dataset(*files)?;
                }
                
                let mut generator = TestDataGenerator::new_with_options(scenario, output.clone(), config, false)?;
                generator.generate()?;
            }
            
            Commands::ListScenarios => {
                Self::list_scenarios();
            }
            
            Commands::Describe { scenario } => {
                Self::describe_scenario(scenario)?;
            }
            
            Commands::Verify { manifest } => {
                let result = verify_dataset(manifest)?;
                if result.missing_files.is_empty() 
                    && result.unexpected_files.is_empty() 
                    && result.size_mismatches.is_empty() 
                    && result.hash_mismatches.is_empty() {
                    std::process::exit(0);
                } else {
                    std::process::exit(1);
                }
            }
            
            Commands::Clean { path, yes } => {
                SafeCleanup::clean(path, *yes)?;
            }
            
            Commands::Regression { output, files, seed, yes } => {
                if *files >= 100000 && !yes {
                    Self::confirm_large_dataset(*files)?;
                }
                
                let config = GenerationConfig {
                    total_files: *files,
                    seed: *seed,
                    duplicate_groups: 200,
                    empty_files: 100,
                    unicode_files: 100,
                    large_files: 5,
                    same_size_diff_content_pairs: 100,
                    ..Default::default()
                };
                
                let mut generator = TestDataGenerator::new_with_options("regression", output.clone(), config, false)?;
                generator.generate()?;
            }
        }
        
        Ok(())
    }
    
    fn confirm_large_dataset(file_count: usize) -> Result<()> {
        let estimated_gb = (file_count as f64 * 50000.0) / (1024.0 * 1024.0 * 1024.0);
        let estimated_entries = (file_count as f64 * 1.125) as usize;
        
        println!("⚠️  WARNING");
        println!();
        println!("You requested: {} files", file_count);
        println!("Estimated filesystem entries: {}", estimated_entries);
        println!("Estimated disk usage: {:.2} GB", estimated_gb);
        println!();
        print!("Continue? [y/N] ");
        
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            anyhow::bail!("Generation cancelled by user");
        }
        
        Ok(())
    }
    
    fn list_scenarios() {
        println!("Available Scenarios:");
        println!();
        
        for (name, scenario_type, description) in crate::scenarios::ScenarioRegistry::list() {
            println!("  {}", name);
            println!("    Type: {}", scenario_type);
            println!("    Description: {}", description);
            println!();
        }
    }
    
    fn describe_scenario(name: &str) -> Result<()> {
        let scenario = crate::scenarios::ScenarioRegistry::get(name)
            .ok_or_else(|| anyhow!("Unknown scenario: {}", name))?;
        
        println!("{}", scenario.name);
        println!();
        println!("Purpose:");
        println!("{}", scenario.description);
        println!();
        println!("Default files: {}", scenario.default_files);
        
        // Show expected counts based on scenario type
        match scenario.scenario_type {
            ScenarioType::Phase1Baseline => {
                println!();
                println!("Expected duplicate groups: 100");
                println!("Expected empty files: 50");
            }
            ScenarioType::Duplicates => {
                println!();
                println!("Expected duplicate groups: ~250");
            }
            ScenarioType::EmptyFiles => {
                println!();
                println!("Expected empty files: all");
            }
            ScenarioType::LargeFiles => {
                println!();
                println!("Expected large files: 3 (1 unique, 1 duplicate, 1 same-size diff)");
            }
            ScenarioType::Regression => {
                println!();
                println!("Expected duplicate groups: 200");
                println!("Expected empty files: 100");
                println!("Expected unicode files: 100");
                println!("Expected large files: 5");
            }
            _ => {}
        }
        
        Ok(())
    }
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ScenarioArg {
    Phase1Baseline,
    Duplicates,
    EmptyFiles,
    Unicode,
    LargeFiles,
    PerformanceSmall,
    PerformanceMedium,
    PerformanceLarge,
    PerformanceStress,
    RecoveredHdd,
    Unstable,
    MissingFiles,
    Symlinks,
    LockedFiles,
    Permissions,
    Regression,
}

impl From<ScenarioArg> for ScenarioType {
    fn from(arg: ScenarioArg) -> Self {
        match arg {
            ScenarioArg::Phase1Baseline => ScenarioType::Phase1Baseline,
            ScenarioArg::Duplicates => ScenarioType::Duplicates,
            ScenarioArg::EmptyFiles => ScenarioType::EmptyFiles,
            ScenarioArg::Unicode => ScenarioType::Unicode,
            ScenarioArg::LargeFiles => ScenarioType::LargeFiles,
            ScenarioArg::PerformanceSmall => ScenarioType::PerformanceSmall,
            ScenarioArg::PerformanceMedium => ScenarioType::PerformanceMedium,
            ScenarioArg::PerformanceLarge => ScenarioType::PerformanceLarge,
            ScenarioArg::PerformanceStress => ScenarioType::PerformanceStress,
            ScenarioArg::RecoveredHdd => ScenarioType::RecoveredHdd,
            ScenarioArg::Unstable => ScenarioType::Unstable,
            ScenarioArg::MissingFiles => ScenarioType::MissingFiles,
            ScenarioArg::Symlinks => ScenarioType::Symlinks,
            ScenarioArg::LockedFiles => ScenarioType::LockedFiles,
            ScenarioArg::Permissions => ScenarioType::Permissions,
            ScenarioArg::Regression => ScenarioType::Regression,
        }
    }
}