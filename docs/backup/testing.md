# Backup Testing

FileHarbor Phase 2 — Test Strategy and Results

## Test Categories

### Unit Tests

Individual functions and components tested in isolation:

- Backup planner (new/changed/unchanged/deleted/unavailable classification)
- Destination validation (source itself, inside source, read-only, etc.)
- Manifest read/write (atomicity, schema versioning)
- Retention policies (keep all, keep last N)
- Error classification and user messages
- Enum serialization/deserialization

### Integration Tests

End-to-end backup flows:

- Full backup creates verified snapshot
- Incremental backup reuses unchanged files
- Backup cancellation is cooperative
- Deleted source files are handled conservatively
- Restore with conflict resolution
- Corruption detection during verification

### Failure-Injection Tests

Controlled failures to verify safe behavior:

- Cancellation before any files processed
- Cancellation mid-backup
- Source file disappearing during backup
- Destination disconnecting during backup
- Insufficient disk space
- Hash mismatch after copy
- Manifest write failure

### Crash Consistency Tests

Simulated termination during:

- Copy phase
- Verification phase
- Manifest commit
- Snapshot commit

On restart, the application determines whether a run is complete,
incomplete, failed, or recoverable. An incomplete run is never displayed
as verified.

### Large Dataset Tests

- Planning performance with 10,000+ files
- Streaming backup of large files (no full-file loading)
- Memory usage bounded during enumeration

## Test Commands

```bash
# Run all tests
cargo test --workspace

# Run backup tests only
cargo test --test backup

# Run with output
cargo test --test backup -- --nocapture

# Run specific test
cargo test --test backup test_engine_full_backup_creates_verified_snapshot
```

## Test Data Generator

The `test-data-generator` crate can create backup-specific fixtures:

- `backup-baseline` — 1,000 files, 100 duplicates, 50 empty files
- `backup-incremental` — modify 100, add 50, delete 20, keep 830 unchanged
- `backup-changed-files` — files with modified content
- `backup-deleted-files` — files removed after initial backup
- `backup-restore` — backup/, source/, restore-target/ directories
- `backup-corruption` — verified backup with one file intentionally modified
- `backup-interrupted` — partial backup with temporary files
- `backup-large` — large file for streaming tests
- `backup-duplicate-content` — identical content across source and backup

## Required Test Matrix

| Scenario | Expected |
|---|---|
| New file | Backed up |
| Unchanged file | Not recopied |
| Changed content | Backed up again |
| Same name, different content | Correctly detected |
| Different name, same content | Content reuse where supported |
| Same size, different content | Not treated as same |
| Empty file | Backed up normally |
| Unicode filename | Correctly backed up |
| Large file | Streaming backup |
| File changes during hash | Unstable |
| File disappears | Warning/error, backup preserved |
| Destination disappears | Backup fails safely |
| Destination fills | Backup stops safely |
| Copy succeeds but hash fails | Not verified |
| Backup file modified externally | Verification detects it |
| Backup file deleted externally | Verification detects it |
| Restore missing file | Restore |
| Restore existing file | Conflict dialog |
| Restore cancelled | No silent overwrite |
| Interrupted backup | Recoverable state |
| Duplicate source + backup copy | Backup remains protected |
| Backup destination scanned by organizer | Protected/warned |
| Snapshot deletion | Explicit confirmation |
| Backup set deletion | Config/data distinction |

## Current Test Count

- organizer-core: 60/60 tests passing
- test-data-generator: 10/10 tests passing
- Total: 70/70 tests passing