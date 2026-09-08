# Backup Architecture

FileHarbor Phase 2 — Backup & Protection System

## Overview

Phase 2 adds a reliable local backup system to FileHarbor. It is built on
top of the Phase 1 filesystem foundation and adds:

- Backup sets (named source + destination configurations)
- Initial and incremental backups
- SHA-256 verification of every copied file
- Snapshot versioning with conservative retention
- Safe restore with conflict resolution
- Backup health dashboard
- Interrupted backup detection and recovery

## Core Principle

> A backup is not considered successful until the destination copy has been verified.

Every file copied to a backup destination is hashed before and after the
copy, and the snapshot is only committed when all verifications pass.

## Module Layout

```
backup/
├── mod.rs          # Public API, CRUD, health computation
├── models.rs       # All data structures and enums
├── errors.rs       # BackupError and ErrorClassification
├── destination.rs  # Destination validation and identity
├── planner.rs      # Diff source vs. latest snapshot
├── engine.rs       # Backup orchestration (plan → copy → verify → commit)
├── verifier.rs     # Backup integrity checking
├── restore.rs      # Restore with conflict resolution
├── retention.rs    # Snapshot retention policies
├── manifest.rs     # Portable JSON manifests
├── scheduler.rs    # Daily/Weekly/DriveConnected scheduling
├── cache.rs        # In-memory hash cache (optimization only)
├── discovery.rs    # .fileharbor marker detection
├── logger.rs       # Structured in-memory logging
├── export_import.rs # Portable backup metadata export/import
└── machine.rs      # Volume identity for drive-letter independence
```

## Data Flow

```
FileRecord (from Phase 1 scan)
    ↓
Backup Planner
    ├── New Files
    ├── Changed Files
    ├── Unchanged Files
    ├── Deleted Files
    └── Unavailable Files
    ↓
Backup Engine
    ├── Copy (to .fileharbor-partial)
    ├── Verify (size + SHA-256)
    └── Atomic rename to final path
    ↓
Verification
    ↓
Backup Record (SQLite + manifest)
```

## Storage Model

Backup data is stored in two places:

1. **SQLite database** (`fileharbor.db`) — metadata index:
   - `backup_sets` — named configurations
   - `backup_destinations` — destination identity
   - `backup_runs` — one execution of a backup set
   - `backup_snapshots` — point-in-time state
   - `backup_files` — per-file records in a snapshot

2. **Backup destination filesystem** — actual file copies:
   - `.fileharbor/destination.json` — destination identity
   - `<backup set name>/` — files copied directly (flat layout)
   - `<backup set name>/snapshots/<label>/manifest.json` — per-snapshot manifest

The SQLite database is an index. If it is lost, manifests in the backup
destination can be used to rebuild the index.

## Incremental Algorithm

The planner compares the current source scan against the last completed
snapshot:

1. Files present in source but not in snapshot → **New**
2. Files in both, but SHA-256 differs → **Changed**
3. Files in both, SHA-256 matches → **Unchanged** (not recopied)
4. Files in snapshot but not in source → **Deleted**
5. Files that fail to read or hash → **Unavailable**

Only new and changed files are physically copied. Unchanged files are
verified by hash but not recopied.

## Integrity Model

- Every file is identified by `size + SHA-256`, never by path or name.
- The existing Phase 1 streaming SHA-256 hasher is reused (256 KiB chunks,
  cancellation checks, unstable-file detection).
- Copies are written to `.fileharbor-partial`, then atomically renamed
  after verification. Partial copies are never visible as backed-up.
- The manifest is written atomically (temp file + rename).

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

## Safety Guarantees

- Source files are never modified, moved, or deleted by backup operations.
- Backup copies are never offered for duplicate cleanup.
- Backup destinations are validated before use (not inside source, not the
  source itself, writable, enough space).
- Deleting a backup set does not automatically delete backup data unless
  the user explicitly chooses to.
- Retention policies default to "keep all".

## Limitations

- No encryption (Phase 2 stores backups in plaintext).
- No cloud storage (local filesystem destinations only).
- No two-way synchronization (source → backup only).
- No automatic snapshot deletion (requires explicit configuration).
- Metadata preservation (permissions, timestamps) is best-effort and
  platform-dependent.