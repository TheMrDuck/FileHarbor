//! Project, server, and repository context detection.
//!
//! Detects whether files belong to distinct project or application environments
//! (such as Minecraft game servers, Git repositories, Node.js packages, Python venvs, etc.)
//! to prevent cross-project duplicate deletions from breaking independent software instances.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Identified project or environment category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    MinecraftServer,
    GameInstance,
    GitRepository,
    NodePackage,
    RustWorkspace,
    PythonEnvironment,
    GenericProject,
}

impl ProjectType {
    pub fn display_label(&self) -> &'static str {
        match self {
            ProjectType::MinecraftServer => "Game Server",
            ProjectType::GameInstance => "Game Save/Instance",
            ProjectType::GitRepository => "Git Repository",
            ProjectType::NodePackage => "Node.js Project",
            ProjectType::RustWorkspace => "Rust Project",
            ProjectType::PythonEnvironment => "Python Project",
            ProjectType::GenericProject => "Project Environment",
        }
    }
}

/// Metadata describing the project root enclosing a given file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectContext {
    pub project_type: ProjectType,
    pub root_path: String,
    pub root_name: String,
    pub is_vcs_internal: bool,
    pub is_runtime_binary: bool,
}

/// Known filenames that establish a directory as a specific project/environment root.
const MINECRAFT_SERVER_MARKERS: &[&str] = &[
    "server.properties",
    "eula.txt",
    "ops.json",
    "banned-players.json",
    "banned-ips.json",
    "spigot.yml",
    "paper.yml",
    "bukkit.yml",
    "purpur.yml",
];

const GAME_INSTANCE_MARKERS: &[&str] = &[
    "instance.cfg",
    "mmc-pack.json",
    "minecraftinstance.json",
];

/// Known runtime binary / engine filenames that should not be stripped across servers.
const RUNTIME_BINARY_NAMES: &[&str] = &[
    "server.jar",
    "paper.jar",
    "spigot.jar",
    "forge.jar",
    "fabric-server-launch.jar",
    "node.exe",
    "python.exe",
    "cargo.exe",
    "rustc.exe",
];

/// Determine the enclosing project root and context for a given file path.
pub fn detect_project_context(file_path: &Path) -> Option<ProjectContext> {
    let path_str = file_path.to_string_lossy().to_lowercase();

    // Check if inside internal VCS directories (.git, .svn, .hg)
    let is_vcs_internal = path_str.contains("/.git/")
        || path_str.contains("\\.git\\")
        || path_str.contains("/.svn/")
        || path_str.contains("\\.svn\\");

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_runtime_binary = RUNTIME_BINARY_NAMES.contains(&file_name.as_str())
        || file_name.ends_with(".jar") && file_name.contains("server");

    // Walk up ancestor directories (up to 5 levels) to identify project root
    let mut current = file_path.parent();
    let mut levels = 0;

    while let Some(dir) = current {
        if levels > 5 {
            break;
        }

        // 1. Filesystem existence checks if directory exists on disk
        if dir.exists() {
            // Check Minecraft server markers
            for marker in MINECRAFT_SERVER_MARKERS {
                if dir.join(marker).exists() {
                    return Some(ProjectContext {
                        project_type: ProjectType::MinecraftServer,
                        root_path: dir.to_string_lossy().to_string(),
                        root_name: dir
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Server".to_string()),
                        is_vcs_internal,
                        is_runtime_binary,
                    });
                }
            }

            // Check Game instance markers
            for marker in GAME_INSTANCE_MARKERS {
                if dir.join(marker).exists() {
                    return Some(ProjectContext {
                        project_type: ProjectType::GameInstance,
                        root_path: dir.to_string_lossy().to_string(),
                        root_name: dir
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Game".to_string()),
                        is_vcs_internal,
                        is_runtime_binary,
                    });
                }
            }

            // Check Git repository root
            if dir.join(".git").exists() {
                return Some(ProjectContext {
                    project_type: ProjectType::GitRepository,
                    root_path: dir.to_string_lossy().to_string(),
                    root_name: dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Repo".to_string()),
                    is_vcs_internal,
                    is_runtime_binary,
                });
            }

            // Check Node package
            if dir.join("package.json").exists() {
                return Some(ProjectContext {
                    project_type: ProjectType::NodePackage,
                    root_path: dir.to_string_lossy().to_string(),
                    root_name: dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "NodeProject".to_string()),
                    is_vcs_internal,
                    is_runtime_binary,
                });
            }

            // Check Rust workspace / crate
            if dir.join("Cargo.toml").exists() {
                return Some(ProjectContext {
                    project_type: ProjectType::RustWorkspace,
                    root_path: dir.to_string_lossy().to_string(),
                    root_name: dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "RustCrate".to_string()),
                    is_vcs_internal,
                    is_runtime_binary,
                });
            }

            // Check Python project
            if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
                return Some(ProjectContext {
                    project_type: ProjectType::PythonEnvironment,
                    root_path: dir.to_string_lossy().to_string(),
                    root_name: dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "PythonApp".to_string()),
                    is_vcs_internal,
                    is_runtime_binary,
                });
            }
        }

        // 2. Path heuristic fallback (works even if drive is offline or permissions restricted)
        let dir_name_lower = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if dir_name_lower.starts_with("server ") || dir_name_lower.starts_with("server_") || dir_name_lower == "server" {
            return Some(ProjectContext {
                project_type: ProjectType::MinecraftServer,
                root_path: dir.to_string_lossy().to_string(),
                root_name: dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Server".to_string()),
                is_vcs_internal,
                is_runtime_binary,
            });
        }

        current = dir.parent();
        levels += 1;
    }

    if is_vcs_internal {
        return Some(ProjectContext {
            project_type: ProjectType::GitRepository,
            root_path: file_path.to_string_lossy().to_string(),
            root_name: "GitInternal".to_string(),
            is_vcs_internal: true,
            is_runtime_binary,
        });
    }

    None
}

/// Returns true if two paths belong to distinct, independent project/server roots.
pub fn are_cross_project(path_a: &Path, path_b: &Path) -> bool {
    let ctx_a = detect_project_context(path_a);
    let ctx_b = detect_project_context(path_b);

    match (ctx_a, ctx_b) {
        (Some(a), Some(b)) => {
            // Distinct roots mean distinct independent environments!
            a.root_path.to_lowercase() != b.root_path.to_lowercase()
        }
        (Some(a), None) => a.is_runtime_binary,
        (None, Some(b)) => b.is_runtime_binary,
        (None, None) => {
            // Check if top-level parent directories differ while having server-like or project-like names
            let parent_a = path_a.parent().map(|p| p.to_string_lossy().to_lowercase());
            let parent_b = path_b.parent().map(|p| p.to_string_lossy().to_lowercase());
            if let (Some(pa), Some(pb)) = (parent_a, parent_b) {
                if pa != pb && (pa.contains("server") || pb.contains("server")) {
                    return true;
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_server_detection() {
        let path1 = Path::new("D:\\Backup\\Server 1.20.4\\server.jar");
        let path2 = Path::new("D:\\Backup\\Server 1.21.4 with saeed\\server.jar");

        assert!(are_cross_project(path1, path2));
    }

    #[test]
    fn test_vcs_internal_flag() {
        let path = Path::new("D:\\Backup\\Server 26.2\\.git\\lfs\\objects\\5b\\b6\\blob");
        let ctx = detect_project_context(path);
        assert!(ctx.is_some());
        assert!(ctx.unwrap().is_vcs_internal);
    }
}
