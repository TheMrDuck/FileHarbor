//! Empty file safety and structural risk classification.
//!
//! Evaluates 0-byte files to distinguish critical application/game architecture
//! (such as Minecraft world region chunks, mod configs, and Git/Python sentinels)
//! from harmless abandoned junk.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Safety risk level when contemplating deleting an empty file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyFileRisk {
    /// Essential structural file (Minecraft chunks, mod config templates, git markers, locks).
    /// Deleting this is likely to crash or corrupt applications, games, or projects.
    High,
    /// User project document (e.g. empty notes or templates).
    Medium,
    /// True junk (e.g. abandoned `New Text Document.txt`, temporary leftovers). Safe to clean.
    Low,
}

impl EmptyFileRisk {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmptyFileRisk::High => "high",
            EmptyFileRisk::Medium => "medium",
            EmptyFileRisk::Low => "low",
        }
    }
}

/// Detailed safety evaluation for an empty file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmptyFileAnalysis {
    pub risk: EmptyFileRisk,
    pub badge_label: String,
    pub reason: String,
    pub category: String,
}

const HIGH_RISK_EXTENSIONS: &[&str] = &[
    "mca", // Minecraft Anvil region file
    "mcr", // Minecraft Region file
    "dat", // Minecraft world data / player data
    "schem",
    "schematic",
    "nbt",
    "snbt",
    "lock", // Process locks
    "pid",
    "sentinel",
    "lck",
    "marker",
    "cfg", // Config templates
    "ini",
    "properties",
    "toml",
    "yaml",
    "yml",
    "conf",
];

const CODE_SENTINEL_FILENAMES: &[&str] = &[
    "__init__.py",
    "__main__.py",
    ".gitkeep",
    ".keep",
    ".npmignore",
    ".dockerignore",
    ".editorconfig",
    ".gitignore",
];

/// Classify an empty file's structural importance and risk profile.
pub fn classify_empty_file(path_str: &str, file_name: &str) -> EmptyFileAnalysis {
    let path_lower = path_str.to_lowercase();
    let name_lower = file_name.to_lowercase();
    let path_obj = Path::new(path_str);
    let ext = path_obj
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. Code & VCS Sentinels
    for sentinel in CODE_SENTINEL_FILENAMES {
        if name_lower == *sentinel {
            return EmptyFileAnalysis {
                risk: EmptyFileRisk::High,
                badge_label: "Code / VCS Marker".to_string(),
                reason: format!("Critical package marker ('{}') defining module imports or git directory structure.", file_name),
                category: "vcs_code_sentinel".to_string(),
            };
        }
    }

    // 2. Minecraft / Game Region and Chunk files (e.g. r.-1.-2.mca)
    if ext == "mca" || ext == "mcr" || ext == "dat" || ext == "schem" || ext == "nbt" {
        return EmptyFileAnalysis {
            risk: EmptyFileRisk::High,
            badge_label: "Game Save / Region".to_string(),
            reason: "Minecraft world chunk/region storage. Deleting this will corrupt world saves or erase player builds.".to_string(),
            category: "game_save_structure".to_string(),
        };
    }

    // 3. Mod & App Configuration directories (e.g. \config\villagernames\customnames.txt)
    if path_lower.contains("/config/") || path_lower.contains("\\config\\")
        || path_lower.contains("/world/") || path_lower.contains("\\world\\")
        || path_lower.contains("/poi/") || path_lower.contains("\\poi\\")
        || path_lower.contains("/region/") || path_lower.contains("\\region\\")
        || path_lower.contains("/mods/") || path_lower.contains("\\mods\\")
        || path_lower.contains("/.git/") || path_lower.contains("\\.git\\")
        || path_lower.contains("/node_modules/") || path_lower.contains("\\node_modules\\")
        || path_lower.contains("/appdata/") || path_lower.contains("\\appdata\\")
    {
        let is_config = path_lower.contains("config");
        return EmptyFileAnalysis {
            risk: EmptyFileRisk::High,
            badge_label: if is_config { "Mod/App Config".to_string() } else { "App/Game Structure".to_string() },
            reason: if is_config {
                format!("Configuration file template for an installed mod or program. Required on startup: '{}'", file_name)
            } else {
                format!("Internal application or database structure file inside '{}'", path_str)
            },
            category: if is_config { "config_template".to_string() } else { "app_internal_structure".to_string() },
        };
    }

    // 4. High-risk extensions (configs, locks, markers)
    if HIGH_RISK_EXTENSIONS.contains(&ext.as_str()) {
        return EmptyFileAnalysis {
            risk: EmptyFileRisk::High,
            badge_label: "System Sentinel / Config".to_string(),
            reason: format!("Configuration or state lock file (.{}). Deleting may reset application preferences.", ext),
            category: "system_sentinel".to_string(),
        };
    }

    // 5. Low Risk Junk checks
    let is_junk_name = name_lower.starts_with("new text document")
        || name_lower.starts_with("new document")
        || name_lower.starts_with("untitled")
        || name_lower.starts_with("temp")
        || name_lower.starts_with("tmp")
        || ext == "tmp"
        || ext == "log"
        || ext == "bak";

    let is_in_user_drop_zone = path_lower.contains("/downloads/") || path_lower.contains("\\downloads\\")
        || path_lower.contains("/desktop/") || path_lower.contains("\\desktop\\")
        || path_lower.contains("/temp/") || path_lower.contains("\\temp\\");

    if is_junk_name || is_in_user_drop_zone {
        return EmptyFileAnalysis {
            risk: EmptyFileRisk::Low,
            badge_label: "Safe Junk".to_string(),
            reason: "Abandoned temporary or draft placeholder. Safe to clean.".to_string(),
            category: "safe_junk".to_string(),
        };
    }

    // 6. Default to Medium Risk (User Document)
    EmptyFileAnalysis {
        risk: EmptyFileRisk::Medium,
        badge_label: "User Document".to_string(),
        reason: "Empty user document. Verify whether this is an intentional placeholder.".to_string(),
        category: "user_document".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minecraft_region_chunk_is_high_risk() {
        let analysis = classify_empty_file("D:\\Backup\\BMC5\\world\\region\\r.-1.-2.mca", "r.-1.-2.mca");
        assert_eq!(analysis.risk, EmptyFileRisk::High);
        assert_eq!(analysis.badge_label, "Game Save / Region");
    }

    #[test]
    fn test_mod_config_template_is_high_risk() {
        let analysis = classify_empty_file(
            "D:\\Backup\\BMC5Updated\\config\\villagernames\\customnames.txt",
            "customnames.txt",
        );
        assert_eq!(analysis.risk, EmptyFileRisk::High);
        assert_eq!(analysis.badge_label, "Mod/App Config");
    }

    #[test]
    fn test_python_init_is_high_risk() {
        let analysis = classify_empty_file("C:\\Projects\\my_app\\__init__.py", "__init__.py");
        assert_eq!(analysis.risk, EmptyFileRisk::High);
        assert_eq!(analysis.badge_label, "Code / VCS Marker");
    }

    #[test]
    fn test_new_text_document_is_low_risk() {
        let analysis = classify_empty_file("C:\\Users\\user\\Desktop\\New Text Document.txt", "New Text Document.txt");
        assert_eq!(analysis.risk, EmptyFileRisk::Low);
        assert_eq!(analysis.badge_label, "Safe Junk");
    }
}
