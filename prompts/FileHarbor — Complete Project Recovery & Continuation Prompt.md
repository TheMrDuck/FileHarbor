# FileHarbor — Complete Project Recovery & Continuation Prompt

## ROLE

You are a senior software architect and engineer taking over an existing desktop application project called **FileHarbor**.

Your job is to understand the project from this document, inspect the recovered/current repository, preserve everything that is already correct, identify what is missing or broken, and continue development from the correct phase without unnecessarily rewriting working functionality.

Do NOT blindly recreate the project.

First inspect the repository and determine what actually exists.

The project is a safety-first desktop file organization, duplicate detection, quarantine, restoration, and eventually backup-assistance application.

The original project was started from an empty repository, so the architecture was intentionally designed around:

- Rust for the filesystem/core engine
- Tauri 2 for the desktop application shell
- React + TypeScript for the UI
- SQLite for persistent application state
- A separate deterministic Rust test-data generator for QA

The application is called:

# FileHarbor

---

# 1. PRODUCT VISION

FileHarbor is intended to solve a common problem:

Users accumulate thousands of files in places such as:

- Downloads
- Desktop
- Documents
- external drives
- old HDDs
- old SSDs
- recovered drives
- backup folders
- phone exports
- cloud-sync folders
- miscellaneous temporary directories

Over time these folders become chaotic.

Users may have:

- exact duplicate files
- renamed duplicates
- duplicate files in different directories
- empty files
- abandoned files
- temporary files
- files with strange names
- recovered files
- corrupted files
- partially recovered files
- inaccessible files
- broken links
- files that the user wants to keep
- files that the user does not want to keep
- files that should be backed up before organization

FileHarbor should help users understand their filesystem and safely organize it.

The central philosophy is:

> **Analyze first. Explain clearly. Never destroy user data without explicit confirmation.**

FileHarbor should feel like a trustworthy filesystem assistant rather than an aggressive "cleanup" utility.

---

# 2. IMPORTANT SAFETY PHILOSOPHY

This is one of the most important aspects of the application.

FileHarbor MUST NOT behave like a dangerous automatic cleaner.

Never:

- automatically delete files
- automatically overwrite files
- automatically modify user files during scanning
- follow symlinks/junctions recursively
- assume an empty file is useless
- assume two files with the same name are duplicates
- assume two files with the same size are duplicates
- assume a recovered file is safe
- silently replace an existing file
- silently discard a file during restore
- permanently delete something without explicit confirmation

The application must always prefer:

**Inspect → Analyze → Recommend → User confirms → Quarantine → Verify → Optional permanent deletion**

---

# 3. CURRENT PROJECT STATUS BEFORE LAPTOP FORMAT

The project had reached a very strong Phase 1 state.

The original repository started completely empty:

- fresh `git init`
- zero commits
- no existing files

Therefore, there was no legacy application code that needed preservation.

The stack was deliberately selected as:

```text
Rust
  ↓
organizer-core
  ↓
Tauri 2
  ↓
React + TypeScript
```

SQLite is used for persistence.

---

# 4. PROJECT STRUCTURE

The intended project structure is:

```text
File Organizer/
│
├── src/
│   ├── components/
│   ├── pages/
│   ├── api.ts
│   └── styles.css
│
├── src-tauri/
│   ├── crates/
│   │   ├── organizer-core/
│   │   └── test-data-generator/
│   │
│   ├── src/
│   └── Cargo.toml
│
├── package.json
└── documentation files
```

The Rust workspace contains:

```text
organizer-core
test-data-generator
```

The application shell is Tauri 2.

The frontend is React + TypeScript + Vite.

---

# 5. PHASE 1 — COMPLETED FUNCTIONALITY

Phase 1 was considered complete.

The following functionality had been implemented and tested.

---

# 6. ORGANIZER CORE

Crate:

```text
organizer-core
```

The core engine is responsible for filesystem analysis and safe file operations.

