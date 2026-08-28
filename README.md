# FileHarbor — Phase 1

A safe filesystem analysis and cleanup utility for Windows/macOS/Linux.
Phase 1 finds **objective facts** about your files — exact duplicates and
empty files — and lets you review them before anything happens.
Cleanup moves files into an application-managed **quarantine** area;
permanent deletion requires explicit confirmation and is only ever
performed on quarantined copies.

> Phase 1 does **not** decide whether a file is "useful", "junk", or
> "safe to delete". It only reports verifiable filesystem facts.

## Features (Phase 1)

- Recursive scanning of one or more root folders with exclusions
- Streaming SHA-256 content hashing (large-file safe)
- Exact duplicate detection: same size + identical SHA-256 → grouped,
  with a *recommendation* of which copy to keep (you always decide)
- Empty file detection, shown separately (empty ≠ useless)
- Inaccessible / hash-failed / changed-during-scan tracking with retry
- Scan progress, cooperative cancellation, partial results on cancel
- Quarantine / Restore / Permanent deletion with full operation history
- Hash cache keyed by `(path, size, mtime)` — optimization only
- SQLite persistence of sessions, results, operations and settings

## Stack

| Layer      | Technology                                    |
|------------|-----------------------------------------------|
| Core       | Rust (`crates/organizer-core`)                |
| Desktop    | Tauri 2                                       |
| UI         | React 18 + TypeScript (Vite)                  |
| Storage    | SQLite via `rusqlite` (bundled)               |
| Hashing    | `sha2`, chunked streaming                     |
| Parallelism| `rayon` for candidate hashing                 |

## Development

```bash
npm install          # frontend dependencies
cargo test           # core engine tests (no GUI needed)
                     #   run inside src-tauri/ or use: cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri dev    # run the desktop app in development mode
npm run build        # type-check + build the frontend
npm run tauri build  # production bundle
```

Requirements: Node.js ≥ 18, Rust stable toolchain, platform webview
(WebView2 on Windows).

## Testing

All core logic is covered by headless integration tests operating only
on temporary directories:

```bash
cd src-tauri
cargo test
```

Test suites:

- `hashing.rs` — identical/different/empty/large content, unstable
  (changed-during-hash), missing files, locked files, cancellation
- `duplicates.rs` — scanner traversal, duplicate matrix
  (A==B dup, same-name-different-content NOT dup, same-size-different-content
  NOT dup, renamed identical IS dup), empty files kept out of duplicate
  groups, symlink policy, search/filters, hash cache, 2000+ file scans
- `lifecycle.rs` — quarantine move + operation records, restore,
  restore conflicts (never overwrites), directory re-creation on restore,
  partial batch failures, verified cross-volume transfers, permanent
  deletion (+ refusal to delete outside quarantine), cancelled scans

Tests never touch real user folders; fixtures live in temp directories.

## Documentation

- [Architecture](docs/architecture.md) — layers, data model, scan pipeline
- [Safety Model](docs/safety.md) — duplicate definition, quarantine design,
  restore conflicts, deletion rules

## Roadmap (planned, not implemented)

- **Phase 2** — intelligent organization suggestions
- **Phase 3** — near-duplicate detection (text similarity, perceptual image hashing)
- **Phase 4** — corruption detection & recovered-media analysis
- **Phase 5** — recovery-assisted reconstruction

The analysis layer (`dupdetect` today) is designed as a pluggable stage so
future detectors can consume `FileRecord`s without changing the scanner.
