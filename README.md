# FileHarbor

[![Latest Release](https://img.shields.io/github/v/release/TheMrDuck/FileHarbor?color=brightgreen&label=latest%20release)](https://github.com/TheMrDuck/FileHarbor/releases/latest)
[![Download Windows EXE](https://img.shields.io/badge/Windows-Download%20.exe-blue?logo=windows)](https://github.com/TheMrDuck/FileHarbor/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-61DAFB.svg)](https://react.dev/)

> [!TIP]
> **Looking to install FileHarbor on Windows?**
> You do not need to compile from source. Download the latest installer directly from [**GitHub Releases**](https://github.com/TheMrDuck/FileHarbor/releases/latest):
> - [**FileHarbor_0.2.0_x64-setup.exe**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64-setup.exe) (Recommended: Standard Windows Setup Wizard)
> - [**FileHarbor_0.2.0_x64_portable.exe**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64_portable.exe) (Portable: Run immediately without installation)
> - [**FileHarbor_0.2.0_x64_en-US.msi**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64_en-US.msi) (MSI package)

A safe, verifiable desktop file organization, deduplication, and backup recovery engine for Windows, macOS, and Linux.

FileHarbor operates on **verifiable filesystem facts**. It never guesses or silently alters data. All destructive actions require explicit user confirmation, and deletions always route through an application-managed quarantine with complete audit logs.

---

## Key Capabilities

### 1. Exact Duplicate & Empty File Detection
- **Streaming SHA-256**: High-performance chunked hashing capable of processing large multi-gigabyte files safely without memory spikes.
- **Rayon Parallelism**: Multi-threaded candidate hashing with fast size pre-filtering.
- **Smart Keeping Recommendations**: Heuristics for shortest path, newest/oldest mtime, while never deleting without explicit user choice.
- **Separate Empty File Audits**: Separates 0-byte placeholders from content duplicates (empty != useless).
- **Persistent Hash Cache**: Cached hashes keyed by `(path, size, mtime)` to accelerate repeated scans.

### 2. Quarantine & Reversible Operations
- **Atomic Quarantine**: Moves flagged files safely into an isolated quarantine repository.
- **Non-Destructive Restore**: Guarantees zero overwrites during restoration, automatically managing collision naming if the original destination path already exists.
- **Complete Operation History**: Every move, restore, and purge action is logged to SQLite with timestamps and original filesystem paths.

### 3. Verified Backup Engine
- **Named Backup Sets**: Define sources, destinations, and exclusion rules.
- **Snapshot Versioning**: Incremental and full snapshots using hardlinks or verified copies.
- **SHA-256 Verification Gate**: A backup run is never marked successful until the destination copy's cryptographic hash matches the source.
- **Conservative Retention**: Automated grandfather-father-son pruning policies that prevent accidental snapshot exhaustion.
- **Interrupted Run Recovery**: Detects aborted backups and safely resumes without duplicating transferred data.

### 4. Health Patrol & Bit-Rot Scrubber
- **Continuous Integrity Audits**: Verifies stored snapshots against their original manifest checksums.
- **Early Bit-Rot Warning**: Identifies decaying storage sectors or silent data corruption before disaster recovery is needed.

### 5. Near-Duplicate & Perceptual Clustering
- **Document Lineage (SimHash)**: 64-bit SimHash hamming distance calculation for tracking drafts, revisions, and near-identical documents.
- **Perceptual Image Hashing (dHash)**: Identifies resized, compressed, or slightly altered versions of images.

### 6. File Reconstruction & Carving
- **Multi-Copy Assembly**: Reconstructs damaged files by stitching valid byte slices across corrupted duplicate copies.
- **Header & EXIF Carving**: Recovers metadata, dimensions, and timestamps from truncated or partially damaged files.

### 7. Triage & Privacy Auditing
- **Downloads Triage**: Categorizes stale downloads, ephemeral installers, and archives by age and access history.
- **Privacy Scanner**: Flags unencrypted private keys, API tokens, and credentials residing in scanned folders.

---

## Which Tool Should I Use? (Quick Guide)

| What you are trying to do | Tool to Use | What it does |
|---|---|---|
| **Free up disk space safely** | **Analysis &rarr; New Scan &amp; Duplicates** | Discovers byte-for-byte exact SHA-256 duplicate files and empty files. Recommends which copy to keep without ever deleting anything automatically. |
| **Clean a chaotic Downloads folder** | **Action &amp; Triage &rarr; Downloads Triage** | Analyzes loose files and neatly groups installers (`.exe`/`.msi`), archives, code, and documents into categorized subfolders. |
| **Fix files with broken extensions (`.bin`/`.tmp`)** | **Health &amp; Recovery &rarr; Signature Rectifier** | Inspects magic bytes (PNG, JPEG, PDF, MP4, etc.) and restores the genuine file extension. |
| **Organize nameless recovered photos (`FILE001.JPG`)** | **Health &amp; Recovery &rarr; Deep Carving** | Extracts EXIF camera metadata and capture dates to restore human-readable structured filenames. |
| **Repair a file using two damaged copies** | **Health &amp; Recovery &rarr; Multi-Copy Reconstruction** | Scans byte sectors across two partially damaged copies and stitches the intact parts into one healthy file. |
| **Find revisions of contracts, essays, or code** | **Health &amp; Recovery &rarr; Document Lineage** | Calculates 64-bit SimHash distances to track edits, revisions, and near-identical document versions. |
| **Find resized, recompressed, or cropped photos** | **Health &amp; Recovery &rarr; Perceptual Near-Duplicates** | Uses perceptual dHash visual fingerprinting to cluster visually similar images. |
| **Protect important folders from drive failure & bit-rot** | **Data Protection &rarr; Verified Backups** | Creates versioned snapshots where every single copied file is verified with SHA-256. |
| **Undo a move or review past actions** | **Action &amp; Triage &rarr; Quarantine &amp; Audit History** | Safely restores quarantined files with zero-overwrite conflict protection, or reviews full SQLite action logs. |
| **Safely test features before touching real files** | **System &rarr; Test Sandbox** | Generates isolated synthetic mock datasets (corrupted files, duplicate trees, messy downloads) in a temporary sandbox. |

---

## Architecture & Tech Stack

| Layer | Technology |
|---|---|
| **Core Engine** | Rust (`crates/organizer-core`) |
| **Desktop Shell** | Tauri 2 |
| **Frontend UI** | React 18, TypeScript, Vite |
| **Local Database** | SQLite via bundled `rusqlite` |
| **Hashing & Integrity** | Streaming SHA-256 (`sha2`), SimHash, dHash |
| **Concurrency** | `rayon` worker pool |

---

## Getting Started

### Prerequisites
- [Node.js](https://nodejs.org/) (>= 18.x)
- [Rust toolchain](https://rustup.rs/) (stable)
- Platform webview (WebView2 on Windows, WebKitGTK on Linux)

### Installation & Development

```bash
# Clone the repository
git clone https://github.com/TheMrDuck/FileHarbor.git
cd FileHarbor

# Install frontend dependencies
npm install

# Run the desktop app in development mode
npm run tauri dev
```

### Production Build

```bash
# Build the production bundle
npm run tauri build
```

---

## Testing & Quality Assurance

FileHarbor includes comprehensive headless test suites covering core engine algorithms, hashing, duplicate detection, lifecycle quarantine, backup operations, and file carving. Tests execute strictly against isolated temporary directories:

```bash
# Run all workspace tests (77+ tests)
cargo test --manifest-path src-tauri/Cargo.toml --workspace

# Frontend typecheck & bundle verification
npm run build
```

---

## License

This project is licensed under the [MIT License](LICENSE).