---

# 7. SCANNER

File:

```text
scanner.rs
```

The scanner uses a staged pipeline:

```text
Discovery
    ↓
Metadata collection
    ↓
Candidate grouping
    ↓
Hashing
    ↓
Duplicate grouping
```

The scanner supports:

- recursive directory discovery
- metadata collection
- batched SQLite inserts
- candidate identification using file size
- parallel hashing
- cancellation
- progress reporting
- quarantine-root exclusion
- symlink safety
- unstable-file detection
- missing/inaccessible file handling

The quarantine root is always excluded from scanning.

Symlinks/junctions are not recursively followed.

File symlinks are recorded as skipped.

---

# 8. HASHING

File:

```text
hasher.rs
```

Hashing uses:

```text
SHA-256
```

with streaming chunks of:

```text
256 KiB
```

Files are NOT loaded entirely into RAM.

The hasher detects files changing during hashing.

If the file changes while being hashed:

```text
Unstable
```

is returned and the hash is discarded.

Cancellation is checked between chunks.

This is important for files that are actively being modified.

---

# 9. DUPLICATE DETECTION

File:

```text
dupdetect.rs
```

The Phase 1 definition of an exact duplicate is:

```text
same file size
AND
identical SHA-256 hash
```

Therefore:

### Same name + different content

NOT a duplicate.

### Same size + different content

NOT a duplicate.

### Different names + identical content

IS a duplicate.

### Different directories + identical content

IS a duplicate.

### Renamed identical files

IS a duplicate.

This is intentional.

File names are never treated as proof of duplication.

---

# 10. EMPTY FILE DETECTION

Empty files are:

```text
size == 0
```

They are NOT automatically considered useless.

The UI explicitly communicates:

> Empty does not necessarily mean useless.

Empty files are presented separately for user review.

They are excluded from duplicate groups.

---

# 11. KEEP RECOMMENDATION

FileHarbor has an advisory heuristic that recommends which duplicate to keep.

The recommendation considers things such as:

- original-looking filename
- path depth
- oldest modification time

This is ONLY a recommendation.

It does not automatically delete anything.

The user remains in control.

---

# 12. QUARANTINE SYSTEM

Files are not immediately deleted.

Instead:

```text
User selects files
        ↓
Explicit confirmation
        ↓
Move to managed quarantine
        ↓
Verify transfer
        ↓
Remove original only after successful verification
```

The transfer process is:

```text
copy
 ↓
verify size
 ↓
verify hash where applicable
 ↓
remove original
```

If the transfer fails:

```text
original remains untouched
```

This is a major safety guarantee.

---

# 13. QUARANTINE MANIFEST

Each quarantine batch receives a:

```text
manifest.json
```

The manifest stores information needed for recovery and restoration.

Filenames are sanitized.

The quarantine system is designed to make recovery possible even after organizational operations.

---

# 14. RESTORE

Restore recreates missing directories where necessary.

If the destination already contains a file, FileHarbor NEVER silently overwrites it.

The user must choose a conflict policy.

Supported policies include:

```text
Keep Existing
New Name
Cancel
```

Restore operations are tracked.

---

# 15. PERMANENT DELETE

Permanent deletion is heavily restricted.

The permanent delete operation can ONLY operate on files already inside the managed quarantine root.

A path outside the quarantine root must be rejected.

Permanent deletion requires explicit confirmation.

There is no automatic permanent cleanup.

---

# 16. PERSISTENCE

File:

```text
store.rs
```

SQLite is used.

The database contains information related to:

- scan sessions
- file records
- duplicate groups
- hash cache
- quarantine items
- operation history
- exclusions
- settings

SQLite is configured with WAL mode.

---

# 17. HASH CACHE

The hash cache uses:

```text
(path, size, mtime)
```

as its identity tuple.

The cache is an optimization only.

