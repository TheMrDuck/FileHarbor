//! Privacy and security shielding primitives.
//!
//! Enforces zero-knowledge local isolation by automatically shielding sensitive
//! credential stores, private keys, password vaults, and crypto wallets from
//! background scans, duplicate matching, and automated triage.

use std::path::Path;

/// Standard directory names containing high-risk credentials or private data.
pub const SENSITIVE_DIR_NAMES: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".kube",
    ".docker",
    ".1password",
    ".bitwarden",
    ".op",
    ".password-store",
    ".bitcoin",
    ".ethereum",
    ".solana",
];

/// Standard file extensions representing encryption keys, password vaults, or certificates.
pub const SENSITIVE_EXTENSIONS: &[&str] = &[
    "kdbx", // KeePass database
    "key",
    "pem",
    "pfx",
    "p12",
    "pkcs12",
    "asc",
    "sig",
];

/// Exact file names containing sensitive secrets or authentication keys.
pub const SENSITIVE_FILE_NAMES: &[&str] = &[
    "id_rsa",
    "id_rsa.pub",
    "id_ed25519",
    "id_ed25519.pub",
    "id_ecdsa",
    "id_dsa",
    "known_hosts",
    "authorized_keys",
    "credentials",
    ".env",
    ".env.local",
    ".env.production",
];

/// Returns true if a given path belongs to a shielded sensitive location.
pub fn is_sensitive_path(path: &Path) -> bool {
    // Check if any component is a sensitive directory
    for comp in path.components() {
        let comp_str = comp.as_os_str().to_string_lossy().to_lowercase();
        for &sensitive_dir in SENSITIVE_DIR_NAMES {
            if comp_str == sensitive_dir {
                return true;
            }
        }
    }

    // Check file name and extension
    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
        let lower_name = file_name.to_lowercase();
        for &sensitive_file in SENSITIVE_FILE_NAMES {
            if lower_name == sensitive_file {
                return true;
            }
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let lower_ext = ext.to_lowercase();
            for &sensitive_ext in SENSITIVE_EXTENSIONS {
                if lower_ext == sensitive_ext {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sensitive_directories() {
        assert!(is_sensitive_path(&PathBuf::from("/home/user/.ssh/id_rsa")));
        assert!(is_sensitive_path(&PathBuf::from("C:\\Users\\user\\.aws\\credentials")));
        assert!(is_sensitive_path(&PathBuf::from("/home/user/.gnupg/secring.gpg")));
        assert!(is_sensitive_path(&PathBuf::from("/home/user/.kube/config")));
        assert!(is_sensitive_path(&PathBuf::from("D:\\Wallets\\.bitcoin\\wallet.dat")));
    }

    #[test]
    fn test_sensitive_files() {
        assert!(is_sensitive_path(&PathBuf::from("/downloads/backup.kdbx")));
        assert!(is_sensitive_path(&PathBuf::from("C:\\project\\.env")));
        assert!(is_sensitive_path(&PathBuf::from("C:\\project\\.env.local")));
        assert!(is_sensitive_path(&PathBuf::from("/certs/server.pem")));
        assert!(is_sensitive_path(&PathBuf::from("/keys/my_cert.pfx")));
    }

    #[test]
    fn test_normal_files_not_sensitive() {
        assert!(!is_sensitive_path(&PathBuf::from("/home/user/Documents/report.pdf")));
        assert!(!is_sensitive_path(&PathBuf::from("C:\\Photos\\IMG_001.JPG")));
        assert!(!is_sensitive_path(&PathBuf::from("C:\\Downloads\\installer.exe")));
        assert!(!is_sensitive_path(&PathBuf::from("/data/audio.mp3")));
    }
}
