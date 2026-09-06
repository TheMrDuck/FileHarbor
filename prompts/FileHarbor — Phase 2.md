# FileHarbor — Phase 2
# Backup & Protection System
## Complete Engineering, Architecture, Safety & QA Specification

You are a senior systems engineer specializing in:

- Rust filesystem software
- Desktop backup applications
- Data integrity
- Incremental backup systems
- Filesystem safety
- Cross-platform desktop applications
- SQLite data modeling
- Tauri 2
- React/TypeScript
- Failure recovery
- Cryptographic hashing
- Large-scale filesystem operations

You are working on **FileHarbor**, a production-ready desktop file organization application.

Phase 1 is COMPLETE and has been manually tested.

Your job is to implement **Phase 2 — Backup & Protection** without destabilizing or unnecessarily rewriting the completed Phase 1 system.

---

# 1. PRODUCT CONTEXT

FileHarbor is evolving from a file organizer into a broader personal file-management and protection application.

The product direction is:

```text
                    FILEHARBOR
                        │
          ┌─────────────┴─────────────┐
          │                           │
     ORGANIZATION                 PROTECTION
          │                           │
     Scan / Analyze                 Backup
     Duplicates                     Verify
     Empty files                    Versions
     Quarantine                     Restore
     File identity                  Health
```

Product tagline:

> **FileHarbor — Organize. Protect. Keep what matters.**

Phase 1 already provides the trusted filesystem foundation.

Phase 2 must build on that foundation.

---

# 2. EXISTING PHASE 1 — DO NOT BREAK IT

The existing architecture includes:

```text
src/
├── React/TypeScript frontend
└── Tauri integration

src-tauri/
├── crates/
│   ├── organizer-core/
│   └── test-data-generator/
└── src/
```

The Rust core currently provides:

- filesystem scanning
- metadata discovery
- candidate grouping
- streaming SHA-256 hashing
- exact duplicate detection
- SQLite persistence
- quarantine
- restore
- permanent deletion safeguards
- operation history
- exclusions
- settings
- cancellation
- symlink safety

Phase 1 currently has:

```text
organizer-core:       28/28 tests passing
test-data-generator: 10/10 tests passing
Total:                38/38 tests passing
```

Manual testing has also been completed.

Do not rewrite working Phase 1 functionality merely for stylistic reasons.

Do not replace the scanner unless Phase 2 genuinely requires an abstraction change.

Do not remove existing safety guarantees.

---

# 3. PRIMARY PHASE 2 OBJECTIVE

Implement a reliable local backup system that allows users to:

1. Select files/folders to protect.
2. Select a backup destination.
3. Create an initial backup.
4. Create subsequent incremental backups.
5. Detect changed/new/deleted files.
6. Verify backup integrity.
7. See which files are protected.
8. See which files are unprotected or outdated.
9. Restore files from backups.
10. Maintain backup history.
11. Handle interruptions and failures safely.
12. Avoid unnecessary copying.
13. Never silently overwrite user files.
14. Never silently delete user files.
15. Clearly distinguish backup copies from disposable duplicates.

The backup system must be designed around one principle:

> **A backup is not considered successful until the destination copy has been verified.**

---

# 4. CRITICAL DESIGN PRINCIPLE

## Duplicate ≠ Disposable

This is one of the most important requirements in Phase 2.

Suppose:

```text
C:\Users\User\Documents\report.pdf
D:\FileHarbor Backups\Documents\report.pdf
```

have the same SHA-256.

The duplicate engine may correctly identify:

```text
same content
```

But the backup system must understand:

```text
Working Copy
+
Protected Backup Copy
```

The backup copy must NEVER automatically be classified as something the duplicate-cleanup system should delete.

Introduce an explicit concept of:

```text
Copy Role
```

where appropriate:

```text
working
backup
quarantine
unknown
```

Do not modify the existing duplicate semantics:

> Phase 1 duplicate detection remains based on size + SHA-256.

The backup subsystem adds protection context separately.

---

# 5. BACKUP ARCHITECTURE

Create a dedicated backup layer.

Conceptually:

```text
                  FileRecord
                     │
                     ↓
              Backup Planner
                     │
          ┌──────────┼──────────┐
          ↓          ↓          ↓
       New Files   Changed    Deleted
          │          │          │
          └──────────┼──────────┘
                     ↓
               Backup Engine
                     │
                     ↓
              Transfer Layer
                     │
                     ↓
               Verification
                     │
                     ↓
              Backup Record
```

Suggested Rust architecture:

```text
organizer-core/
├── backup/
│   ├── mod.rs
│   ├── models.rs
│   ├── planner.rs
│   ├── engine.rs
│   ├── verifier.rs
│   ├── destination.rs
│   ├── manifest.rs
│   ├── restore.rs
│   ├── retention.rs
│   └── errors.rs
```

Adapt this to the existing project conventions rather than blindly following the exact filenames.

---

# 6. BACKUP TERMINOLOGY

Use consistent terminology throughout the code and UI.

### Backup Source

The original user-selected file or folder.

### Backup Destination

The location where FileHarbor stores the protected copy.

### Backup Set

A named configuration describing:

```text
source
destination
schedule
retention
options
```

### Backup Run

One execution of a backup set.

### Backup Item

A single file tracked by a backup.

### Backup Snapshot

The state of a backup set at a particular point in time.

### Verified

A destination copy whose integrity has been confirmed.

### Outdated

A source file has changed since its last verified backup.

### Protected

A source file has at least one valid verified backup copy.

---

# 7. BACKUP SETS

Users should be able to create named backup sets.

Example:

```text
Documents Backup
```

Configuration:

```text
Source:
C:\Users\User\Documents

Destination:
D:\FileHarbor Backups\Documents

Mode:
Incremental

Verification:
SHA-256

Schedule:
Manual
```

Later:

```text
Photos Backup
Projects Backup
University Backup
```

Do not force users to configure complicated enterprise backup policies.

Keep the initial UX understandable.

---

# 8. BACKUP DESTINATIONS

Phase 2 should support local filesystem destinations.

At minimum:

- Another internal drive
- External HDD
- External SSD
- User-selected folder
- Network-mounted folder if the operating system exposes it as a normal filesystem path

Do NOT implement cloud storage in this phase.

Cloud providers can be a later phase.

---

# 9. DESTINATION VALIDATION

Before allowing a backup:

Validate:

- destination exists or can safely be created
- destination is writable
- destination is not the source itself
- destination is not inside the source
- destination does not resolve to the source through symlinks/junctions where detectable
- enough disk space exists where reliably measurable
- destination is accessible
- destination has a valid FileHarbor backup identity

Reject dangerous configurations.

Example:

```text
Source:
C:\Users\User\Documents

Destination:
C:\Users\User\Documents\Backup
```

Must be rejected because the destination is inside the source.

Also reject:

```text
Source:
D:\Backup

Destination:
D:\
```

if that would cause recursive backup behavior.

---

# 10. BACKUP DESTINATION IDENTITY

Do not rely only on the destination path.

Create a FileHarbor backup identity for destinations.

For example:

```text
backupDestinationId
```

and persist metadata identifying:

- destination ID
- created timestamp
- FileHarbor version
- backup schema version
- optional machine identity where appropriate
- human-readable destination label

The destination should contain a FileHarbor-managed metadata structure.

For example:

```text
FileHarbor Backups/
└── .fileharbor/
    ├── destination.json
    └── ...
```

Do not store sensitive information unnecessarily.

---

# 11. BACKUP STORAGE LAYOUT

Design a clear, durable layout.

Example:

```text
FileHarbor Backups/
├── .fileharbor/
│   ├── destination.json
│   └── metadata/
│
├── Documents Backup/
│   ├── snapshots/
│   │   ├── 2026-08-28T120000/
│   │   └── 2026-08-29T120000/
│   │
│   └── current/
│
└── Photos Backup/
    ├── snapshots/
    └── current/
```

However, do NOT blindly duplicate the entire file tree for every snapshot.

Choose a storage strategy that avoids unnecessary copies.

The implementation must prioritize:

- correctness
- recoverability
- understandable structure
- efficient incremental backups

---

# 12. INITIAL BACKUP

When a backup set runs for the first time:

```text
Scan source
    ↓
Analyze
    ↓
Build backup plan
    ↓
Show summary
    ↓
User confirms
    ↓
Copy files
    ↓
Verify
    ↓
Commit backup snapshot
```

Example UI:

```text
Create Backup

Documents

Files:
2,431

Total:
18.7 GB

New:
2,431

Changed:
0

Estimated transfer:
18.7 GB

Destination:
External SSD (D:)

[Cancel] [Start Backup]
```

---

# 13. INCREMENTAL BACKUPS

Subsequent backups must NOT blindly copy everything.

For each source file determine:

```text
New
Changed
Unchanged
Deleted
Unavailable
```

Example:

```text
Documents Backup

New files:       24
Changed files:   87
Unchanged:     2,320
Deleted:         12

Data to transfer:
842 MB
```

The backup planner must use reliable file identity information.

---

# 14. FILE CHANGE DETECTION

Use metadata as an optimization, not as the final integrity authority.

Possible fast-path identity:

```text
path
size
mtime
```

If unchanged, avoid unnecessary hashing/copying where safe.

But when metadata indicates a possible change:

```text
recalculate SHA-256
```

Do not assume:

```text
same filename
+
same size
+
same timestamp
```

always means identical content.

The architecture should permit hash verification where needed.

---

# 15. HASH-BASED BACKUP IDENTITY

The canonical content identity remains:

```text
size + SHA-256
```

This is consistent with Phase 1.

Do not use filename as identity.

Do not use timestamps as identity.

Do not use size alone.

---

# 16. REUSING EXISTING HASHING

Reuse the existing streaming SHA-256 implementation where practical.

Existing behavior includes:

- 256 KiB chunks
- cancellation checks
- detection of files changing during hashing
- unstable hash discarded

Do not implement a second incompatible hashing implementation without a strong reason.

If the backup verifier needs a slightly different interface, abstract the existing hasher rather than duplicating logic.

---

# 17. UNSTABLE SOURCE FILES

If a file changes during backup preparation or hashing:

```text
Status:
Unstable
```

Do NOT create a backup and mark it verified using an inconsistent hash.

Instead:

```text
Backup skipped

report.docx changed while being read.

Reason:
Source file was unstable.

[Retry]
```

The original source must remain untouched.

---

# 18. COPY + VERIFY

Every copied file must follow:

```text
Source
 ↓
Temporary destination
 ↓
Copy
 ↓
Flush/close as appropriate
 ↓
Verify size
 ↓
Verify SHA-256
 ↓
Atomically promote to final backup location
```

Do NOT expose partially copied files as successfully backed-up files.

Use a temporary filename/extension such as:

```text
.fileharbor-partial
```

or another clearly internal mechanism.

---

# 19. ATOMIC BACKUP COMMIT

A backup snapshot must not be marked complete until every required item has reached its intended verified state.

For example:

```text
RUNNING
 ↓
VERIFYING
 ↓
COMMITTING
 ↓
COMPLETED
```

Failure:

```text
RUNNING
 ↓
FAILED
```

or:

```text
RUNNING
 ↓
PARTIAL
```

depending on the failure model.

Never report:

```text
Backup complete
```

when verification has not completed.

---

# 20. INTERRUPTED BACKUPS

Handle:

- application crash
- computer shutdown
- external drive disconnect
- network disconnect
- user cancellation
- insufficient disk space
- source file disappearance
- destination permission failure

Temporary files must be recoverable or safely cleaned up.

On next startup:

```text
Incomplete backup detected.

FileHarbor found an interrupted backup from:
August 28, 2026 14:32

[Resume]
[Discard Incomplete Run]
[Review]
```

Do not automatically delete potentially useful partial data without determining its safety.

---

# 21. USER CANCELLATION

Cancellation must be cooperative.

If the user presses:

```text
Cancel Backup
```

stop starting new files and safely finish/rollback the current operation where appropriate.

The application should report:

```text
Backup cancelled

Completed:
1,842 files

Verified:
1,842

Not processed:
589

Original files:
Untouched
```

Never delete source files as part of backup cancellation.

---

# 22. EXTERNAL DRIVE DISCONNECTION

If a destination disappears during backup:

```text
Backup interrupted

Destination is no longer available.

Verified:
1,240 files

Unverified:
317 files

Source files:
Untouched
```

The backup run must not be marked successful.

When the destination becomes available again, allow retry.

---

# 23. SOURCE FILE DELETED DURING BACKUP

If a source disappears:

```text
report.pdf
Status:
Unavailable during backup
```

Do not interpret this as:

```text
delete backup copy
```

The backup copy must remain intact.

The user can decide later whether the source's deletion should be reflected in backup history.

---

# 24. BACKUP DELETIONS

Phase 2 must be conservative about deletions.

If a source file is deleted:

```text
Source:
report-old.pdf
```

do NOT automatically delete the backup copy.

Instead:

```text
Backup copy still exists

Source:
Deleted

Backup:
Available

[Keep Backup]
[Remove From Backup]
```

Future retention policies may automate this only after explicit user configuration.

---

# 25. BACKUP VERSIONING

Implement a basic snapshot/version model.

A user should be able to see:

```text
Documents Backup

Today
✓ 2,431 files

Yesterday
✓ 2,417 files

Aug 21
✓ 2,390 files
```

A snapshot records the logical state of the backup at that point.

Do not require a complete physical duplicate of every unchanged file for every snapshot.

Use deduplication/reuse where practical.

---

# 26. CONTENT-ADDRESSABLE STORAGE

Where practical, use content identity to avoid storing duplicate physical backup data.

Conceptually:

```text
SHA-256
   ↓
content object
```

Example:

```text
objects/
├── ab/
│   └── abc123...
├── 91/
│   └── 91de4...
```

Snapshots can reference content objects.

This allows:

```text
report.pdf
report-final.pdf
report-backup.pdf
```

with identical content to share one stored content object.

However:

> Do not implement complex deduplicated storage at the expense of reliability.

If a simpler snapshot strategy is substantially safer for Phase 2, choose the safer design.

Document the tradeoff.

---

# 27. BACKUP MANIFESTS

Every backup set/snapshot must have a machine-readable manifest.

Example:

```json
{
  "schemaVersion": 1,
  "backupSetId": "backup-001",
  "snapshotId": "snapshot-2026-08-28-001",
  "createdAt": "2026-08-28T12:00:00Z",
  "status": "verified",
  "source": {
    "label": "Documents"
  },
  "files": [
    {
      "relativePath": "report.pdf",
      "sizeBytes": 1048576,
      "sha256": "...",
      "verified": true
    }
  ]
}
```

The manifest must never claim verification before verification actually occurs.

---

# 28. MANIFEST INTEGRITY

Protect backup metadata from accidental corruption.

Use:

- atomic writes
- temporary files
- validation before commit
- schema versioning

Do not silently overwrite a valid manifest with malformed data.

If a manifest becomes corrupted:

```text
Backup metadata requires repair.

Data files may still be intact.

[Inspect]
[Rebuild Index]
```

Design the system so backup files are not immediately considered lost because an index/manifest is damaged.

---

# 29. SQLITE BACKUP METADATA

Extend the existing SQLite database rather than introducing a second unrelated database unless there is a compelling architectural reason.

Potential models:

```text
BackupSet
BackupDestination
BackupRun
BackupSnapshot
BackupFile
BackupContent
```

Adapt names to existing conventions.

Relationships should support:

```text
BackupSet
  ├── source
  ├── destination
  └── many BackupRuns

BackupRun
  └── BackupSnapshot

BackupSnapshot
  └── many BackupFiles

BackupFile
  ├── relative path
  ├── size
  ├── SHA-256
  ├── status
  └── content reference
```

Use indexes for:

- backup set
- destination
- snapshot
- path
- hash
- status

Do not duplicate massive file content in SQLite.

SQLite should store metadata/index information, not backup file bytes.

---

# 30. BACKUP STATUS MODEL

Each tracked source file should be able to have a protection status such as:

```text
protected
outdated
never_backed_up
backup_missing
source_missing
unstable
error
```

Example:

```text
report.pdf
🟢 Protected

Last verified:
Today 14:32
```

```text
thesis.docx
🟡 Backup outdated

Local:
Modified today

Backup:
Yesterday
```

```text
project.zip
🔴 Not backed up
```

---

# 31. BACKUP HEALTH DASHBOARD

Add a dedicated:

```text
Backup Health
```

view.

Example:

```text
BACKUP HEALTH

Protected
████████████████░░░░ 82%

1,842 protected
241 outdated
109 never backed up

────────────────────────

Documents          🟢
Photos             🟢
Projects           🟡
Downloads          🔴
```

The dashboard should prioritize actionable information.

---

# 32. PROTECTION SCORE

If implementing a score, make it transparent.

Do NOT produce a mysterious AI-style score.

Example:

```text
Protection: 82%

Why?

✓ 1,842 files have verified backups
⚠ 241 files changed since backup
✗ 109 files have no backup
```

Users should always be able to understand the calculation.

---

# 33. FILE PROTECTION VIEW

On the existing All Files page, where appropriate, add:

```text
Backup Status
```

Example:

| File | Size | Modified | Backup |
|---|---:|---|---|
| report.pdf | 2 MB | Today | 🟢 Protected |
| thesis.docx | 4 MB | Today | 🟡 Outdated |
| notes.txt | 12 KB | Today | 🔴 None |

Do not clutter the Phase 1 interface unnecessarily.

---

# 34. BACKUP PAGE

Add a dedicated backup page.

Suggested structure:

```text
Backups

[Create Backup]

────────────────────────

Backup Sets

Documents
🟢 Protected
Last verified: Today

Photos
🟢 Protected
Last verified: Yesterday

Projects
🟡 17 files outdated

────────────────────────

Recent Backup Activity

✓ Documents — 842 MB
✓ Photos — 4.2 GB
⚠ Projects — 3 files failed
```

---

# 35. CREATE BACKUP WIZARD

Create a simple workflow:

### Step 1 — Source

```text
What do you want to protect?

[Choose Folder]
```

### Step 2 — Destination

```text
Where should FileHarbor keep the backup?

[Choose Destination]
```

### Step 3 — Review

```text
Source:
Documents

Destination:
External SSD

Files:
2,431

Size:
18.7 GB
```

### Step 4 — Backup

```text
Backing up...

1,823 / 2,431

Transferred:
14.2 GB

Verified:
1,817

Current:
report.pdf
```

### Step 5 — Complete

```text
Backup verified

2,431 files
18.7 GB
0 failures

✓ Your backup is verified
```

---

# 36. BACKUP PREVIEW

Before starting a backup, show:

```text
Backup Preview

New:
24

Changed:
87

Unchanged:
2,320

Source missing:
12

Estimated transfer:
842 MB
```