It MUST NOT be treated as cryptographic proof that a file is unchanged if stronger verification is needed.

If the identity changes, the file must be rehashed.

---

# 18. FILESYSTEM SAFETY

File:

```text
fs_layer.rs
```

The filesystem abstraction exists to keep filesystem operations centralized and safe.

Important behavior:

- do not follow symlinks recursively
- detect inaccessible files
- handle missing files
- handle locked files
- avoid panics
- return structured errors

---

# 19. DELETE SAFETY

File:

```text
deleter.rs
```

Permanent deletion validates that the target is under the managed quarantine root.

It must reject:

- arbitrary filesystem paths
- paths outside quarantine
- invalid paths
- unsafe deletion requests

---

# 20. FRONTEND

Frontend stack:

```text
React
TypeScript
Vite
Tauri 2
```

The UI was implemented as a desktop-first dark interface.

---

# 21. FRONTEND PAGES

The application contains the following conceptual pages.

## New Scan

Allows the user to:

- choose a folder
- configure exclusions
- start a scan
- access recent sessions

---

## Overview

Displays:

- total files
- total size
- duplicate statistics
- empty-file statistics
- problems
- reclaimable space

Provides navigation into detailed analysis.

---

## Duplicates

Shows duplicate groups.

The UI allows users to:

- inspect each group
- see files
- see sizes
- see locations
- see recommended keep
- select files
- see reclaimable space

Default behavior:

> select all duplicate files except the recommended keep.

This is a recommendation, not an automatic destructive operation.

---

## Empty Files

Shows zero-byte files separately.

Features include:

- table/list
- selection
- select all
- quarantine selected

The UI warns:

> Empty files are not necessarily useless.

---

## Problems

Shows files that could not be properly analyzed.

Examples:

- inaccessible
- locked
- missing
- unstable
- unsupported/problematic filesystem conditions

Includes retry analysis functionality.

---

## All Files

Provides:

- search
- filters
- sorting
- pagination

The UI must remain usable with large datasets.

---

## Quarantine

Provides:

- quarantined files
- restore
- permanent deletion

Restore conflicts open an explicit conflict dialog.

Permanent delete requires guarded confirmation.

---

## Operations

Displays operation history.

Users can inspect:

- what happened
- when
- which files were affected
- success/failure
- per-item outcomes

---

## Settings

Contains configurable options such as:

- default locations
- exclusions
- concurrency
- quarantine retention
- related safety settings

---

# 22. SCAN PROGRESS UI

During scanning, the UI displays a live progress overlay.

It should show:

- current phase
- counts
- current file where available
- progress
- cancellation

Cancellation is cooperative.

A cancelled scan must not cause destructive side effects.

Partial scan results may remain persisted.

---

# 23. ACCESSIBILITY

The frontend includes:

- keyboard navigation
- ARIA labels
- focusable controls
- Tab navigation
- Enter interaction
- Escape handling
- confirmation dialogs

Do not regress accessibility while extending the UI.

---

# 24. TEST DATA GENERATOR

A separate Rust crate exists:

```text
test-data-generator
```

CLI name:

```text
testgen
```

Its purpose is deterministic QA and regression testing.

It is NOT user-facing production functionality.

---

# 25. TESTGEN COMMANDS

Examples:

```bash
testgen generate \
  --scenario phase1-baseline \
  --output ./test-data/test \
  --files 2500
```

List scenarios:

```bash
testgen list-scenarios
```

Describe:

```bash
testgen describe phase1-baseline
```

Verify:

```bash
testgen verify \
  --manifest ./test-data/test/manifest.json
```

Clean:

```bash
testgen clean ./test-data/test
```

Regression:

```bash
testgen regression \
  --output ./test-data/regression
```

---

# 26. TESTGEN SCENARIOS

Implemented scenarios include:

```text
phase1-baseline
duplicates
empty-files
unicode
large-files
performance-small
performance-medium
performance-large
performance-stress
recovered-hdd
unstable
missing-files
symlinks
locked-files
permissions
regression
```

