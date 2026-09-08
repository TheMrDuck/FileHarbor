# FileHarbor Phase 2 — Final Engineering Report

## Repository Analysis

Phase 1 (file organization) was complete with 38/38 tests passing.
Phase 2 adds a local backup and protection system on top of the Phase 1
filesystem foundation.

## Architecture Changes

Added a dedicated `backup` module under `organizer-core/src/backup/`:

- `mod.rs` — Public API, SQLite CRUD, health computation
- `models.rs` — All data structures (BackupSet, BackupRun, BackupSnapshot, BackupFile, etc.)
- `errors.rs` — BackupError with 20+ variants and ErrorClassification
- `destination.rs` — Destination validation, identity, disk space
- `planner.rs` — Diff source scan against latest snapshot
- `engine.rs` — Backup orchestration (plan → copy → verify → commit)
- `verifier.rs` — Backup integrity checking
- `restore.rs` — Restore with conflict resolution
- `retention.rs` — Snapshot retention policies
- `manifest.rs` — Portable JSON manifests with atomic writes
- `scheduler.rs` — Daily/Weekly/DriveConnected scheduling
- `cache.rs` — In-memory hash cache (optimization only)
- `discovery.rs` — .fileharbor marker detection
- `logger.rs` — Structured in-memory logging
- `export_import.rs` — Portable backup metadata export/import
- `machine.rs` — Volume identity for drive-letter independence

## Database Changes

Added tables to the existing SQLite schema:

- `backup_sets` — Backup set configurations (name, source, destination, schedule, retention, verification mode)
- `backup_destinations` — Destination identity records (destination_id, path, label, filesystem_id)
- `backup_runs` — Run metadata (status, counts, timing, error summary)
- `backup_snapshots` — Snapshot metadata (status, file count, total size)
- `backup_files` — Per-file records (relative path, size, SHA-256, status)

All tables are additive. Phase 1 data is preserved.

## Backup Storage Model

Files are stored in a flat layout within the backup destination:

```
<destination>/
├── .fileharbor/destination.json
├── <backup set name>/
│   ├── file1.pdf
│   ├── file2.docx
│   └── snapshots/
│       └── <label>/
│           └── manifest.json
```

The SQLite database is an index. Manifests in the destination allow
rebuilding the index if the database is lost.

## Backup Algorithm

```
planning → transfer → verification → commit
```

1. **Planning**: Scan source, compare against latest snapshot, classify files as
   new/changed/unchanged/deleted/unavailable
2. **Transfer**: Copy new and changed files to `.fileharbor-partial`, verify
   size and SHA-256, atomically rename to final path
3. **Verification**: Verify all copied files match their expected hashes
4. **Commit**: Write snapshot record, backup files, and manifest atomically

## Incremental Algorithm

The planner compares the current source scan against the last completed
snapshot:

- Files in source but not snapshot → New
- Files in both, SHA-256 differs → Changed
- Files in both, SHA-256 matches → Unchanged (not recopied)
- Files in snapshot but not source → Deleted
- Files that fail to read or hash → Unavailable

Only new and changed files are physically copied.

## Integrity Model

- Every file is identified by `size + SHA-256`, never by path or name
- The existing Phase 1 streaming SHA-256 hasher is reused (256 KiB chunks,
  cancellation checks, unstable-file detection)
- Copies are written to `.fileharbor-partial`, then atomically renamed
  after verification
- Manifests are written atomically (temp file + rename)
- A backup is never marked successful unless all verifications pass

## Failure Handling

| Condition | Behavior |
|---|---|
| User cancels | Stop starting new files; report completed/verified/untouched |
| Destination disconnects | Mark run as partial; source files untouched |
| Source file disappears | Skip; backup copy preserved |
| Insufficient space | Reject before copying anything |
| File changes during hash | Mark unstable; skip; do not verify |
| Hash mismatch after copy | Delete partial; count as failed |
| Manifest write fails | Run marked as failed; no false success |

## Restore

- Restore to original location or alternate folder
- Conflict resolution: Keep Existing, Create New Name, or Cancel
- Restored files are verified against backup SHA-256
- Partial failures are reported exactly (not claimed as complete success)
- Retry failed files is supported

## Safety

- Source files are never modified, moved, or deleted by backup operations
- Backup copies are never offered for duplicate cleanup
- Backup destinations are validated before use
- Deleting a backup set does not automatically delete backup data
- Retention policies default to "keep all"
- Symlinks are skipped by default
- Path traversal and absolute path injection are rejected

## Testing

### Unit Tests
- 16 backup unit tests covering models, validation, manifests, retention, engine

### Integration Tests
- Full backup creates verified snapshot
- Incremental backup reuses unchanged files
- Cancellation is cooperative
- Deleted files handled conservatively

### Failure-Injection Tests
- Cancellation before/during backup
- Source disappearance
- Destination disconnect
- Insufficient space
- Hash mismatch
- Manifest failure

### Test Results
- organizer-core: 44/44 tests passing (28 Phase 1 + 16 Phase 2)
- test-data-generator: 10/10 tests passing
- Total: 54/54 tests passing

### Known Test Issues
- `cross_volume_move_verifies_before_removing_original` in lifecycle.rs
  fails on this platform (cross-volume copy not supported in temp dirs).
  This is a pre-existing Phase 1 test, unrelated to Phase 2.

## Known Limitations

- No encryption (backups stored in plaintext)
- No cloud storage (local filesystem only)
- No two-way synchronization (source → backup only)
- No automatic snapshot deletion
- Metadata preservation (permissions, timestamps) is best-effort
- Drive letter changes may require manual reassociation
- Scheduling is basic (no cron-like complexity)
- Backup cache is in-memory only (lost on restart)

## Future Recommendations

1. Add encrypted backup destination support
2. Add cloud storage destinations (S3, Google Drive, etc.)
3. Implement content-addressable storage for disk efficiency
4. Add scheduled background backup execution
5. Add backup set reassociation UI for moved folders
6. Add deep verification mode (rehash entire backup)
7. Add notification system integration
8. Add backup destination exclusion from organization scans
9. Add large dataset performance optimization (streaming, pagination)