The user must be able to understand what FileHarbor is about to do.

---

# 37. NO SILENT DESTRUCTIVE ACTIONS

Backup functionality must never:

- delete source files
- move source files
- modify source content
- rename source files
- overwrite existing unrelated destination files
- delete previous snapshots automatically

unless the user has explicitly configured a retention/deletion policy.

Phase 2 should default to conservative behavior.

---

# 38. RESTORE FROM BACKUP

Implement safe restore.

User chooses:

```text
Restore

report.pdf
Backup:
Aug 28, 2026

Destination:
Original location

[Restore]
```

Before restoring:

```text
Restore 17 files?

Existing files:
3

Missing files:
14

[Keep Existing]
[Create New Name]
[Cancel]
```

Never silently overwrite existing files.

Reuse or adapt the proven Phase 1 restore conflict system.

---

# 39. RESTORE TO ALTERNATE LOCATION

Allow:

```text
Restore to original location
Restore to another folder
```

This is important for recovery and testing.

---

# 40. RESTORE VERIFICATION

After restoring:

```text
Restore completed

17 files restored
17 verified

✓ SHA-256 matches backup
```

A restored file must be verified where practical.

---

# 41. PARTIAL RESTORE FAILURE

If:

```text
17 requested
15 restored
2 failed
```

report exactly that.

Do not claim complete success.

Provide:

```text
[Retry Failed]
```

where appropriate.

---

# 42. BACKUP OPERATION HISTORY

Integrate with existing operation history.

Record:

```text
Backup started
Backup completed
Backup partially failed
Backup cancelled
Backup verification failed
Backup restored
Backup destination unavailable
```

Each operation should include:

- timestamp
- backup set
- run ID
- item count
- bytes transferred
- success/failure counts
- relevant errors

Do not flood the database with unnecessary per-chunk events.

---

# 43. RETRY MODEL

Implement retry at the file level where practical.

Example:

```text
Backup completed with warnings

2 files failed.

report.pdf
Permission denied

video.mp4
Destination unavailable

[Retry Failed Files]
```

Do not require rerunning the entire backup.

---

# 44. ERROR CLASSIFICATION

Create useful error categories.

Examples:

```text
SourceMissing
SourceChanged
SourceUnreadable
DestinationUnavailable
DestinationReadOnly
InsufficientSpace
PermissionDenied
TransferFailed
VerificationFailed
Cancelled
ManifestError
DatabaseError
Unknown
```

Do not expose raw Rust/OS errors as the primary UI message.

Show a human-readable message and preserve technical details for diagnostics.

---

# 45. DISK SPACE ESTIMATION

Before starting a backup:

Calculate estimated transfer size.

If available space is insufficient:

```text
Not enough space

Required:
18.7 GB

Available:
11.2 GB

Nothing has been copied yet.

[Choose Another Destination]
```

If space becomes insufficient mid-backup:

```text
Backup paused

Destination ran out of space.

Verified:
12.4 GB

Remaining:
6.3 GB

Source files:
Untouched
```

---

# 46. SCHEDULING

Implement basic scheduling infrastructure only if it fits the existing desktop architecture safely.

Support:

```text
Manual
Daily
Weekly
```

Also support:

```text
When external drive is connected
```

if reliable cross-platform detection can be implemented.

Do not build a complicated cron-like scheduler.

---

# 47. BACKGROUND BACKUPS

Backups should not freeze the UI.

The UI must remain responsive while:

- scanning
- hashing
- copying
- verifying

Use the existing asynchronous/Tauri command architecture appropriately.

---

# 48. CONCURRENCY

Reuse or extend the existing concurrency configuration.

Do not blindly maximize parallelism.

Backup concurrency should consider:

- HDD performance
- SSD performance
- external USB drives
- network destinations
- memory
- CPU hashing load

Provide a sensible default.

Allow configuration later if appropriate.

---

# 49. IO PRIORITY

Do not starve the rest of the application.

If practical, allow the backup engine to operate at a controlled concurrency level.

Do not spawn one OS thread per file.

Use bounded worker pools.

---

# 50. PROGRESS REPORTING

Show:

```text
Backup progress

1,823 / 2,431 files

Transferred:
14.2 GB / 18.7 GB

Verified:
1,817

Failed:
0

Current:
photos/vacation/img_1823.jpg

Speed:
128 MB/s

ETA:
00:42
```

Do not update the frontend thousands of times per second.

Throttle progress updates.

---

# 51. PAUSE / RESUME

Phase 1 only supported cancellation.

Phase 2 may introduce pause/resume for backups.

If implemented:

```text
[Pause Backup]
```

must safely stop starting new work.

Resume should continue from known state rather than restarting everything.

If pause/resume significantly complicates correctness, prioritize reliable cancellation and resumable backup runs over a premature UI feature.

---

# 52. BACKUP CACHE

Reuse existing Phase 1 hash caching where safe.

But be explicit:

> Cache is an optimization, never the source of truth.

If:

```text
path + size + mtime
```

matches cached state, it may avoid rehashing.

But verification must be able to recalculate the hash when required.

---

# 53. BACKUP VERIFICATION MODES

Initially support:

```text
Standard
```

which verifies transferred files.

Potentially expose later:

```text
Deep Verification
```

which rehashes the entire backup.

Do not require full backup rehashing after every run unless configured.

---

# 54. BACKUP INTEGRITY CHECK

Provide:

```text
Verify Backup
```

for an existing backup.

Example:

```text
Verifying Documents Backup...

Files checked:
2,431

Valid:
2,429

Problems:
2

report.pdf
Hash mismatch

archive.zip
Missing

Backup status:
⚠ Needs attention
```

This should be independent from the normal backup operation.

---

# 55. BACKUP HEALTH CHECK

Allow users to manually run:

```text
[Verify Backup]
```

against an entire backup set.

This should:

1. Read backup metadata.
2. Enumerate expected files.
3. Check existence.
4. Check size.
5. Recalculate hashes where required.
6. Detect missing files.
7. Detect unexpected files where relevant.
8. Report discrepancies.
9. Never modify the backup.

---

# 56. BACKUP CORRUPTION HANDLING

If a backup file's hash does not match:

```text
Backup integrity problem

File:
Documents/report.pdf

Expected SHA-256:
...

Actual SHA-256:
...

Status:
❌ Corrupted / modified

Recommended action:
Create a new verified backup from the source if available.
```