Approximate scenario sizes:

| Scenario | Files |
|---|---:|
| phase1-baseline | 2,500 |
| duplicates | 1,000 |
| empty-files | 500 |
| unicode | 500 |
| large-files | 50 |
| performance-small | 1,000 |
| performance-medium | 10,000 |
| performance-large | 100,000 |
| performance-stress | 1,000,000 |
| recovered-hdd | 5,000 |
| unstable | 10 |
| missing-files | 1,000 |
| symlinks | 100 |
| locked-files | 10 |
| permissions | 50 |
| regression | 5,000 |

---

# 27. TESTGEN MANIFEST

Each generated dataset contains a manifest.

Example structure:

```json
{
  "schemaVersion": 1,
  "generatorVersion": "0.1.0",
  "scenario": "phase1-baseline",
  "seed": 12345,
  "files": [
    {
      "id": "file-000001",
      "relativePath": "documents/report.pdf",
      "sizeBytes": 1048576,
      "sha256": "...",
      "categories": [
        "exact_duplicate_copy"
      ],
      "duplicateGroup": "dup-0001",
      "expectedDuplicate": true,
      "expectedEmpty": false
    }
  ]
}
```

The manifest represents the ground truth.

---

# 28. TESTGEN SAFETY

The test generator must never accidentally destroy real user data.

It rejects dangerous output paths such as:

- home directory
- Downloads
- system roots
- other dangerous locations

unless an explicit testing-only override is provided.

The override is:

```text
allow_dangerous
```

Manifest writing is atomic:

```text
write temp
 ↓
verify
 ↓
rename
```

Cleanup requires a valid manifest.

Ctrl+C handling exists.

---

# 29. TESTING STATUS

At the last known state:

## organizer-core

```text
28 tests
28 passed
0 failed
```

## test-data-generator

```text
10 tests
10 passed
0 failed
```

Total:

```text
38 tests
38 passed
0 failed
```

---

# 30. CORE TEST COVERAGE

Important tested scenarios include:

- duplicate detection matrix
- same-name different-content
- same-size different-content
- renamed identical files
- empty files
- symlink behavior
- locked files
- changed-during-hash
- missing files
- cancelled scans
- quarantine moves
- restore
- restore conflicts
- directory recreation
- verified cross-volume transfers
- partial batch failures
- permanent deletion restrictions
- large file scanning
- hash cache behavior

A 2010-file scan was tested successfully.

---

# 31. MANUAL TESTING

Manual testing had also been completed.

The project was therefore not merely unit-test complete.

The application had undergone manual verification of the implemented Phase 1 behavior.

---

# 32. VERIFIED DATASET EXAMPLE

One generated dataset was verified with output equivalent to:

```text
Expected files: 1745
Found:          1739
Missing:        0
Unexpected:     0
Size mismatches: 0
Hash mismatches: 0

Duplicate groups:
  Expected: 160
  Verified: 160

Dataset is valid
```

The discrepancy between generated requested file count and expected manifest count is intentional where scenario generation creates metadata/group structures; verification itself showed:

```text
0 unexpected missing
0 unexpected files
0 size mismatches
0 hash mismatches
160 duplicate groups verified
```

Do not interpret the raw requested number as the verification ground truth.

---

# 33. BUILD VERIFICATION

At the last known state:

```bash
cargo test --workspace
```

passed.

```bash
cargo clippy --workspace
```

was clean.

```bash
cargo build --workspace
```

succeeded.

Frontend:

```bash
npm run build
```

succeeded.

---

# 34. KNOWN LIMITATIONS AFTER PHASE 1

These limitations were known and intentionally left for future phases.

## No pause/resume

Cancellation exists.

Pause/resume does not.

Future work can introduce checkpointing.

---

## Large UI lists

The current UI uses pagination.

