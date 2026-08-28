# Safety Model

Data safety outranks convenience. These are the enforced rules of Phase 1.

## What counts as a duplicate?

> Two files are duplicates **only** when they have the same size AND the
> same SHA-256 content hash.

Explicitly insufficient signals that are never used as identity:
filename, extension, timestamps, metadata, size alone. A file named
`report.txt` next to a different `report.txt` is not flagged. Two
byte-identical files named completely differently ARE grouped.

Empty files are excluded from duplicate groups even though zero-byte
content trivially matches — empties are presented separately because
empty ≠ useless (.gitkeep, lock files, flags).

## Nothing destructive happens automatically

- Scanning never writes to, renames, or deletes user files.
- There is no "Clean Everything" button.
- Duplicates and empty files have separate review screens and separate
  action flows.
- The keep-recommendation in each duplicate group is advisory text; the
  user selects exactly which copies to quarantine.

## Quarantine instead of deletion

"Move to quarantine" transfers the file into the application-managed
quarantine area and records: original path, quarantine path, size, hash,
timestamp and batch id (SQLite + `manifest.json`). Only after the record
is safely stored does the transfer complete.

Cross-volume moves use copy → verify destination size (and stored hash
when available) → only then remove the original. If any step fails, the
original stays in place and the failure is reported per file.

## Restore

Restore returns quarantined copies to their original locations,
recreating missing parent directories when needed.

Conflict rule: if a file already exists at the original path it is
**never overwritten silently**. The user explicitly chooses:
Keep Existing · Restore With New Name (`name (restored).ext`) · Cancel.
Previews detect conflicts before anything moves.

## Permanent deletion

- Operates exclusively on items already inside quarantine — the service
  verifies every target path is under the managed quarantine root and
  refuses anything else.
- Requires explicit confirmation showing the number of files and total
  size with a clear "cannot be undone" statement.
- Every item's outcome (success or failure) is recorded individually.

## Failure handling

One failing file never aborts a scan or batch operation. Failures become
visible states (`inaccessible`, `hash_failed`, `changed_during_scan`)
with human-readable reasons, and remain retryable. Locked files
(other processes), vanished files, permission errors and mid-read
modifications are all handled this way.

## Other guarantees

- Symlinks/junctions are never followed for directories (cycle +
  double-count protection); file symlinks are recorded but skipped.
- Quarantine filenames are sanitized; quarantine paths are unique per
  operation.
- Paths from the database are treated as untrusted input by the
  deletion safety check.
- Errors surface with plain-language messages; technical detail stays in
  structured logs (`<app data>/logs/app.log`). No file contents are ever
  logged.

## Known Phase 1 limitations

- Does not determine usefulness, junk status, age, versioning,
  visual similarity, corruption, or recovery lineage of any file.
- Non-UTF-8 representable paths are stored lossily (`to_string_lossy`).
- Pause/resume of a running scan is a documented future enhancement;
  cancellation is fully supported.
