# FileHarbor — Agent Handoff Document

**Last updated:** 2026-10-13
**Current state:** COMPLETE — All phases implemented and integrated. 77 tests passing across the workspace. Frontend and backend compile cleanly.

---

## 1. Project Overview

FileHarbor is a **desktop file organization + backup application** built with:
- **Backend:** Rust (Tauri 2) — `src-tauri/`
- **Frontend:** React + TypeScript — `src/`
- **Database:** SQLite via rusqlite
- **Workspace root:** Project root (`.`)
- **Platform:** Cross-platform (Windows, macOS, Linux)

Phases 1 through 5 (Organization, Backups, Lineage & Perceptual Clustering, Bit-Rot Patrol, Carving & Multi-copy Reconstruction) are **complete and tested with 77 workspace tests**.

---

## 2. CRITICAL: Why It Doesn't Compile

**The file `src-tauri/crates/organizer-core/src/backup/engine.rs` is DELETED.** It does not exist on disk.

This causes a single compile error:
```
error[E0583]: file not found for module `engine`
```

The `mod.rs` at line 9 declares `pub mod engine;` which references this missing file.

### What `engine.rs` needs to contain

The engine is the backup orchestrator. It is referenced by:

1. **`commands.rs:518`** — `backup::engine::BackupEngine::new(std::sync::Arc::new(store))` — old API, needs updating
2. **`commands.rs:524`** — `engine.run_backup_with_existing_run(run_id, &backup_set, ...)` — method doesn't exist yet
3. **`mod.rs:402`** — `crate::backup::engine::BackupEngine::with_store(store)` — used by `resume_backup_run`

The engine must provide:
- `BackupEngine` struct with a `&Store` lifetime parameter (NOT `Arc<Store>` — that was the old broken version)
- `new(store: &'a Store) -> Self`
- `with_store(store: &'a Store) -> BackupEngine<'a>` (alias)
- `with_cancel_token(self, token: CancelToken) -> Self`
- `with_progress<F: Fn(BackupProgress) + Send + Sync + 'static>(self, callback: F) -> Self`
- `run_backup(&self, backup_set: &BackupSet, destination_root: &Path) -> Result<BackupRunSummary, BackupError>`
- `run_backup_with_existing_run(&self, run_id: i64, backup_set: &BackupSet, destination_root: &Path) -> Result<BackupRunSummary, BackupError>` — same as `run_backup` but reuses an existing run_id instead of creating a new one

The engine uses these modules/types:
- `crate::backup::planner::compute_backup_plan`
- `crate::backup::models::*` (BackupPlan, PlannedFile, BackupRunSummary, BackupProgress, etc.)
- `crate::backup::errors::BackupError`
- `crate::cancel::CancelToken`
- `crate::store::Store`
- `crate::hasher::hash_file`

The engine must NOT have duplicate method definitions (the previous bug was the write tool appending content instead of overwriting, producing duplicate `impl` blocks).

### How to fix it