Virtualized million-file lists were identified as future work.

---

## Restore conflict policy

Current batch restore conflict resolution applies one selected policy to all conflicts in the batch.

Per-file conflict decisions can be improved later.

---

## Non-UTF-8 paths

Some paths are stored lossily for UI purposes.

Future work may improve native path handling.

---

## Manual large-scale GUI testing

Large real-world folders still need more extensive testing.

---

# 35. ORIGINAL PHASE 2 DIRECTION

Phase 2 was planned to expand FileHarbor beyond basic exact duplicate cleanup.

The original recommendations were:

1. Analysis-layer plugin architecture
2. Near-duplicate detection
3. Image similarity
4. Corruption/recovery analysis
5. Pause/resume
6. Million-file UI virtualization
7. Quarantine retention reminders
8. Per-file retry

But Phase 2 should NOT simply become a giant feature dump.

It should be implemented incrementally.

---

# 36. IMPORTANT FUTURE PRODUCT DIRECTION — BACKUPS

A major product idea was added during planning:

FileHarbor should eventually help users create safer backups.

This does NOT mean Phase 1 should be rewritten.

Phase 1's core organization behavior remains valid.

Backup functionality should be layered on top.

The intended concept is:

```text
User files
    ↓
Analyze
    ↓
Identify duplicates/problems
    ↓
Recommend organization
    ↓
Backup important data
    ↓
Verify backup
    ↓
Optionally organize/quarantine
```

FileHarbor should eventually be able to help users answer:

> "What should I back up before I clean this folder?"

and:

> "Did my backup actually contain everything important?"

---

# 37. BACKUP PHILOSOPHY

Backup functionality must follow the same safety principles.

Never assume:

```text
copied = backed up
```

A backup should ideally be:

```text
Selected source files
        ↓
Copy
        ↓
Verify
        ↓
Compare metadata/hash
        ↓
Record backup manifest
        ↓
Report result
```

Future backup functionality should support:

- destination selection
- backup profiles
- incremental backup
- verification
- manifests
- failed-file reporting
- interrupted backup recovery
- resume
- duplicate-aware backup decisions
- clear storage requirements
- backup history

Do NOT implement destructive synchronization by default.

A backup tool should never unexpectedly delete the destination's files.

---

# 38. RECOVERED HDD / SSD SUPPORT

One of the most important long-term goals is supporting files recovered from damaged drives.

Recovery software can produce situations where:

- filenames are corrupted
- directory structure is lost
- multiple copies exist
- files have the same name
- files have the same size
- content differs
- files are partially corrupted
- metadata is unreliable
- files were recovered from multiple locations

Therefore FileHarbor must distinguish:

## Exact duplicate

Same size + same SHA-256.

## Same name

Not enough.

## Same size

Not enough.

## Potentially related

May be useful for future analysis.

## Corrupted

Requires stronger analysis.

---

# 39. IMPORTANT CORRUPTION-DESIGN PRINCIPLE

Do NOT claim that SHA-256 alone can determine whether a recovered file is "correct."

A hash can establish:

> Two byte sequences are identical.

It cannot establish:

> This file is the original uncorrupted version.

Future corruption detection may require:

- file-format validation
- parser validation
- container integrity checks
- image decoding
- PDF validation
- archive testing
- media probing
- document structure validation
- comparing multiple recovered copies
- known-good reference hashes
- checksums from external sources
- sector/recovery metadata where available

This is future functionality.

---

# 40. NEAR-DUPLICATE DETECTION

Phase 2 should eventually support cases such as:

```text
photo.jpg
photo-edited.jpg
photo-recovered.jpg
photo-copy.jpg
```

where files are not byte-for-byte identical.

Possible future methods:

- perceptual image hashes
- normalized metadata comparison
- document similarity
- audio fingerprints
- video fingerprints
- structural comparison

But these MUST be treated as:

```text
possible match
```

not:

```text
duplicate
```

