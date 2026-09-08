# Backup Storage

FileHarbor Phase 2 — Storage Layout and Design

## Destination Layout

A FileHarbor backup destination contains:

```
FileHarbor Backups/
├── .fileharbor/
│   ├── destination.json      # Destination identity
│   └── metadata/             # Internal metadata (future use)
│
├── Documents Backup/         # Backup set name
│   ├── report.pdf            # File copies (flat layout)
│   ├── thesis.docx
│   └── snapshots/
│       ├── snapshot-2026-08-28T120000/
│       │   └── manifest.json # Per-snapshot manifest
│       └── snapshot-2026-08-29T120000/
│           └── manifest.json
│
└── Photos Backup/
    ├── IMG_001.jpg
    └── snapshots/
        └── snapshot-2026-08-28T120000/
            └── manifest.json
```

## Storage Strategy

Phase 2 uses a **flat file layout** with per-snapshot manifests. This is
simpler and more reliable than content-addressable storage, at the cost
of some disk space.

### Tradeoff

| Approach | Pros | Cons |
|---|---|---|
| Flat files + manifests | Simple, reliable, easy to inspect | Duplicate content stored multiple times |
| Content-addressable storage | Saves disk space for duplicate content | More complex, harder to debug |

Phase 2 chooses the simpler approach. The spec explicitly allows this:

> Do not implement complex deduplicated storage at the expense of reliability.
> If a simpler snapshot strategy is substantially safer for Phase 2, choose
> the safer design.

## Manifest Format

Every snapshot has a machine-readable manifest:

```json
{
  "schemaVersion": 1,
  "backupSetId": "backup-001",
  "snapshotId": "snapshot-2026-08-28-001",
  "createdAt": "2026-08-28T12:00:00Z",
  "status": "verified",
  "source": {
    "label": "Documents",
    "path": "C:\\Users\\User\\Documents"
  },
  "files": [
    {
      "relativePath": "report.pdf",
      "sizeBytes": 1048576,
      "sha256": "abc123...",
      "verified": true
    }
  ]
}
```

## Manifest Integrity

- Manifests are written atomically (temp file + rename)
- Schema versioning allows future format changes
- A corrupted manifest does not mean data is lost — the index can be
  rebuilt from manifests in the destination

## SQLite Index

The local SQLite database stores:

- `backup_sets` — backup set configurations
- `backup_destinations` — destination identity records
- `backup_runs` — run metadata (status, counts, timing)
- `backup_snapshots` — snapshot metadata (status, file count, size)
- `backup_files` — per-file records (path, hash, status)

SQLite stores metadata only, never file contents.

## Recovery

If the local SQLite database is lost:

1. FileHarbor can scan a backup destination for `.fileharbor/` markers
2. Read manifests from each snapshot directory
3. Rebuild the SQLite index from manifests
4. The `Rebuild Backup Index` command supports this

The backup destination remains independently understandable even without
the local database.

## Disk Space

- Initial backup copies all files
- Incremental backups copy only new and changed files
- Unchanged files are verified by hash but not recopied
- Disk space estimation is performed before starting a backup
- Insufficient space is detected before any files are copied