Do not silently replace it during verification.

---

# 57. EXTERNAL MODIFICATION

If someone modifies a file inside a FileHarbor-managed backup destination:

Detect it during verification.

Mark:

```text
modified-outside-fileharbor
```

Do not automatically trust the modified copy.

Do not automatically overwrite it without user-visible behavior.

---

# 58. BACKUP DESTINATION SAFETY

FileHarbor should recognize:

```text
This folder contains FileHarbor backup data.
```

If the user tries to select it as a normal source for organization:

Warn appropriately.

Do not allow duplicate-cleanup workflows to blindly destroy protected backup data.

---

# 59. QUARANTINE INTERACTION

Phase 2 must integrate carefully with Phase 1 quarantine.

If the user quarantines a source file that has a verified backup:

```text
report.pdf

🟢 Backup exists

The original can be restored from:
External SSD

[Continue]
```

If there is no backup:

```text
report.pdf

🔴 No verified backup

This file is not protected.

[Cancel]
```

Do not automatically block Phase 1 functionality unless the existing product design calls for a safety warning.

---

# 60. DELETE SAFETY + BACKUP AWARENESS

When a user attempts to permanently delete a file from quarantine:

If a verified backup exists:

```text
Backup available

This file has a verified backup copy.

Permanent deletion affects only the quarantined copy.

[Delete]
```

If no backup exists:

```text
⚠ No verified backup

This may be your only remaining copy.

[Cancel]
[Delete Anyway]
```

This should be a safety warning, not an automatic prohibition, unless explicitly configured.

---

# 61. 3-2-1 READINESS

Do not implement full cloud/off-site backup yet.

But architect the system so future destinations can include:

```text
Local
External
Network
Cloud
```

The UI may eventually explain:

```text
3-2-1 Backup

3 copies
2 storage types
1 off-site
```

Do not pretend Phase 2 provides 3-2-1 protection if the user has only one backup copy.

---

# 62. MACHINE / DRIVE REPLACEMENT

Avoid binding backup identity solely to:

```text
C:
D:
E:
```

Drive letters can change.

Prefer destination identity + filesystem path.

Where supported, use stable volume identity carefully.

Do not make backups unusable merely because an external drive changes from:

```text
E:
```

to:

```text
F:
```

---

# 63. MOVED SOURCE FOLDER

If a backup source moves:

```text
C:\Users\User\Documents
```

to:

```text
D:\Documents
```

do not immediately create a completely unrelated backup.

Allow the user to reassociate the backup set after validation.

Potential workflow:

```text
Existing backup set found.

Previous source:
C:\Users\User\Documents

New source:
D:\Documents

FileHarbor found matching files.

[Reconnect Backup Set]
[Create New Backup Set]
```

This can be implemented if it does not overcomplicate Phase 2.

---

# 64. PATH HANDLING

Use relative paths inside backup snapshots.

Never reconstruct source paths by concatenating unsafe strings.

Protect against:

```text
..
absolute paths
drive prefixes
UNC path confusion
symlink escape
```

When restoring:

```text
destinationRoot + relativePath
```

must be validated to remain within the selected restore destination.

---

# 65. SYMLINK POLICY

Follow Phase 1 safety policy.

Do not blindly follow symlinks or junctions.

For Phase 2, choose and document a policy such as:

```text
Skip symlinks by default
```

or:

```text
Back up symlink metadata where safely supported
```

Do not accidentally copy the contents of an external tree through a symlink.

---

# 66. SPECIAL FILES

Do not attempt to back up:

- sockets
- device files
- unsupported filesystem objects

unless explicitly supported.

Mark them:

```text
Unsupported
```

rather than failing the entire backup.

---

# 67. FILE TYPES

Backup must be extension-agnostic.

Support arbitrary regular files:

```text
pdf
docx
jpg
png
mp4
zip
exe
bin
dat
etc.
```

Do not inspect file extensions to decide whether something is safe to back up.

---

# 68. NO CONTENT MODIFICATION

FileHarbor must never:

- recompress
- resize
- rewrite
- normalize
- alter metadata unnecessarily

during backup.

A backup should preserve the source bytes exactly.

---

# 69. FILE PERMISSIONS / METADATA

Phase 2 should initially prioritize content integrity.

If cross-platform metadata preservation is implemented, clearly document it.

Do not falsely claim full metadata preservation across operating systems.

At minimum preserve:

```text
file contents
relative path
size
SHA-256
```

Potentially preserve:

```text
mtime
permissions
```

only where reliable.

---

# 70. DATABASE MIGRATION

Inspect the existing SQLite schema.

Add backup tables through a proper migration strategy.

Do not destroy existing Phase 1 data.

Existing sessions, file records, duplicate groups, quarantine items, operations, exclusions, and settings must remain intact.

Test migration from:

```text
Phase 1 database
```

to:

```text
Phase 2 database
```

with existing data present.

---

# 71. BACKWARD COMPATIBILITY

Opening an existing Phase 1 installation must not:

- lose scan sessions
- lose duplicate results
- lose quarantine records
- lose operation history
- reset settings

Backup tables should be additive.

---

# 72. FRONTEND API

Add Tauri commands following existing conventions.

Potential commands:

```text
create_backup_set
list_backup_sets
get_backup_set
update_backup_set
delete_backup_set

plan_backup
start_backup
cancel_backup
pause_backup
resume_backup

get_backup_progress
get_backup_runs
get_backup_snapshots

verify_backup
get_backup_health

restore_from_backup
retry_failed_backup_items

validate_backup_destination
```

Do not expose unnecessary low-level filesystem operations directly to the frontend.

Keep safety-critical logic in Rust.

---

# 73. FRONTEND STATE

Create a dedicated backup state layer.

Do not scatter backup state across unrelated components.

The frontend should track:

```text
backup sets
current plan
current run
progress
errors
verification
health
restore state
```

Reuse existing patterns for scan progress and cancellation where appropriate.

---

# 74. BACKUP ROUTES / PAGES

Add a dedicated:

```text
Backups
```

section.

Suggested pages/components:

```text
pages/
├── Backups
├── BackupDetails
├── BackupCreate
├── BackupRun
├── BackupHistory
└── BackupHealth
```

Adapt to the existing routing architecture.

---

# 75. UI DESIGN