unless confidence is extremely high and the user explicitly accepts the classification.

---

# 41. ANALYSIS ARCHITECTURE

A major Phase 2 architectural goal is introducing an analysis layer.

Conceptually:

```text
Filesystem Scanner
        ↓
FileRecord
        ↓
Analysis Engine
        ├── Exact Duplicate Analyzer
        ├── Empty File Analyzer
        ├── Near Duplicate Analyzer
        ├── Image Analyzer
        ├── Document Analyzer
        ├── Corruption Analyzer
        └── Backup Analyzer
        ↓
Findings
        ↓
User Review
```

The scanner should not become a giant monolithic component.

Analysis modules should consume normalized `FileRecord` data.

---

# 42. PLUGIN-STYLE ANALYZER INTERFACE

Phase 2 should consider a trait/interface similar to:

```rust
trait Analyzer {
    fn id(&self) -> &str;

    fn analyze(
        &self,
        files: &[FileRecord],
        context: &AnalysisContext,
    ) -> AnalysisResult;
}
```

The exact API may differ after repository inspection.

The important principle is modularity.

Adding a new analyzer should not require rewriting the scanner.

---

# 43. DATA MODEL DIRECTION

Future analysis results should be represented independently from raw file records.

Conceptually:

```text
FileRecord
Analysis
Finding
Recommendation
Action
```

Example:

```text
FileRecord
    ↓
ExactDuplicateFinding
    ↓
Recommendation
    ↓
UserAction
```

Another:

```text
FileRecord
    ↓
CorruptionFinding
    ↓
Warning
```

Another:

```text
FileRecord
    ↓
BackupCandidate
    ↓
BackupPlan
```

---

# 44. USER TRUST MODEL

FileHarbor should clearly distinguish:

### Fact

Example:

> These two files have identical SHA-256 hashes.

### Inference

Example:

> This file is probably the best copy to keep.

### Warning

Example:

> This recovered JPEG failed image decoding.

### Recommendation

Example:

> Consider backing up this file before organizing.

### Action

Example:

> Move selected files to quarantine.

The UI should never present an inference as an absolute fact.

---

# 45. PHASE 2 UX PRINCIPLE

The application should become more intelligent without becoming more dangerous.

Good:

> "We found 43 exact duplicate groups. You could reclaim 2.8 GB."

Bad:

> "We cleaned 2.8 GB automatically."

Good:

> "This recovered PDF appears damaged and failed structural validation."

Bad:

> "This file is corrupted."

unless the system has sufficient evidence.

---

# 46. PERFORMANCE REQUIREMENTS

FileHarbor must be designed for:

- thousands of files
- hundreds of thousands of files
- potentially millions of files

Avoid:

- reading every file completely when unnecessary
- loading all records into frontend memory
- blocking the UI
- synchronous filesystem operations on the UI thread
- excessive SQLite transactions
- unnecessary hashing

Candidate reduction should happen before expensive hashing.

The Phase 1 size-group optimization should remain.

---

# 47. CACHING PRINCIPLES

Cache expensive analysis where safe.

But caches must never silently turn stale information into truth.

A cache entry should include enough identity information to determine whether it is still applicable.

Potential future cache identity:

```text
path
size
mtime
file-id
analysis-version
algorithm-version
```

If an analyzer changes, its cached results may need invalidation.

---

# 48. TESTING REQUIREMENTS FOR FUTURE WORK

Every Phase 2 feature must include:

## Unit tests

Test the core algorithm independently.

## Integration tests

Test interaction with SQLite/filesystem.

## Safety tests

Verify that dangerous operations remain impossible.

## Regression tests

Ensure Phase 1 remains intact.

## Test-data-generator scenarios

Add deterministic fixtures where applicable.

---

# 49. DO NOT BREAK PHASE 1

This is critical.

When implementing Phase 2:

DO NOT rewrite working Phase 1 components merely for stylistic reasons.

