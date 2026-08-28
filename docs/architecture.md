# Architecture

```
Filesystem Layer            fs_layer     entry inspection, symlink policy,
        ↓                                exclusion matching
Scanner                     scanner      staged pipeline: discover → candidates
        ↓                                → hash → group; progress + cancellation
Metadata                    fs_layer + store  sizes, timestamps, states persisted per session
Hashing Engine              hasher       streaming SHA-256, instability detection,
        ↓                                result cache
Duplicate Detection Engine  dupdetect    size grouping, SHA-256 grouping,
        ↓                                keep recommendation (advisory only)
Analysis Layer              (future detectors plug in here)
Persistence Layer           store        SQLite: sessions, files, groups,
        ↓                                quarantine, operations, settings
Application Services        quarantine / restore / deleter / ops
        ↓
UI (Tauri commands/events)  src-tauri/src/commands.rs
        ↓
Frontend (React)            src/
```

## Data model (`organizer-core::models`)

- **ScanSession** — one scan run: roots, start/end, status
  (`running/completed/cancelled/failed`)
- **FileRecord** — path, name, extension, size, timestamps (where the OS
  provides them), empty flag, SHA-256 + hash status, scan status, error text
- **DuplicateGroup** — hash + size + members + recommended keep id
  (advisory)
- **QuarantineItem** — original path ↔ quarantine path, size, hash, status
- **FileOperation / OperationItem** — batch history with per-item outcomes

### File states

`ready · scanning · hashing · completed · inaccessible ·
changed_during_scan · hash_failed · skipped`

Failed states are never treated as successful results and are excluded
from duplicate grouping. Files whose size changed while being hashed get
`changed_during_scan` and no usable hash; they can be retried via
"Retry analysis" (`rebuild_analysis`).

## Scan pipeline

1. **Discover + metadata** — single iterative traversal (`std::fs`),
   alphabetical order, batched inserts (512/batch). Directory symlinks
   and junctions are never followed; file symlinks are recorded as
   `skipped`. The quarantine root and user exclusions are filtered here.
2. **Candidates** — SQL `GROUP BY size HAVING count > 1`; only these
   files need hashing. Unique-size files skip hashing entirely.
3. **Hashing** — rayon parallel hashing (pure filesystem work only;
   DB access stays on the scan thread because rusqlite connections are
   not `Sync`). Cached hashes reused when `(path, size, mtime)` matches.
4. **Grouping** — group by `(size, sha256)` where both match; groups
   sorted largest-first; recommendation computed per group.

Cancellation is checked between chunks during hashing and between
entries/phases elsewhere. A cancelled scan keeps all discovered records
and marks the session `cancelled`.

## Quarantine layout

```
<app data dir>/Quarantine/<batch-id>/<seq>_<safe-filename>
                            └── manifest.json (batch summary)
```

Filenames are sanitized (illegal characters, Windows reserved names,
trailing dots/spaces). Cross-volume moves fall back to
copy → verify(size [+ hash]) → remove-original; any verification failure
keeps the original untouched and removes the unverified copy.

## Future-phase hooks

The store schema and models leave room for additional per-file analysis
tables (fingerprints, similarity scores, relationships) without touching
the scanner contract. New detectors would consume `Vec<FileRecord>` and
produce their own result tables, mirroring `dupdetect`.
