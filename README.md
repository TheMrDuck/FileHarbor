# FileHarbor

[![Latest Release](https://img.shields.io/badge/release-v0.2.0-brightgreen.svg)](https://github.com/TheMrDuck/FileHarbor/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-Download%20.exe-blue?logo=windows)](https://github.com/TheMrDuck/FileHarbor/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-Download%20.dmg-black?logo=apple)](https://github.com/TheMrDuck/FileHarbor/releases/latest)
[![Linux](https://img.shields.io/badge/Linux-AppImage%20%7C%20.deb-FCC624?logo=linux&logoColor=black)](https://github.com/TheMrDuck/FileHarbor/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-61DAFB.svg)](https://react.dev/)

> [!TIP]
> **Download FileHarbor v0.2.0 for your platform:**
>
> | Platform | Package / Architecture | Direct Download Link |
> |---|---|---|
> | **Windows** | Setup Wizard (`.exe`) | [**FileHarbor_0.2.0_x64-setup.exe**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64-setup.exe) |
> | **Windows** | Portable (`.exe`) | [**FileHarbor_0.2.0_x64_portable.exe**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64_portable.exe) |
> | **Windows** | Enterprise MSI | [**FileHarbor_0.2.0_x64_en-US.msi**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64_en-US.msi) |
> | **macOS** | Apple Silicon (`aarch64` / M1/M2/M3/M4) | [**FileHarbor_0.2.0_aarch64.dmg**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_aarch64.dmg) |
> | **macOS** | Intel Mac (`x86_64`) | [**FileHarbor_0.2.0_x64.dmg**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_x64.dmg) |
> | **Linux** | Universal Standalone (`.AppImage`) | [**FileHarbor_0.2.0_amd64.AppImage**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/FileHarbor_0.2.0_amd64.AppImage) |
> | **Linux** | Debian / Ubuntu / Mint (`.deb`) | [**fileharbor_0.2.0_amd64.deb**](https://github.com/TheMrDuck/FileHarbor/releases/download/v0.2.0/fileharbor_0.2.0_amd64.deb) |

A safe, verifiable desktop file organization, deduplication, and backup recovery engine for Windows, macOS, and Linux.

FileHarbor operates on **verifiable filesystem facts**. It never guesses or silently alters data. All destructive actions require explicit user confirmation, and deletions always route through an application-managed quarantine with complete audit logs.

---

## Key Capabilities

### 1. Exact Duplicate & Empty File Detection
- **Streaming SHA-256**: High-performance chunked hashing capable of processing large multi-gigabyte files safely without memory spikes.
- **Rayon Parallelism & Live Streaming**: Multi-threaded candidate hashing with fast size pre-filtering and real-time progress metrics (files/s, hashes/s, live path stream).
- **Cross-Project & Multi-Server Boundary Intelligence**: Detects distinct project roots (e.g., independent server deployments, game servers, git repos, virtualenvs) and warns before treating identical configuration files as duplicates.
- **Path-Aware Canonical Recommendations**: Intelligently recommends keeping the canonical original copy while penalizing `- Copy` and redundant subdirectories.
- **0-Byte Media Shield & Safety Audits**: Classifies empty files by safety level—separating corrupt zero-byte media (`0-Byte Corrupt Photo / Video`) from essential project placeholders (`__init__.py`, `.gitkeep`).
- **Universal File Inspection**: Interactive `↗️ Open` (launch in default viewer) and `📁 Reveal` (File Explorer / Finder) actions on every candidate file before taking action.
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
- **Early Bit-Rot Warning & Auto-Heal**: Identifies decaying storage sectors or silent data corruption before disaster recovery is needed.

### 5. Near-Duplicate & Perceptual Clustering
- **Document Lineage (SimHash)**: 64-bit SimHash hamming distance calculation for tracking drafts, revisions, and near-identical documents.
- **Perceptual Image Hashing (dHash)**: Identifies resized, compressed, or slightly altered versions of images.

### 6. File Reconstruction & Carving
- **Multi-Copy Assembly**: Reconstructs damaged files by stitching valid byte slices across corrupted duplicate copies.
- **Header & EXIF Carving**: Recovers metadata, dimensions, and timestamps from truncated or partially damaged files.

### 7. Universal Folder Triage & Privacy Auditing
- **Folder Triage & Clutter Organizer**: Categorizes loose clutter, stale installers, and unorganized files from any directory (Downloads, Desktop, USB drives) into structured subfolders with collision avoidance.
- **Quick Folder Presets**: Instant one-click path resolution for Downloads, Desktop, and Documents.
- **Privacy Shield & Air-Gapped Security**: 100% offline, zero-telemetry architecture that automatically shields sensitive keys (`.ssh`, `.aws`, `.kube`, crypto wallets) and credential vaults (`.kdbx`, `.pem`, `.key`) during all operations.

---

## Which Tool Should I Use? (Quick Guide)

| What you are trying to do | Tool to Use | What it does |
|---|---|---|
| **Free up disk space safely** | **Analysis &rarr; New Scan &amp; Duplicates** | Discovers byte-for-byte SHA-256 duplicates with cross-server project boundary awareness, path-aware keep recommendations, and one-click inspection (`↗️ Open` / `📁 Reveal`). |
| **Inspect & review zero-byte files** | **Analysis &rarr; Empty Files** | Audits 0-byte items with the 0-Byte Media Shield, separating corrupt images/videos from harmless placeholders and critical project markers. |
| **Clean a chaotic folder or Downloads** | **Action &amp; Triage &rarr; Folder Triage** | Analyzes loose files in any messy folder (Downloads, Desktop, external drive) and neatly groups installers (`.exe`/`.msi`), archives, code, and documents into categorized subfolders. |
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

FileHarbor includes comprehensive headless test suites covering core engine algorithms, streaming hashing, duplicate detection, cross-project boundary detection, 0-byte media classification, lifecycle quarantine, verified backup operations, and file carving. Tests execute strictly against isolated temporary directories:

```bash
# Run all workspace tests (86+ tests)
cargo test --manifest-path src-tauri/Cargo.toml --workspace

# Frontend typecheck & bundle verification
npm run build
```

---

## License

This project is licensed under the [MIT License](LICENSE).