Do not:

- replace SHA-256 duplicate detection
- remove quarantine
- remove restore safety
- remove delete restrictions
- remove manifests
- remove cancellation
- remove scan sessions
- remove operation history
- remove testgen
- remove safety checks

Extend the architecture instead.

---

# 50. RECOVERY PROCEDURE AFTER LAPTOP FORMAT

When taking over the recovered repository, do this FIRST.

## Step 1 — Inspect repository

Run:

```bash
git status
git log --oneline --all
```

Then inspect:

```text
package.json
Cargo.toml
src-tauri/Cargo.toml
src-tauri/crates/
src/
```

Do not assume files exist.

---

## Step 2 — Inspect workspace

Run:

```bash
cargo metadata
```

Determine:

- workspace members
- package names
- versions
- dependencies

---

## Step 3 — Verify frontend

Run:

```bash
npm install
npm run build
```

If dependencies are missing, restore them according to package.json.

---

## Step 4 — Verify Rust

Run:

```bash
cargo test --workspace
cargo clippy --workspace
cargo build --workspace
```

---

## Step 5 — Identify regressions

If tests fail:

1. determine whether failure is environmental
2. determine whether code is actually broken
3. inspect git history
4. fix root cause
5. rerun full test suite

Do not immediately rewrite modules.

---

# 51. IF THE REPOSITORY IS EMPTY AGAIN

If the repository is actually empty after recovery:

Do not pretend Phase 1 exists.

Instead:

1. confirm repository is empty
2. reconstruct Phase 1 architecture from this document
3. implement the project systematically
4. rebuild the test suite
5. rebuild test-data-generator
6. run all tests
7. only then continue to Phase 2

---

# 52. IF PHASE 1 CODE EXISTS

If the repository contains the Phase 1 implementation:

Treat it as the source of truth.

This document is the architectural/context reference.

Inspect actual code before making decisions.

---

# 53. REQUIRED FIRST RESPONSE WHEN CONTINUING DEVELOPMENT

Before implementing anything substantial, provide:

## Repository Reality Check

Report:

```text
Repository status
Git status
Workspace packages
Frontend status
Backend/core status
Test status
Build status
Detected missing pieces
Detected regressions
Phase currently implemented
Recommended next step
```

Then proceed.

Do not fabricate missing files or results.

---

# 54. DEVELOPMENT RULES

Follow these rules throughout the project.

### Rule 1 — Safety first

Filesystem operations are potentially destructive.

Treat every operation as high risk.

### Rule 2 — Verify before delete

Never delete an original until a verified quarantine transfer has succeeded.

### Rule 3 — Explicit user intent

No destructive action without explicit user confirmation.

### Rule 4 — Explain decisions

Whenever FileHarbor recommends something, show why.

### Rule 5 — Preserve originals during analysis

Scanning and analysis should be read-only.

### Rule 6 — Deterministic tests

Use deterministic seeds for generated datasets.

### Rule 7 — Test edge cases

Especially:

- Unicode
- long paths
- locked files
- missing files
- changed files
- symlinks
- permissions
- cross-volume moves
- crashes/interruption
- duplicate names
- duplicate contents
- corrupted files

### Rule 8 — Do not trust filenames

Names are metadata, not identity.

### Rule 9 — Do not trust file size alone

Size is only a candidate filter.

### Rule 10 — Do not overclaim corruption detection

Only claim what the analysis can prove.

---

# 55. PHASE 2 PRIORITY ORDER

Unless the existing repository or product requirements dictate otherwise, use this order:

## Phase 2A — Architecture

Implement:

- analysis-layer abstraction
- findings model
- analyzer registry
- versioned analysis results
- improved progress model
- cancellation propagation

Do not add complicated UI yet.

---

## Phase 2B — Advanced duplicate analysis

Implement:

- near-duplicate architecture
- image similarity
- possible related files
- confidence levels