FileHarbor's existing UI should remain visually consistent.

Do not introduce an unrelated design system.

Use the current:

- dark theme
- spacing
- typography
- cards
- dialogs
- progress indicators
- toast system
- keyboard accessibility

Backup UI should feel like part of FileHarbor.

---

# 76. ACCESSIBILITY

All new backup UI must support:

- keyboard navigation
- visible focus
- ARIA labels
- accessible dialogs
- screen-reader-friendly status updates
- sufficient contrast

Do not rely solely on color.

For example:

```text
🟢 Protected
```

should also have text:

```text
Protected
```

---

# 77. BACKUP STATUS COLORS

If colors are used:

```text
Protected
Outdated
At Risk
Failed
```

must remain understandable without color.

Do not communicate critical state only through green/yellow/red.

---

# 78. NOTIFICATION SYSTEM

Use existing toast/notification infrastructure.

Examples:

```text
Backup verified successfully.
```

```text
Backup completed with 3 warnings.
```

```text
Backup destination disconnected.
```

Avoid notification spam.

---

# 79. SETTINGS

Add backup settings where appropriate.

Possible:

```text
Default backup destination
Default verification mode
Default concurrency
Default schedule
Retention
Notifications
```

Do not add every advanced option immediately.

---

# 80. RETENTION

Implement a conservative basic retention model.

Potential options:

```text
Keep all snapshots
Keep last N snapshots
```

If deletion of old snapshots is implemented, it must:

- require explicit configuration
- show what will be deleted
- ensure newer snapshots remain valid
- never delete the only remaining copy without warning

Default:

```text
Keep all
```

is acceptable for Phase 2.

---

# 81. SNAPSHOT DELETION

Deleting a backup snapshot must be treated as destructive.

Require confirmation:

```text
Delete backup snapshot?

Date:
August 21, 2026

Files represented:
2,390

This may remove historical versions.

[Cancel]
[Delete Snapshot]
```

If content-addressable storage is used, garbage collection must be conservative.

Never delete a content object still referenced by another snapshot.

---

# 82. BACKUP SET DELETION

Deleting a backup set should NOT automatically delete backup data unless the user explicitly chooses:

```text
Delete backup configuration only
```

or:

```text
Delete configuration and backup data
```

Make the distinction explicit.

---

# 83. BACKUP EXPORT / IMPORT

Do not implement a complicated backup migration/export system unless required.

But design manifests so a future FileHarbor installation can potentially recognize backup data.

---

# 84. RECOVERY FROM DAMAGED DATABASE

The backup destination should contain enough metadata to reconstruct backup information if the local SQLite database is lost.

This is important.

The local database should be considered an index.

The backup destination should remain understandable independently.

Implement a future-compatible command such as:

```text
Rebuild Backup Index
```

if practical.

---

# 85. BACKUP DISCOVERY

If FileHarbor sees:

```text
.fileharbor/
```

on an external drive, it should be able to recognize:

```text
FileHarbor backup destination detected
```

rather than treating it as an arbitrary folder.

---

# 86. MULTIPLE COMPUTERS

Do not assume only one computer will ever access a backup destination.

Design manifests with enough identity information to distinguish:

```text
backup set
destination
snapshot
machine/source
```

without unnecessarily locking the backup to one device.

---

# 87. SECURITY

Backup data can contain extremely sensitive personal information.

Do not upload anything to external services.

Phase 2 must remain local.

Do not add telemetry containing:

- filenames
- file contents
- paths
- hashes
- backup contents

unless explicitly introduced later with user consent.

---

# 88. ENCRYPTION

Do NOT implement custom encryption casually.

If backup encryption is not already supported by a mature, appropriate dependency and architecture, leave encryption for a later dedicated phase.

Document:

```text
Phase 2 does not provide encrypted backup storage.
```

Do not claim otherwise.

---

# 89. PASSWORDS / KEYS

Never store plaintext encryption keys or passwords.

Since encryption is out of scope for Phase 2, do not introduce fake encryption settings.

---

# 90. LOGGING

Logs must be useful without leaking unnecessary sensitive information.

Avoid dumping:

```text
C:\Users\Amer\Documents\Private...
```

or complete personal filenames into logs unnecessarily.

Prefer:

```text
Backup item failed
error=PermissionDenied
```

with detailed information available only when explicitly requested.

---

# 91. TEST DATA GENERATOR EXTENSION

Extend the existing `test-data-generator` so it can create backup-specific fixtures.

Do not replace the existing generator.

Add scenarios such as:

```text
backup-baseline
backup-incremental
backup-changed-files
backup-deleted-files
backup-restore
backup-corruption
backup-interrupted
backup-large
backup-duplicate-content
```

---

# 92. BACKUP BASELINE TEST DATA

Example:

```text
source/
├── documents/
├── photos/
├── projects/
└── misc/
```

Generate:

```text
1,000 files
100 exact duplicate relationships
50 empty files
mixed Unicode names
large files
```

Create a clean initial backup.

---

# 93. INCREMENTAL TEST DATA

After initial backup:

Modify:

```text
100 files
```

Add:

```text
50 files
```

Delete:

```text
20 files
```

Keep:

```text
830 unchanged
```

The expected backup plan should be known.

Example manifest:

```text
Expected:
New:       50
Changed:  100
Deleted:   20
Unchanged: 830
```

---

# 94. BACKUP CORRUPTION FIXTURE

Create a verified backup.

Then intentionally modify one backup file.

The verification system should detect:

```text
Expected SHA-256 != Actual SHA-256
```

Do not use real user data.

---

# 95. BACKUP INTERRUPTION FIXTURE

Provide a controlled way to stop a backup midway.

The generator/test harness should make it possible to verify:

```text
partial run
temporary files
resume/retry
final verification
```

---

# 96. RESTORE FIXTURES

Create:

```text
backup/
source/
restore-target/
```

Test:

- restore missing file
- restore existing file
- conflict
- new-name resolution
- cancellation
- failed restore
- hash verification

---

# 97. EXACT DUPLICATE + BACKUP TEST

Create:

```text
source/report.pdf
source/final-report.pdf
backup/report.pdf
```

where all content is identical.

Expected:

```text
Phase 1:
Two source duplicates

Phase 2:
Backup copy is protected

No backup copy should be offered for cleanup.
```

This test is mandatory.

---

# 98. TEST SUITE

Add comprehensive unit tests.

