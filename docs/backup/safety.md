# Backup Safety

FileHarbor Phase 2 — Safety Model

## Core Safety Rule

> FileHarbor must never confuse "there is another copy" with "this file is
> safe to delete."

A backup copy is a **protected copy**, not a disposable duplicate.

## Copy Roles

FileHarbor distinguishes four copy roles:

| Role | Description |
|---|---|
| `working` | User's original file (source of truth) |
| `backup` | Protected backup copy (must not be auto-deleted) |
| `quarantine` | Quarantined copy awaiting user decision |
| `unknown` | Not yet classified |

## Duplicate Cleanup vs. Backup

Phase 1 duplicate detection remains based on size + SHA-256. It does not
know about backup copies. Phase 2 adds protection context separately.

If a user scans a backup destination with the duplicate cleanup tool:

1. FileHarbor detects `.fileharbor/` marker
2. Shows a warning: "This location contains FileHarbor backup data.
   Cleaning duplicates here may damage your backups."
3. Offers [Scan Read-Only] and [Cancel]
4. Does not allow destructive cleanup by default

## No Silent Destructive Actions

Backup functionality never:

- Deletes source files
- Moves source files
- Modifies source content
- Renames source files
- Overwrites existing unrelated destination files
- Deletes previous snapshots automatically

Unless the user has explicitly configured a retention/deletion policy.

## Quarantine Interaction

When a user quarantines a source file:

- If a verified backup exists: shows "Backup exists — the original can be
  restored from [destination]" and offers [Continue]
- If no backup exists: shows "No verified backup — this file is not
  protected" and offers [Cancel]

## Delete Safety

When a user permanently deletes a file from quarantine:

- If a verified backup exists: shows "Backup available — permanent deletion
  affects only the quarantined copy" and offers [Delete]
- If no backup exists: shows "No verified backup — this may be your only
  remaining copy" and offers [Cancel] and [Delete Anyway]

## Destination Validation

Before allowing a backup, FileHarbor validates:

- Destination exists or can safely be created
- Destination is writable
- Destination is not the source itself
- Destination is not inside the source
- Destination does not resolve to the source through symlinks/junctions
- Enough disk space exists
- Destination is accessible

## Path Safety

- Relative paths are used inside backup snapshots
- Source paths are never reconstructed by concatenating unsafe strings
- Path traversal (`..`) and absolute path injection are rejected
- Symlink escapes are detected and warned about

## Symlink Policy

FileHarbor follows Phase 1 safety policy: symlinks are skipped by default
during backup. They are not followed, and their contents are not copied.

## Encryption Status

**Phase 2 does not provide encrypted backup storage.**

Backups are stored in plaintext on the destination filesystem. If
encryption is needed, use filesystem-level encryption (BitLocker, FileVault,
etc.) or wait for a future dedicated encryption phase.

## Cloud Status

**Phase 2 does not provide cloud backup.**

Only local filesystem destinations are supported. The architecture is
designed so future phases can add network and cloud destinations.

## 3-2-1 Readiness

FileHarbor does not pretend to provide 3-2-1 protection unless the user
has configured multiple backup destinations across different storage types.

The UI may eventually show:

```
3-2-1 Backup
3 copies
2 storage types
1 off-site
```

But only when the user's actual configuration meets those criteria.