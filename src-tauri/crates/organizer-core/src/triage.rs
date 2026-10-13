use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Classification category for user files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriageCategory {
    Documents,
    Images,
    AudioVideo,
    Archives,
    Installers,
    IncompleteDownloads,
    Code,
    AtomOrOther,
}

impl TriageCategory {
    pub fn default_folder_name(&self) -> &'static str {
        match self {
            Self::Documents => "Documents",
            Self::Images => "Images",
            Self::AudioVideo => "Media",
            Self::Archives => "Archives",
            Self::Installers => "Installers",
            Self::IncompleteDownloads => "Incomplete_Downloads",
            Self::Code => "Code",
            Self::AtomOrOther => "Other_Files",
        }
    }
}

/// A classified item found in a messy folder (e.g., Downloads, Documents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageItem {
    pub file_path: PathBuf,
    pub file_name: String,
    pub extension: String,
    pub file_size: u64,
    pub modified_timestamp: u64,
    pub category: TriageCategory,
    /// Indicates an installer (.exe, .msi) that has not been modified in > 30 days.
    pub is_stale_installer: bool,
    /// Indicates incomplete downloads (.crdownload, .part) that are abandoned.
    pub is_incomplete_download: bool,
    /// Proposed organized path.
    pub suggested_destination: PathBuf,
}

/// Proposed plan for organizing a folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriagePlan {
    pub source_directory: PathBuf,
    pub destination_base_directory: PathBuf,
    pub items: Vec<TriageItem>,
    pub category_counts: HashMap<String, usize>,
    pub total_size_bytes: u64,
    pub stale_installers_count: usize,
    pub incomplete_downloads_count: usize,
}

/// Options to customize the triage planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageOptions {
    /// Organize files into subfolders by category. Default: true.
    pub organize_by_category: bool,
    /// Group files by year/month inside category (e.g. Images/2026-03). Default: false.
    pub group_by_date: bool,
    /// Destination directory. If None, organizes inside a clean subdirectory in the source folder.
    pub custom_destination_dir: Option<PathBuf>,
}

impl Default for TriageOptions {
    fn default() -> Self {
        Self {
            organize_by_category: true,
            group_by_date: false,
            custom_destination_dir: None,
        }
    }
}

/// Categorizes a file extension into a TriageCategory.
pub fn categorize_extension(ext: &str) -> TriageCategory {
    let ext_lower = ext.to_lowercase();
    match ext_lower.as_str() {
        // Documents
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "rtf"
        | "odt" | "ods" | "csv" | "epub" | "pages" | "numbers" => TriageCategory::Documents,

        // Images
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "psd"
        | "ai" | "raw" | "cr2" | "nef" | "heic" | "avif" => TriageCategory::Images,

        // Audio & Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "mp3" | "wav" | "flac"
        | "aac" | "ogg" | "m4a" | "wma" => TriageCategory::AudioVideo,

        // Archives
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "dmg" => {
            TriageCategory::Archives
        }

        // Installers
        "exe" | "msi" | "pkg" | "deb" | "rpm" | "appimage" | "appx" => TriageCategory::Installers,

        // Incomplete / temp downloads
        "crdownload" | "part" | "download" | "tmp" => TriageCategory::IncompleteDownloads,

        // Code / development
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "c" | "cpp" | "h" | "hpp" | "java"
        | "go" | "html" | "css" | "scss" | "json" | "yaml" | "yml" | "toml" | "sql" | "sh"
        | "bat" | "ps1" => TriageCategory::Code,

        // Atom or unknown
        _ => TriageCategory::AtomOrOther,
    }
}

/// Scans a target folder and produces a TriagePlan with suggested non-destructive destination paths.
pub fn scan_and_plan_triage(
    source_dir: &Path,
    options: &TriageOptions,
) -> std::io::Result<TriagePlan> {
    let dest_base = options
        .custom_destination_dir
        .clone()
        .unwrap_or_else(|| source_dir.join("Organized_Files"));

    let mut items = Vec::new();
    let mut category_counts = HashMap::new();
    let mut total_size_bytes = 0u64;
    let mut stale_installers_count = 0;
    let mut incomplete_downloads_count = 0;

    let now_sec = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let entries = fs::read_dir(source_dir)?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        // We only triage loose files in the directory, not subfolders
        if !file_type.is_file() {
            continue;
        }

        let file_path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files or system files
        if file_name.starts_with('.') || file_name == "desktop.ini" || file_name == "Thumbs.db" {
            continue;
        }

        // Shield sensitive credential vaults and keys
        if crate::privacy::is_sensitive_path(&file_path) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let file_size = metadata.len();
        let modified_timestamp = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let ext = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let category = categorize_extension(&ext);

        let thirty_days_sec = 30 * 24 * 60 * 60;
        let is_stale_installer = category == TriageCategory::Installers
            && now_sec > modified_timestamp
            && (now_sec - modified_timestamp) > thirty_days_sec;

        let is_incomplete_download = category == TriageCategory::IncompleteDownloads;

        if is_stale_installer {
            stale_installers_count += 1;
        }
        if is_incomplete_download {
            incomplete_downloads_count += 1;
        }

        let category_folder = category.default_folder_name();
        let suggested_destination = if options.organize_by_category {
            dest_base.join(category_folder).join(&file_name)
        } else {
            dest_base.join(&file_name)
        };

        let cat_key = format!("{:?}", category);
        *category_counts.entry(cat_key).or_insert(0) += 1;
        total_size_bytes += file_size;

        items.push(TriageItem {
            file_path,
            file_name,
            extension: ext,
            file_size,
            modified_timestamp,
            category,
            is_stale_installer,
            is_incomplete_download,
            suggested_destination,
        });
    }

    Ok(TriagePlan {
        source_directory: source_dir.to_path_buf(),
        destination_base_directory: dest_base,
        items,
        category_counts,
        total_size_bytes,
        stale_installers_count,
        incomplete_downloads_count,
    })
}

/// Generates a unique destination path avoiding collisions by adding suffix `(1)`, `(2)`, etc.
pub fn resolve_collision_path(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }

    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = target.extension().and_then(|s| s.to_str());

    let mut counter = 1;
    loop {
        let new_name = match ext {
            Some(e) => format!("{} ({}).{}", stem, counter, e),
            None => format!("{} ({})", stem, counter),
        };
        let candidate = parent.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}