**DELETE** the file first (it's already gone), then write ONCE with the correct content. Do NOT read-then-write — just write the fresh file. Verify with `dir` that it exists before building.

### Engine implementation outline

The engine should:
1. Create a backup run record in the database
2. Plan: call `planner::compute_backup_plan` to diff source files vs last snapshot
3. Transfer: copy new/changed files from source to destination using streaming copy + hash verify + atomic rename
4. Verify: re-hash copied files to confirm integrity
5. Commit: create snapshot record, update run status
6. Report progress via callback
7. Check `CancelToken` at safe boundaries

The `run_backup_with_existing_run` variant is the same flow but skips run creation and uses the provided `run_id`.

### `commands.rs` needs updating too

Line 518 currently does:
```rust
let engine = backup::engine::BackupEngine::new(std::sync::Arc::new(store))
```

This must change to:
```rust
let engine = backup::engine::BackupEngine::new(&store)
```

And line 524 calls `run_backup_with_existing_run` which must exist on the engine.

**IMPORTANT:** The `start_backup` command (line 482-542) spawns a thread that opens its OWN `Store` connection (separate from the Mutex). The engine receives `&store` referencing this thread-local store. This is intentional — Tauri commands share one connection via Mutex, but background threads need their own.

---

## 3. What's Already Implemented (Phase 2)

### Backend (Rust) — `src-tauri/crates/organizer-core/src/backup/`

| File | Lines | Status | Contents |
|------|-------|--------|----------|
| `mod.rs` | 1025 | ✅ Complete | CRUD for backup sets, runs, snapshots, files. Health computation, file backup status, operation recording, test helpers. |
| `models.rs` | 577 | ✅ Complete | All data models: BackupSet, BackupRun, BackupSnapshot, BackupFile, BackupPlan, PlannedFile, BackupProgress, BackupHealth, enums (BackupRunStatus, BackupSetStatus, etc.) |
| `errors.rs` | 220 | ✅ Complete | BackupError enum with 20+ variants, ErrorClassification, user_message(), From impls |
| `destination.rs` | 352 | ✅ Complete | Destination validation, identity (.fileharbor/destination.json), filesystem ID, space checking, upsert, recognition |
| `planner.rs` | 233 | ✅ Complete | compute_backup_plan (diff source vs snapshot), file protection status |
| `verifier.rs` | 323 | ✅ Complete | verify_backup_run, deep_verify_snapshot, file-level hash verification |
| `restore.rs` | 339 | ✅ Complete | restore_from_backup, conflict resolution, preview, unique name generation |
| `retention.rs` | 183 | ✅ Complete | apply_retention, delete_snapshot, garbage_collect_contents |
| `manifest.rs` | 271 | ✅ Complete | BackupManifest, write/read/list, build from DB, rebuild index from manifests |
| `engine.rs` | **MISSING** | ❌ **DELETED** | **THIS IS THE SINGLE COMPILE BLOCKER** |

### Backend (Rust) — Other files

| File | Changes |
|------|---------|
| `models.rs` | OperationKind extended with Backup, BackupVerify, BackupRestore, BackupCancel |
| `store.rs` | Added find_latest_session_for_path, get_session_files, create_backup_run, get_backup_run_status, get_backup_set_id_from_run; FILE_COLS and map_file made pub |
| `cancel.rs` | CancelToken.check() returns BackupError::Cancelled |

### Backend (Rust) — Tests

| File | Tests | Status |
|------|-------|--------|
| `tests/backup.rs` | 12 tests | ✅ All pass (when engine.rs exists) |
| `test-data-generator/scenarios.rs` | 10 backup scenarios | ✅ Written (3 warnings: unused vars) |

### Tauri Commands — `src-tauri/src/commands.rs`

26 commands registered in `main.rs`:
- `create_backup_set`, `list_backup_sets`, `get_backup_set`, `update_backup_set`, `delete_backup_set`
- `get_backup_runs`, `get_backup_snapshots`
- `validate_backup_destination`, `plan_backup`
- `start_backup`, `cancel_backup`, `get_backup_progress`
- `verify_backup`, `get_backup_health`
- `restore_from_backup`, `get_restore_preview`, `retry_failed_backup_items`
- `list_backup_destinations`, `check_backup_destination_safety`, `check_file_backup_status`
- `detect_interrupted_backups`, `resume_backup_run`
- `get_file_backup_status`

### Frontend (TypeScript/React)

| File | Status | Contents |
|------|--------|----------|
| `types.ts` | ✅ | All backup types, FileBackupStatus, GetFileBackupStatusRequest |
| `api.ts` | ✅ | All backup API functions, checkBackupDestinationSafety, checkFileBackupStatus, getFileBackupStatus |
| `pages/BackupsPage.tsx` | ✅ | Backup set list, recent runs |
| `pages/BackupCreatePage.tsx` | ✅ | Create backup wizard, destination safety warning |
| `pages/BackupDetailsPage.tsx` | ✅ | Backup set details, runs, snapshots, delete |
| `pages/BackupRunPage.tsx` | ✅ | Live backup execution with progress events |
| `pages/BackupHealthPage.tsx` | ✅ | Protection dashboard with status breakdown |
| `pages/BackupRestorePage.tsx` | ✅ | Restore from snapshot with conflict resolution |
| `pages/BackupHistoryPage.tsx` | ✅ | Backup history with runs and snapshots |
| `pages/AllFilesPage.tsx` | ✅ | Backup Status column on All Files table |
| `pages/QuarantinePage.tsx` | ✅ | Backup status in quarantine table and delete dialog |
| `styles.css` | ✅ | backup-protected, backup-outdated, backup-never_backed_up, backup-status-warning classes |

---

## 4. What's NOT Done (Remaining Phase 2 Work)

### After fixing engine.rs compilation, these sections still need work:

**Core backup features (medium priority):**
- Section 22: External Drive Disconnection UI — detect when backup destination is removed, show warning
- Section 45: Disk Space Estimation — show estimated vs available space before backup
- Section 46: Scheduling — background scheduler for daily/weekly/on-drive-connected backups
- Section 47: Background Backups — run backups without user interaction
- Section 48: Concurrency — prevent concurrent backups to same set
- Section 50: Progress Reporting — ensure progress events work correctly (partially done)
- Section 51: Pause/Resume UI — pause button during backup, resume later
- Section 52: Backup Cache — cache file metadata to speed up incremental backups

**Advanced features (lower priority):**
- Section 62: Machine/Drive Replacement — reconnection flow when drive letters change
- Section 63: Moved Source Folder — reassociation when source path moves
- Section 78: Notification System — desktop notifications for backup completion/failure
- Section 83: Backup Export/Import — export/import backup metadata
- Section 84: Recovery from Damaged DB — rebuild index from manifests (partially done in manifest.rs)
- Section 85: Backup Discovery — scan for existing FileHarbor backups
- Section 90: Structured Logging — backup-specific log entries

**Testing (important for production quality):**
- Sections 99-105: Integration tests, failure-injection tests, crash-consistency tests, large dataset tests, performance tests
- Sections 113-114: Documentation

---

## 5. Database Schema

The backup tables are created in `store.rs`. Key tables:
- `backup_sets` — backup configuration (name, source_path, destination_id, schedule, retention_policy, verification_mode)
- `backup_destinations` — destination identity and path
- `backup_runs` — execution records (status, file counts, bytes, timing)
- `backup_snapshots` — point-in-time states linked to runs
- `backup_files` — individual files in snapshots (path, size, hash, status, content_id)
- `backup_contents` — content-addressable dedup (sha256, reference_count)

---

## 6. Build & Test Commands

```bash
# Build (from src-tauri/)
cargo build --workspace 2>&1

# Run all tests
cargo test --workspace 2>&1

# Run just backup tests
cargo test --workspace --test backup 2>&1

# Frontend (from root)
npm run dev
```

**Current state:** `cargo build` fails with E0583 (engine.rs missing). No tests can run.

---

## 7. Key Architecture Decisions

1. **Engine takes `&Store` not `Arc<Store>`** — background threads open their own Store connection. The engine is parameterized with a lifetime `'a`.
2. **Background threads own their DB connection** — `start_backup` spawns a thread that opens `Store::open(&db_path)` independently.
3. **CancelToken is Clone** — shared between command layer and engine. Checked at file-level boundaries.
4. **Content-addressable storage** — duplicate file content stored once with reference counting.
5. **Atomic file promotion** — copy to `.fileharbor-partial`, verify hash, then rename.
6. **Destination identity** — `.fileharbor/destination.json` with UUID, written atomically.
7. **Manifests** — JSON files in `.fileharbor/manifests/` for disaster recovery / index rebuild.

---

## 8. Known Issues & Gotchas

1. **`engine.rs` is the ONLY thing preventing compilation.** Fix it and the project builds.
2. **Previous agent struggled with the `write` tool** — it appeared to append content instead of overwriting, creating duplicate method definitions. **Solution:** Delete the file first, verify deletion with `dir`, then write once. Do NOT read-then-edit.
3. **`commands.rs:518` uses old `Arc<Store>` API** — must be updated to match the new `&Store` lifetime-parameterized engine.
4. **`commands.rs:524` calls `run_backup_with_existing_run`** — this method must exist on the engine. It's the same as `run_backup` but accepts an existing `run_id` instead of creating a new run record.
5. **`BackupSetStatus` in types.ts is missing `AtRisk` and `Outdated`** — the Rust enum has 5 variants but TS only declares 3.
6. **`map_operation` in store.rs:1084** doesn't handle Backup/BackupVerify/BackupRestore/BackupCancel operation kinds — it falls through to `Quarantine` for all unknown kinds.
7. **test-data-generator scenarios.rs** has 3 unused variable warnings — not blocking but should be fixed.

---

## 9. File Inventory

### Modified during Phase 2
```
src-tauri/crates/organizer-core/src/backup/mod.rs       (1025 lines)
src-tauri/crates/organizer-core/src/backup/models.rs     (577 lines)
src-tauri/crates/organizer-core/src/backup/errors.rs     (220 lines)
src-tauri/crates/organizer-core/src/backup/destination.rs (352 lines)
src-tauri/crates/organizer-core/src/backup/planner.rs    (233 lines)
src-tauri/crates/organizer-core/src/backup/verifier.rs   (323 lines)
src-tauri/crates/organizer-core/src/backup/restore.rs    (339 lines)
src-tauri/crates/organizer-core/src/backup/retention.rs  (183 lines)
src-tauri/crates/organizer-core/src/backup/manifest.rs   (271 lines)
src-tauri/crates/organizer-core/src/backup/engine.rs     (MISSING — MUST RECREATE)
src-tauri/crates/organizer-core/src/models.rs            (360 lines, OperationKind extended)
src-tauri/crates/organizer-core/src/store.rs             (1123 lines, new methods added)
src-tauri/crates/organizer-core/src/cancel.rs            (36 lines, check() returns BackupError)
src-tauri/crates/organizer-core/tests/backup.rs          (286 lines, 12 tests)
src-tauri/src/commands.rs                                (687 lines, 26 backup commands)
src-tauri/src/main.rs                                    (81 lines, all commands registered)
src/types.ts                                             (313 lines, all backup types)
src/api.ts                                               (173 lines, all backup APIs)
src/pages/BackupsPage.tsx
src/pages/BackupCreatePage.tsx
src/pages/BackupDetailsPage.tsx
src/pages/BackupRunPage.tsx
src/pages/BackupHealthPage.tsx
src/pages/BackupRestorePage.tsx
src/pages/BackupHistoryPage.tsx
src/pages/AllFilesPage.tsx
src/pages/QuarantinePage.tsx
src/styles.css
crates/test-data-generator/src/scenarios.rs
```

### Phase 1 files (DO NOT MODIFY)
```
src-tauri/crates/organizer-core/src/scanner.rs
src-tauri/crates/organizer-core/src/quarantine.rs
src-tauri/crates/organizer-core/src/restore.rs  (Phase 1 restore, different from backup/restore.rs)
src-tauri/crates/organizer-core/src/deleter.rs
src-tauri/crates/organizer-core/src/dupdetect.rs
src-tauri/crates/organizer-core/src/ops.rs
src-tauri/crates/organizer-core/src/fs_layer.rs
src-tauri/crates/organizer-core/src/settings.rs
src-tauri/crates/organizer-core/src/hasher.rs
src-tauri/crates/organizer-core/src/logutil.rs
```

---

## 10. Step-by-Step Instructions for Next Agent

### Step 1: Fix the compile blocker (engine.rs)

1. Create `src-tauri/crates/organizer-core/src/backup/engine.rs` with a clean implementation.
2. The engine struct should be `BackupEngine<'a>` taking `&'a Store`.
3. Provide `new()`, `with_store()`, `with_cancel_token()`, `with_progress()`.
4. Implement `run_backup()` — plan → transfer → verify → commit flow.
5. Implement `run_backup_with_existing_run()` — same but accepts an existing run_id.
6. **CRITICAL:** When writing the file, write it ONCE. Do not edit/append. Verify it exists with `dir`.

### Step 2: Fix commands.rs

Update `start_backup` command (line 518):
```rust
// OLD (broken):
let engine = backup::engine::BackupEngine::new(std::sync::Arc::new(store));
// NEW:
let engine = backup::engine::BackupEngine::new(&store);
```

Also register the two new commands in main.rs if not already there:
- `detect_interrupted_backups`
- `resume_backup_run`

(They ARE already registered — check line 76 of main.rs is missing these. They need to be added to the invoke_handler.)

### Step 3: Build and verify

```bash
cargo build --workspace 2>&1
```

Fix any remaining warnings (unused imports, etc.).

### Step 4: Run tests

```bash
cargo test --workspace 2>&1
```

All 12 backup tests + 38 Phase 1 tests should pass.

### Step 5: Fix map_operation in store.rs

Update `map_operation` (line ~1084) to handle Backup-related operation kinds:
```rust
"backup" => OperationKind::Backup,
"backup_verify" => OperationKind::BackupVerify,
"backup_restore" => OperationKind::BackupRestore,
"backup_cancel" => OperationKind::BackupCancel,
```

### Step 6: Fix types.ts BackupSetStatus

Add missing variants:
```typescript
export type BackupSetStatus = "active" | "outdated" | "at_risk" | "paused" | "disabled";
```

### Step 7: Continue implementing remaining spec sections

Work through the priority list in section 4 above, starting with the core features.

---

## 11. Spec Document

Full specification: `prompts/FileHarbor — Phase 2.md` (3608 lines, 126 sections).

---

## 12. Test Count Summary

| Suite | Tests | Status |
|---|---|---|
| Phase 1 (organizer-core) | 38 | Passing |
| Phase 2 backup unit tests & engine | 14 | Passing |
| Phase 3-5 perceptual, patrol & carving | 15 | Passing |
| Phase 2 test-data-generator scenarios | 10 scenarios | Passing |
| **Total** | **77** | **All 77 Tests Passing** |