Keep exact duplicates separate and authoritative.

---

## Phase 2C — File integrity analysis

Implement format-aware integrity checks where practical.

Examples:

- JPEG decoding
- PNG validation
- PDF structural validation
- ZIP integrity
- common archive checks
- basic media probing

Results should be warnings/findings, not automatic deletion decisions.

---

## Phase 2D — Backup assistance

Introduce:

```text
Backup
Backup Profile
Backup Job
Backup Manifest
Backup Verification
```

Support:

- selecting source folders
- selecting destination
- calculating required space
- incremental backup
- content-aware reuse of existing identical files
- verification
- resume
- history
- failure reporting

Never make backup behave like destructive sync by default.

---

## Phase 2E — Large-scale UX

Introduce:

- virtualized lists
- improved database pagination
- background analysis
- better search
- filtering
- analyzer-specific findings
- scalable dashboards

Target:

```text
1,000,000+ files
```

without freezing the UI.

---

## Phase 2F — Recovery workflows

Build dedicated support for recovered drives:

- recovery dataset recognition
- duplicate recovered files
- suspicious names
- integrity warnings
- related-copy grouping
- backup-before-cleanup workflow

---

# 56. BACKUP FEATURE — LONG-TERM UX

The ideal user workflow eventually becomes:

```text
Choose folder
     ↓
Scan
     ↓
FileHarbor analyzes
     ↓
"You have 48,293 files"
     ↓
"3,421 exact duplicates"
     ↓
"12 suspicious/inaccessible files"
     ↓
"1.8 TB potentially important data"
     ↓
"Recommended: create a backup first"
     ↓
Choose backup destination
     ↓
Backup
     ↓
Verify
     ↓
"Backup verified"
     ↓
Review organization recommendations
     ↓
Quarantine selected files
     ↓
Optional permanent cleanup
```

This is the larger product vision.

---

# 57. PRODUCT POSITIONING

FileHarbor should not feel like:

> "Delete junk from my computer."

It should feel like:

> **"Help me understand, protect, organize, and safely clean up my files."**

That distinction should guide future UX and architecture.

---

# 58. DOCUMENTATION REQUIREMENTS

Maintain:

```text
README.md
docs/architecture.md
docs/safety.md
```

Add documentation when introducing:

- backup
- analysis plugins
- integrity checking
- recovery workflows
- database migrations
- new safety mechanisms

Documentation should describe actual behavior, not intended behavior.

---

# 59. RELEASE QUALITY BAR

A feature is not complete simply because it compiles.

Before calling a phase complete:

```text
Code implemented
        ↓
Unit tests
        ↓
Integration tests
        ↓
Safety tests
        ↓
Regression tests
        ↓
Build
        ↓
Lint
        ↓
Manual testing
        ↓
Documentation
        ↓
Final verification
```

---

# 60. FINAL INSTRUCTION

You are taking over **FileHarbor**, a safety-first filesystem organization and backup-assistance application.

The previous known state was:

```text
Phase 1
    ✅ Core scanner
    ✅ Exact duplicate detection
    ✅ Empty file detection
    ✅ Hashing
    ✅ Quarantine
    ✅ Restore
    ✅ Permanent delete safety
    ✅ SQLite persistence
    ✅ React/Tauri UI
    ✅ Test-data generator
    ✅ 38 automated tests passing
    ✅ Manual testing completed
    ✅ Build verified
```

The intended next stage is:

```text
Phase 2
    ↓
Modular analysis architecture
    ↓
Near-duplicate analysis
    ↓
Integrity/corruption analysis
    ↓
Backup assistance
    ↓
Large-scale UX
    ↓
Recovery-drive workflows
```

Do not lose the safety model while adding intelligence.

The core promise of FileHarbor is:

> **Your files remain yours. FileHarbor analyzes them, explains what it finds, recommends safe actions, and only changes or deletes data when you explicitly tell it to.**