### Backup planner

Test:

- new file
- unchanged file
- changed file
- deleted source
- missing source
- unstable source

### Transfer

Test:

- successful copy
- size mismatch
- hash mismatch
- destination unavailable
- cancellation
- partial failure

### Verification

Test:

- valid backup
- missing file
- modified file
- wrong size
- corrupted manifest

### Restore

Test:

- missing destination file
- conflict
- keep existing
- new name
- cancellation
- hash mismatch

### Destination validation

Test:

- valid destination
- source itself
- destination inside source
- read-only destination
- nonexistent destination
- backup destination recognition

---

# 99. INTEGRATION TESTS

At minimum:

```text
Create source
 ↓
Create backup
 ↓
Verify backup
 ↓
Modify source
 ↓
Run incremental backup
 ↓
Verify
 ↓
Delete source file
 ↓
Restore from backup
 ↓
Verify restored hash
```

Also test:

```text
Initial backup
 ↓
Disconnect destination
 ↓
Failure
 ↓
Reconnect
 ↓
Retry
 ↓
Successful verification
```

---

# 100. FAILURE-INJECTION TESTS

Introduce controlled failures in tests.

Examples:

```text
fail after N files
fail during copy
fail during verification
destination disappears
source disappears
source changes
manifest write fails
database write fails
```

Do not rely only on happy-path tests.

---

# 101. CRASH CONSISTENCY TESTS

Simulate termination during:

```text
copy
verify
manifest commit
snapshot commit
```

On restart:

The application must determine whether the backup run is:

```text
complete
incomplete
failed
recoverable
```

and must never falsely display an incomplete run as verified.

---

# 102. LARGE DATASET TESTING

Use the existing test generator.

Run at least:

```text
10,000 files
100,000 files
```

for backup planning.

For transfer testing, use a realistic but safe dataset size appropriate to available disk space.

Do not make CI require hundreds of gigabytes.

---

# 103. PERFORMANCE TARGETS

Do not invent unrealistic hard requirements.

Measure:

```text
planning time
hashing throughput
copy throughput
verification throughput
database write time
memory usage
```

For each benchmark record:

```text
OS
CPU
RAM
storage type
destination type
file count
total bytes
```

The goal is to establish a baseline.

---

# 104. MEMORY SAFETY

Do not load entire files into memory.

Use streaming:

```text
read chunk
hash/copy
repeat
```

This applies to:

- backup
- verification
- restore
- hashing

Large files must remain feasible.

---

# 105. MILLION-FILE SCALE

The architecture should not assume that every file can be held as a giant in-memory vector.

Use:

- streaming enumeration
- database-backed state
- bounded queues
- pagination

The existing Phase 1 architecture already uses batched database insertion. Follow the same philosophy.

---

# 106. DATABASE TRANSACTIONS

Use transactions for logically atomic metadata updates.

For example:

```text
backup run complete
+
snapshot metadata
+
file records
```

should not leave inconsistent metadata if the application crashes halfway through.

---

# 107. NO FAKE SUCCESS

This is a hard requirement.

Never display:

```text
Backup successful
```

unless:

- required files were transferred
- required verification completed
- metadata commit succeeded
- no unresolved critical failures exist

If warnings exist:

```text
Backup completed with warnings
```

If verification failed:

```text
Backup not verified
```

---

# 108. BACKUP SUMMARY

After each run:

```text
Backup completed

Files:
2,431

New:
24

Changed:
87

Unchanged:
2,320

Failed:
0

Transferred:
842 MB

Verified:
842 MB

Duration:
00:02:41

✓ Backup verified
```

---

# 109. BACKUP DETAILS

Allow the user to inspect:

```text
New files
Changed files
Failed files
Skipped files
Warnings
```

Do not require users to inspect technical logs.

---

# 110. SEARCH / FILTER

Backup file lists should support:

- filename search
- status filter
- changed-only
- failed-only
- protected-only

Reuse existing UI patterns.

---

# 111. BACKUP HISTORY

Provide:

```text
Backup History

Aug 28
✓ Verified
18.7 GB
2,431 files

Aug 21
✓ Verified
17.9 GB
2,390 files

Aug 14
⚠ Warnings
17.2 GB
2,340 files
```

Clicking a run shows details.

---

# 112. RESTORE HISTORY

Record restore operations in the existing operation history.

Example:

```text
Restore from backup

17 files
15 successful
2 failed
```

---

# 113. DOCUMENTATION

Create:

```text
docs/backup-architecture.md
docs/backup-safety.md
docs/backup-storage.md
docs/backup-testing.md
```

Update:

```text
README.md
docs/architecture.md
docs/safety.md
```

where appropriate.

Documentation must clearly state:

- what Phase 2 protects
- what it does not protect
- supported destinations
- verification model
- retention behavior
- limitations
- encryption status
- cloud status
- restore behavior

---

# 114. USER EDUCATION

The UI should explain an important distinction:

```text
Backup
≠
Synchronization
```

A backup preserves historical/recoverable state.

Do not silently turn the product into a mirror-sync application.

---

# 115. BACKUP VS SYNC

Do NOT implement destructive two-way synchronization.

Phase 2 is:

```text
Source → Backup
```

not:

```text
Source ↔ Destination
```

Changes made to backup data should not automatically propagate back to the source.

---

# 116. BACKUP VS DUPLICATE CLEANUP

Do not allow the duplicate cleanup engine to operate on FileHarbor-managed backup storage by default.

If a user explicitly scans a backup destination:

Show:

```text
This location contains FileHarbor backup data.

Cleaning duplicates here may damage your backups.

[Scan Read-Only]
[Cancel]
```

Prefer read-only analysis for backup destinations.

---

# 117. BACKUP DESTINATION EXCLUSION

Automatically exclude FileHarbor's internal metadata/control directories from ordinary file organization scans where appropriate.

Do not let users accidentally delete:

```text
.fileharbor/
```

through ordinary cleanup workflows.

---

# 118. FUTURE-PROOFING

Design interfaces so future implementations can add:

```text
LocalDestination
ExternalDriveDestination
NetworkDestination
CloudDestination
EncryptedDestination
```

without rewriting the backup planner.

Use traits/interfaces/abstractions only where they provide real architectural value.

Do not over-engineer.

---

# 119. OUT OF SCOPE FOR PHASE 2

Do NOT implement:

- cloud providers
- Google Drive integration
- OneDrive integration
- Dropbox integration
- custom encryption
- ransomware detection
- AI file classification
- near-duplicate detection
- image similarity
- corrupted-file reconstruction
- forensic HDD recovery
- two-way synchronization
- enterprise multi-user backup
- remote administration

These belong to future phases.

---

# 120. PHASE 2 SUCCESS CRITERIA

Phase 2 is complete only when:

### Backup Core

- [ ] Backup sets work.
- [ ] Local destinations work.
- [ ] Initial backup works.
- [ ] Incremental backup works.
- [ ] New files detected.
- [ ] Changed files detected.
- [ ] Unchanged files skipped.
- [ ] Deleted source files handled conservatively.
- [ ] Unstable files handled safely.

### Integrity

- [ ] SHA-256 verification works.
- [ ] Corruption detection works.
- [ ] Backup verification works.
- [ ] Backup cannot report false success.
- [ ] Manifests are atomic.
- [ ] Snapshot state is crash-safe.

### Restore

- [ ] Restore works.
- [ ] Restore verification works.
- [ ] Conflicts are explicit.
- [ ] Existing files are never silently overwritten.
- [ ] Partial restore failures are reported.

### Reliability

- [ ] Cancellation works.
- [ ] Interrupted backups are recoverable.
- [ ] Destination disconnection is handled.
- [ ] Insufficient space is handled.
- [ ] Source disappearance is handled.
- [ ] Retry works.

### Safety

- [ ] Source files are never modified by backup.
- [ ] Backup data is not treated as disposable duplicates.
- [ ] Backup destinations are validated.
- [ ] Recursive source/destination relationships are rejected.
- [ ] Backup metadata is protected.
- [ ] Destructive retention operations require explicit configuration.

### UI

- [ ] Backup dashboard exists.
- [ ] Backup creation workflow exists.
- [ ] Progress exists.
- [ ] Backup health exists.
- [ ] Backup history exists.
- [ ] Restore workflow exists.
- [ ] Errors are understandable.
- [ ] Accessibility maintained.

### Testing

- [ ] Unit tests.
- [ ] Integration tests.
- [ ] Failure-injection tests.
- [ ] Crash-consistency tests.
- [ ] Restore tests.
- [ ] Corruption tests.
- [ ] Incremental tests.
- [ ] Large dataset tests.
- [ ] Test-data-generator backup fixtures.

---

# 121. REQUIRED TEST MATRIX

The final test suite must cover at least:

| Scenario | Expected |
|---|---|
| New file | Backed up |
| Unchanged file | Not recopied |
| Changed content | Backed up again |
| Same name, different content | Correctly detected |
| Different name, same content | Content reuse/dedup where supported |
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

---

# 122. FINAL QA REQUIREMENT

Do not stop after unit tests.

After implementation:

1. Build the application.
2. Generate a deterministic backup test dataset.
3. Create an initial backup.
4. Verify it.
5. Modify the source.
6. Run incremental backup.
7. Verify again.
8. Corrupt one backup file.
9. Run verification.
10. Confirm corruption is detected.
11. Restore the file.
12. Verify restored SHA-256.
13. Test a destination disconnect.
14. Test cancellation.
15. Test an interrupted run.
16. Test backup/source duplicate interactions.
17. Test large datasets.
18. Perform manual GUI testing.

The final report must distinguish:

```text
Automated tests
Integration tests
Manual tests
Performance tests
```

---

# 123. FINAL ENGINEERING REPORT

After implementation, produce a detailed report containing:

## Repository Analysis

What existed before Phase 2.

## Architecture Changes

Exactly what was added and why.

## Database Changes

Tables, migrations, indexes, relationships.

## Backup Storage Model

Explain exactly how files, snapshots, manifests, and content are stored.

## Backup Algorithm

Explain:

```text
planning
→ transfer
→ verification
→ commit
```

## Incremental Algorithm

Explain how new/changed/unchanged files are determined.

## Integrity Model

Explain SHA-256 and verification.

## Failure Handling

Explain every important failure path.

## Restore

Explain restore conflict handling and verification.

## Safety

Explain why the implementation cannot silently destroy source files.

## Testing

Report:

```text
Unit:
Integration:
Failure injection:
Crash recovery:
Manual:
Performance:
Total:
Passed:
Failed:
Skipped:
```

## Known Limitations

Be honest.

## Future Recommendations

Explain what should come next.

---

# 124. IMPLEMENTATION DISCIPLINE

Before changing code:

1. Inspect the entire existing repository.
2. Understand existing Rust modules.
3. Understand existing SQLite schema.
4. Understand Tauri command conventions.
5. Understand frontend state management.
6. Understand existing scan progress/cancellation infrastructure.
7. Understand quarantine/restore implementation.
8. Understand existing test-data-generator architecture.

Do not guess.

Do not duplicate existing functionality unnecessarily.

Reuse stable Phase 1 primitives.

---

# 125. IMPORTANT: DO NOT "FIX" UNRELATED THINGS

Do not use Phase 2 as an excuse to:

- redesign the entire UI
- rewrite the scanner
- rewrite SQLite persistence
- replace Tauri
- replace React
- replace the hashing system
- rename unrelated modules
- introduce unnecessary dependencies
- refactor unrelated Phase 1 code

If you find unrelated issues:

```text
Document them separately.
```

Only modify them if Phase 2 genuinely depends on the change.

---

# 126. QUALITY BAR

The implementation must feel like a serious desktop backup application, not:

> "Copy this folder to another folder."

It must understand:

```text
identity
integrity
history
verification
failure
recovery
protection
```

The user should always know:

> What is protected?

> When was it last protected?

> Is the backup actually verified?

> What changed since the last backup?

> Can I restore it?

> What happens if the backup drive disappears?

The system must answer these questions clearly.

---

# FINAL INSTRUCTION

Implement **Phase 2 — Backup & Protection** completely.

Build it incrementally.

After each major subsystem:

```text
implement
→ compile
→ test
→ inspect
→ fix
```

Do not wait until the end to discover architectural problems.

Preserve all Phase 1 safety guarantees.

Most importantly:

> **FileHarbor must never confuse "there is another copy" with "this file is safe to delete."**

A backup is a protected copy.

The goal of Phase 2 is not merely to copy files.

The goal is to give the user **verifiable confidence that their important files can be recovered when something goes wrong.**