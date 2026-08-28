//! Test Data Generator for File Organizer
//!
//! Generates deterministic, realistic filesystem test datasets with ground truth manifests
//! for QA, integration testing, performance testing, and regression testing.

pub mod cli;
pub mod content;
pub mod generator;
pub mod manifest;
pub mod scenarios;
pub mod safety;
pub mod verify;

pub use cli::{Cli, Commands};
pub use content::ContentType;
pub use generator::{TestDataGenerator};
pub use manifest::{FileCategory, FileRecord, Manifest, ManifestSummary, ScenarioType};
pub use scenarios::{GenerationConfig, GenerationProgress, Scenario, ScenarioRegistry};
pub use safety::{is_dangerous_path, validate_output_path, SafeCleanup};
pub use verify::verify_dataset;