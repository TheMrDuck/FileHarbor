//! SQLite persistence layer.
//!
//! Stores scan sessions, file records, duplicate groups, hash cache,
//! quarantine records, operation history, user exclusions and settings.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, Row, OptionalExtension};

use crate::backup::errors::BackupError;
use crate::models::*;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub struct Store {
    pub(crate) conn: Connection,
}

pub struct FilePage {
    pub items: Vec<FileRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, Default)]
pub struct FileFilter {
    pub search: Option<String>,
    pub extension: Option<String>,
    pub scan_status: Option<ScanStatus>,
    pub empty_only: bool,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub sort: Option<String>,
    pub ascending: bool,
    pub limit: usize,
    pub offset: usize,
}

impl Store {
    pub fn open(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open(db_path)
            .map_err(|e| format!("unable to open database {}: {e}", db_path.display()))?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS scan_sessions (
                    id INTEGER PRIMARY KEY,
                    started_at_ms INTEGER NOT NULL,
                    finished_at_ms INTEGER,
                    status TEXT NOT NULL,
                    roots TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS file_records (
                    id INTEGER PRIMARY KEY,
                    session_id INTEGER NOT NULL REFERENCES scan_sessions(id),
                    path TEXT NOT NULL,
                    file_name TEXT NOT NULL,
                    extension TEXT,
                    size_bytes INTEGER NOT NULL,
                    created_at_ms INTEGER,
                    modified_at_ms INTEGER,
                    accessed_at_ms INTEGER,
                    is_empty INTEGER NOT NULL DEFAULT 0,
                    hash TEXT,
                    hash_algorithm TEXT,
                    hash_status TEXT NOT NULL DEFAULT 'not_computed',
                    scan_status TEXT NOT NULL,
                    error TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_files_session ON file_records(session_id);
                CREATE INDEX IF NOT EXISTS idx_files_session_status ON file_records(session_id, scan_status);
                CREATE INDEX IF NOT EXISTS idx_files_session_empty ON file_records(session_id, is_empty);
                CREATE INDEX IF NOT EXISTS idx_files_size ON file_records(session_id, size_bytes);

                CREATE TABLE IF NOT EXISTS duplicate_groups (
                    id INTEGER PRIMARY KEY,
                    session_id INTEGER NOT NULL REFERENCES scan_sessions(id),
                    hash TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    recommended_keep_file_id INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_dupgroups_session ON duplicate_groups(session_id);

                CREATE TABLE IF NOT EXISTS duplicate_members (
                    group_id INTEGER NOT NULL REFERENCES duplicate_groups(id) ON DELETE CASCADE,
                    file_id INTEGER NOT NULL REFERENCES file_records(id),
                    PRIMARY KEY (group_id, file_id)
                );

                CREATE TABLE IF NOT EXISTS hash_cache (
                    path TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    modified_at_ms INTEGER NOT NULL,
                    algorithm TEXT NOT NULL,
                    hash TEXT NOT NULL,
                    computed_at_ms INTEGER NOT NULL,
                    PRIMARY KEY (path, size_bytes, modified_at_ms, algorithm)
                );

                CREATE TABLE IF NOT EXISTS operations (
                    id INTEGER PRIMARY KEY,
                    batch_id TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    status TEXT NOT NULL,
                    started_at_ms INTEGER NOT NULL,
                    finished_at_ms INTEGER,
                    detail TEXT
                );

                CREATE TABLE IF NOT EXISTS operation_items (
                    id INTEGER PRIMARY KEY,
                    operation_id INTEGER NOT NULL REFERENCES operations(id),
                    source TEXT,
                    destination TEXT,
                    file_id INTEGER,
                    status TEXT NOT NULL,
                    error TEXT
                );

                CREATE TABLE IF NOT EXISTS quarantine_items (
                    id INTEGER PRIMARY KEY,
                    operation_id INTEGER NOT NULL REFERENCES operations(id),
                    original_path TEXT NOT NULL,
                    quarantine_path TEXT NOT NULL UNIQUE,
                    file_name TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    hash TEXT,
                    quarantined_at_ms INTEGER NOT NULL,
                    status TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS exclusions (
                    id INTEGER PRIMARY KEY,
                    path TEXT NOT NULL UNIQUE,
                    created_at_ms INTEGER NOT NULL
                );

CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS backup_sets (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    source_path TEXT NOT NULL,
                    destination_id INTEGER NOT NULL REFERENCES backup_destinations(id),
                    schedule TEXT NOT NULL DEFAULT 'manual',
                    retention_policy TEXT NOT NULL DEFAULT 'keep_all',
                    verification_mode TEXT NOT NULL DEFAULT 'standard',
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_backup_sets_dest ON backup_sets(destination_id);

                CREATE TABLE IF NOT EXISTS backup_destinations (
                    id INTEGER PRIMARY KEY,
                    destination_id TEXT NOT NULL UNIQUE,
                    path TEXT NOT NULL,
                    label TEXT,
                    filesystem_id TEXT,
                    created_at_ms INTEGER NOT NULL,
                    last_seen_at_ms INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_backup_destinations_path ON backup_destinations(path);

                CREATE TABLE IF NOT EXISTS backup_runs (
                    id INTEGER PRIMARY KEY,
                    backup_set_id INTEGER NOT NULL REFERENCES backup_sets(id),
                    snapshot_id INTEGER REFERENCES backup_snapshots(id),
                    status TEXT NOT NULL,
                    started_at_ms INTEGER NOT NULL,
                    finished_at_ms INTEGER,
                    files_total INTEGER NOT NULL DEFAULT 0,
                    files_new INTEGER NOT NULL DEFAULT 0,
                    files_changed INTEGER NOT NULL DEFAULT 0,
                    files_unchanged INTEGER NOT NULL DEFAULT 0,
                    files_deleted INTEGER NOT NULL DEFAULT 0,
                    files_failed INTEGER NOT NULL DEFAULT 0,
                    bytes_transferred INTEGER NOT NULL DEFAULT 0,
                    bytes_verified INTEGER NOT NULL DEFAULT 0,
                    error_summary TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_backup_runs_set ON backup_runs(backup_set_id);
                CREATE INDEX IF NOT EXISTS idx_backup_runs_status ON backup_runs(status);

                CREATE TABLE IF NOT EXISTS backup_snapshots (
                    id INTEGER PRIMARY KEY,
                    backup_set_id INTEGER NOT NULL REFERENCES backup_sets(id),
                    run_id INTEGER NOT NULL REFERENCES backup_runs(id),
                    label TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    files_count INTEGER NOT NULL DEFAULT 0,
                    total_size_bytes INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_backup_snapshots_set ON backup_snapshots(backup_set_id);

                CREATE TABLE IF NOT EXISTS backup_files (
                    id INTEGER PRIMARY KEY,
                    snapshot_id INTEGER NOT NULL REFERENCES backup_snapshots(id) ON DELETE CASCADE,
                    relative_path TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    sha256 TEXT NOT NULL,
                    status TEXT NOT NULL,
                    content_id INTEGER REFERENCES backup_contents(id),
                    error TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_backup_files_snapshot ON backup_files(snapshot_id);
                CREATE INDEX IF NOT EXISTS idx_backup_files_hash ON backup_files(sha256);
                CREATE INDEX IF NOT EXISTS idx_backup_files_status ON backup_files(status);

                CREATE TABLE IF NOT EXISTS backup_contents (
                    id INTEGER PRIMARY KEY,
                    sha256 TEXT NOT NULL UNIQUE,
                    size_bytes INTEGER NOT NULL,
                    reference_count INTEGER NOT NULL DEFAULT 1,
                    first_seen_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_backup_contents_hash ON backup_contents(sha256);
                "#,
            )
            .map_err(|e| format!("database migration failed: {e}"))?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Scan sessions
    // ------------------------------------------------------------------

    pub fn insert_session(&self, roots: &[String]) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO scan_sessions (started_at_ms, status, roots) VALUES (?1, ?2, ?3)",
                params![
                    now_ms(),
                    SessionStatus::Running.as_str(),
                    serde_json::to_string(roots).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_session(
        &self,
        session_id: i64,
        status: SessionStatus,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE scan_sessions SET status = ?1, finished_at_ms = ?2 WHERE id = ?3",
                params![status.as_str(), now_ms(), session_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_session(&self, session_id: i64) -> Result<ScanSession, String> {
        self.conn
            .query_row(
                "SELECT id, roots, started_at_ms, finished_at_ms, status
                 FROM scan_sessions WHERE id = ?1",
                params![session_id],
                map_session,
            )
            .map_err(|e| e.to_string())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<ScanSession>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, roots, started_at_ms, finished_at_ms, status
                 FROM scan_sessions ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![(limit as i64)], map_session)
            .map_err(|e| e.to_string())?;
rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    /// Get all file records for a scan session.
    pub fn get_session_files(&self, session_id: i64) -> Result<Vec<FileRecord>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT {FILE_COLS} FROM file_records WHERE session_id = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![session_id], map_file)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    /// Find the latest completed scan session that covers the given path.
    /// Returns the session_id if found.
    pub fn find_latest_session_for_path(&self, path: &str) -> Result<Option<i64>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, roots FROM scan_sessions
                 WHERE status = 'completed'
                 ORDER BY id DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let roots_json: String = row.get(1)?;
                Ok((id, roots_json))
            })
            .map_err(|e| e.to_string())?;
        
        let path_buf = std::path::Path::new(path);
        for row in rows {
            let (id, roots_json) = row.map_err(|e| e.to_string())?;
            let roots: Vec<String> = serde_json::from_str(&roots_json).unwrap_or_default();
            for root in roots {
                let root_path = std::path::Path::new(&root);
                if path_buf == root_path || path_buf.starts_with(root_path) {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }

    // ------------------------------------------------------------------
    // File records
    // ------------------------------------------------------------------

    pub fn insert_file_records(&self, records: &[NewFileRecord]) -> Result<(), String> {
        if records.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO file_records
                     (session_id, path, file_name, extension, size_bytes,
                      created_at_ms, modified_at_ms, accessed_at_ms,
                      is_empty, hash_status, scan_status, error)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'not_computed',?10,?11)",
                )
                .map_err(|e| e.to_string())?;
            for r in records {
                stmt.execute(params![
                    r.session_id,
                    r.path,
                    r.file_name,
                    r.extension,
                    r.size_bytes as i64,
                    r.created_at_ms,
                    r.modified_at_ms,
                    r.accessed_at_ms,
                    r.is_empty as i64,
                    r.scan_status.as_str(),
                    r.error,
                ])
                .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn update_file_hash_result(
        &self,
        file_id: i64,
        hash: Option<&str>,
        hash_status: HashStatus,
        scan_status: ScanStatus,
        error: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE file_records
                 SET hash = ?2, hash_algorithm = 'sha-256', hash_status = ?3,
                     scan_status = ?4, error = ?5
                 WHERE id = ?1",
                params![file_id, hash, hash_status.as_str(), scan_status.as_str(), error],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn candidate_sizes(&self, session_id: i64) -> Result<Vec<u64>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT size_bytes, COUNT(*) AS n
                 FROM file_records
                 WHERE session_id = ?1 AND scan_status = 'completed' AND is_empty = 0
                 GROUP BY size_bytes HAVING n > 1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![session_id], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map(|v| v.into_iter().map(|s| s as u64).collect())
            .map_err(|e| e.to_string())
    }

    pub fn files_with_sizes(
        &self,
        session_id: i64,
        sizes: &[u64],
    ) -> Result<Vec<FileRecord>, String> {
        // Chunked IN(...) queries to stay well under SQLite's variable limit.
        let mut out = Vec::new();
        for chunk in sizes.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT {FILE_COLS} FROM file_records f WHERE f.session_id = ? AND f.size_bytes IN ({placeholders})"
            );
            let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
            let ids: Vec<i64> = chunk.iter().map(|s| *s as i64).collect();
            let mut rows = stmt
                .query(rusqlite::params_from_iter(std::iter::once(session_id).chain(ids)))
                .map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                match map_file(row) {
                    Ok(rec) => out.push(rec),
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        Ok(out)
    }

    pub fn list_files(&self, session_id: i64, filter: &FileFilter) -> Result<FilePage, String> {
        let mut where_clauses = vec!["session_id = ?".to_string()];
        let mut bind_vals: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(session_id)];

        if let Some(search) = &filter.search {
            where_clauses.push(format!(
                "(file_name LIKE ?{} OR path LIKE ?{})",
                bind_vals.len() + 1,
                bind_vals.len() + 1
            ));
            bind_vals.push(Box::new(format!(
                "%{}%",
                search.replace('%', "\\%").replace('_', "\\_")
            )));
        }
        if let Some(ext) = &filter.extension {
            where_clauses.push(format!("extension = ?{}", bind_vals.len() + 1));
            bind_vals.push(Box::new(ext.trim_start_matches('.').to_lowercase()));
        }
        if let Some(status) = filter.scan_status {
            where_clauses.push(format!("scan_status = ?{}", bind_vals.len() + 1));
            bind_vals.push(Box::new(status.as_str().to_string()));
        }
        if filter.empty_only {
            where_clauses.push("is_empty = 1".to_string());
        }
        if let Some(min) = filter.min_size {
            where_clauses.push(format!("size_bytes >= ?{}", bind_vals.len() + 1));
            bind_vals.push(Box::new(min as i64));
        }
        if let Some(max) = filter.max_size {
            where_clauses.push(format!("size_bytes <= ?{}", bind_vals.len() + 1));
            bind_vals.push(Box::new(max as i64));
        }

        let sort_col = match filter.sort.as_deref() {
            Some("name") => "file_name",
            Some("size") => "size_bytes",
            Some("modified") => "modified_at_ms",
            Some("created") => "created_at_ms",
            Some("path") => "path",
            _ => "file_name",
        };
        let dir = if filter.ascending { "ASC" } else { "DESC" };
        let where_sql = where_clauses.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM file_records WHERE {where_sql}");
        let total: i64 = self
            .conn
            .query_row(
                &count_sql,
                rusqlite::params_from_iter(bind_vals.iter().map(|b| b.as_ref())),
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let sql = format!(
            "SELECT {FILE_COLS} FROM file_records f WHERE f.{where_sql}
             ORDER BY f.{sort_col} {dir} LIMIT {} OFFSET {}",
            filter.limit, filter.offset
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(bind_vals.iter().map(|b| b.as_ref())),
                map_file,
            )
            .map_err(|e| e.to_string())?;
        let items = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        Ok(FilePage { items, total })
    }

    /// Files in problematic states (inaccessible / failed / unstable).
    pub fn problem_files(&self, session_id: i64, limit: usize, offset: usize) -> Result<FilePage, String> {
        let statuses = "'inaccessible','hash_failed','changed_during_scan'";
        let total: i64 = self
            .conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM file_records
                     WHERE session_id = ? AND scan_status IN ({statuses})"
                ),
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {FILE_COLS} FROM file_records f
                 WHERE f.session_id = ?1 AND f.scan_status IN ({statuses})
                 ORDER BY f.path LIMIT ?2 OFFSET ?3"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![session_id, limit as i64, offset as i64], |row| {
                map_file(row)
            })
            .map_err(|e| e.to_string())?;
        let items = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        Ok(FilePage { items, total })
    }

    pub fn get_file(&self, file_id: i64) -> Result<FileRecord, String> {
        self.conn
            .query_row(
                &format!("SELECT {FILE_COLS} FROM file_records f WHERE f.id = ?1"),
                params![file_id],
                map_file,
            )
            .map_err(|e| e.to_string())
    }

    // ------------------------------------------------------------------
    // Duplicate groups
    // ------------------------------------------------------------------

    pub fn replace_duplicate_groups(
        &self,
        session_id: i64,
        groups: &[DuplicateGroupInput],
    ) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM duplicate_members WHERE group_id IN (SELECT id FROM duplicate_groups WHERE session_id = ?1)", params![session_id])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM duplicate_groups WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(|e| e.to_string())?;
        {
            let mut gstmt = tx
                .prepare(
                    "INSERT INTO duplicate_groups (session_id, hash, size_bytes, recommended_keep_file_id)
                     VALUES (?1,?2,?3,?4)",
                )
                .map_err(|e| e.to_string())?;
            let mut mstmt = tx
                .prepare("INSERT OR IGNORE INTO duplicate_members (group_id, file_id) VALUES (?1,?2)")
                .map_err(|e| e.to_string())?;
            for g in groups {
                gstmt
                    .execute(params![
                        session_id,
                        g.hash,
                        g.size_bytes as i64,
                        g.recommended_keep_file_id
                    ])
                    .map_err(|e| e.to_string())?;
                let gid = tx.last_insert_rowid();
                for fid in &g.member_file_ids {
                    mstmt
                        .execute(params![gid, fid])
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn duplicate_group_count(&self, session_id: i64) -> Result<i64, String> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM duplicate_groups WHERE session_id = ?1",
                params![session_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
    }

    pub fn duplicate_groups_page(
        &self,
        session_id: i64,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<DuplicateGroup>, i64), String> {
        let total = self.duplicate_group_count(session_id)?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id FROM duplicate_groups WHERE session_id = ?1
                 ORDER BY size_bytes DESC, hash ASC LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| e.to_string())?;
        let ids: Vec<i64> = stmt
            .query_map(params![session_id, limit as i64, offset as i64], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut groups = Vec::with_capacity(ids.len());
        for gid in ids {
            let (hash, size, keep): (String, i64, i64) = self
                .conn
                .query_row(
                    "SELECT hash, size_bytes, recommended_keep_file_id FROM duplicate_groups WHERE id = ?1",
                    params![gid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .map_err(|e| e.to_string())?;
            let mut mstmt = self
                .conn
                .prepare(&format!(
                    "SELECT {FILE_COLS} FROM file_records f
                     JOIN duplicate_members m ON m.file_id = f.id
                     WHERE m.group_id = ?1 ORDER BY f.path"
                ))
                .map_err(|e| e.to_string())?;
            let members = mstmt
                .query_map(params![gid], map_file)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            groups.push(DuplicateGroup {
                id: gid,
                session_id,
                hash,
                size_bytes: size as u64,
                members,
                recommended_keep_file_id: keep,
            });
        }
        Ok((groups, total))
    }

    // ------------------------------------------------------------------
    // Hash cache
    // ------------------------------------------------------------------

    /// Cached hash keyed by (path, size, mtime). This is an optimization
    /// only; it is never treated as content identity. If anything about the
    /// identity tuple differs, callers must recompute.
    pub fn hash_cache_get(
        &self,
        path: &str,
        size_bytes: u64,
        modified_at_ms: Option<i64>,
    ) -> Option<String> {
        let mtime = modified_at_ms?;
        self.conn
            .query_row(
                "SELECT hash FROM hash_cache
                 WHERE path = ?1 AND size_bytes = ?2 AND modified_at_ms = ?3 AND algorithm = 'sha-256'",
                params![path, size_bytes as i64, mtime],
                |r| r.get(0),
            )
            .ok()
    }

    pub fn hash_cache_put(
        &self,
        path: &str,
        size_bytes: u64,
        modified_at_ms: Option<i64>,
        hash: &str,
    ) {
        let Some(mtime) = modified_at_ms else { return };
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO hash_cache
             (path, size_bytes, modified_at_ms, algorithm, hash, computed_at_ms)
             VALUES (?1,?2,?3,'sha-256',?4,?5)",
            params![path, size_bytes as i64, mtime, hash, now_ms()],
        );
    }

    // ------------------------------------------------------------------
    // Operations + quarantine
    // ------------------------------------------------------------------

    pub fn insert_operation(&self, batch_id: &str, kind: OperationKind) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO operations (batch_id, kind, status, started_at_ms) VALUES (?1,?2,?3,?4)",
                params![batch_id, kind.as_str(), OperationStatus::InProgress.as_str(), now_ms()],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_operation_item(
        &self,
        operation_id: i64,
        source: Option<&str>,
        destination: Option<&str>,
        file_id: Option<i64>,
        status: OpItemStatus,
        error: Option<&str>,
    ) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO operation_items (operation_id, source, destination, file_id, status, error)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    operation_id,
                    source,
                    destination,
                    file_id,
                    status.as_str(),
                    error
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_operation_item(
        &self,
        item_id: i64,
        status: OpItemStatus,
        destination: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE operation_items SET status=?2, destination=COALESCE(?3,destination), error=?4 WHERE id=?1",
                params![item_id, status.as_str(), destination, error],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn finish_operation(
        &self,
        operation_id: i64,
        status: OperationStatus,
        detail: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE operations SET status=?2, finished_at_ms=?3, detail=?4 WHERE id=?1",
                params![operation_id, status.as_str(), now_ms(), detail],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_operations(&self, limit: usize, offset: usize) -> Result<Vec<FileOperation>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, batch_id, kind, status, started_at_ms, finished_at_ms, detail
                 FROM operations ORDER BY id DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], map_operation)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn operation_items(&self, operation_id: i64) -> Result<Vec<OperationItem>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, operation_id, source, destination, file_id, status, error
                 FROM operation_items WHERE operation_id = ?1 ORDER BY id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![operation_id], |row| {
                Ok(OperationItem {
                    id: row.get(0)?,
                    operation_id: row.get(1)?,
                    source: row.get(2)?,
                    destination: row.get(3)?,
                    file_id: row.get(4)?,
                    status: OpItemStatus::parse(&row.get::<_, String>(5)?),
                    error: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn insert_quarantine_item(
        &self,
        operation_id: i64,
        original_path: &str,
        quarantine_path: &str,
        file_name: &str,
        size_bytes: u64,
        hash: Option<&str>,
    ) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO quarantine_items
                 (operation_id, original_path, quarantine_path, file_name, size_bytes, hash, quarantined_at_ms, status)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,'stored')",
                params![
                    operation_id,
                    original_path,
                    quarantine_path,
                    file_name,
                    size_bytes as i64,
                    hash,
                    now_ms()
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn quarantine_set_status(
        &self,
        item_id: i64,
        status: QuarantineItemStatus,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE quarantine_items SET status=?2 WHERE id=?1",
                params![item_id, status.as_str()],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_quarantine_items(&self, include_consumed: bool) -> Result<Vec<QuarantineItem>, String> {
        let sql = if include_consumed {
            "SELECT id, operation_id, original_path, quarantine_path, file_name, size_bytes, hash, quarantined_at_ms, status
             FROM quarantine_items ORDER BY id DESC"
        } else {
            "SELECT id, operation_id, original_path, quarantine_path, file_name, size_bytes, hash, quarantined_at_ms, status
             FROM quarantine_items WHERE status = 'stored' ORDER BY id DESC"
        };
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], map_quarantine_item)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_quarantine_item(&self, item_id: i64) -> Result<QuarantineItem, String> {
        self.conn
            .query_row(
                "SELECT id, operation_id, original_path, quarantine_path, file_name, size_bytes, hash, quarantined_at_ms, status
                 FROM quarantine_items WHERE id = ?1",
                params![item_id],
                map_quarantine_item,
            )
            .map_err(|e| e.to_string())
    }

    // ------------------------------------------------------------------
    // Exclusions + settings
    // ------------------------------------------------------------------

    pub fn add_exclusion(&self, path: &str) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO exclusions (path, created_at_ms) VALUES (?1, ?2)",
                params![path, now_ms()],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_exclusion(&self, id: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM exclusions WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_exclusions(&self) -> Result<Vec<(i64, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM exclusions ORDER BY path")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Summary
    // ------------------------------------------------------------------

    pub fn scan_summary(&self, session_id: i64) -> Result<ScanSummary, String> {
        let session = self.get_session(session_id)?;
        let (files_scanned, total_size): (i64, i64) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(size_bytes),0) FROM file_records
                 WHERE session_id = ?1 AND scan_status != 'inaccessible'",
                params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let (empty_files, inaccessible, errors): (i64, i64, i64) = self
            .conn
            .query_row(
                "SELECT
                   COALESCE(SUM(CASE WHEN is_empty=1 THEN 1 ELSE 0 END),0),
                   COALESCE(SUM(CASE WHEN scan_status='inaccessible' THEN 1 ELSE 0 END),0),
                   COALESCE(SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END),0)
                 FROM file_records WHERE session_id = ?1",
                params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| e.to_string())?;
        let (dup_groups, dup_files, reclaimable): (i64, i64, i64) = self
            .conn
            .query_row(
                "SELECT
                   COUNT(*),
                   COALESCE(SUM(member_count),0),
                   COALESCE(SUM((member_count - 1) * size_bytes),0)
                 FROM (
                   SELECT g.id, g.size_bytes, (SELECT COUNT(*) FROM duplicate_members m WHERE m.group_id = g.id) AS member_count
                   FROM duplicate_groups g WHERE g.session_id = ?1
                 )",
                params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|e| e.to_string())?;

Ok(ScanSummary {
            session,
            files_scanned: files_scanned as u64,
            total_size_bytes: total_size as u64,
            empty_files: empty_files as u64,
            inaccessible_files: inaccessible as u64,
            error_files: errors as u64,
            duplicate_groups: dup_groups as u64,
            duplicate_files: dup_files as u64,
            potential_reclaimable_bytes: reclaimable.max(0) as u64,
        })
    }

    pub fn get_backup_run_status(&self, run_id: i64) -> Result<Option<(String, Option<i64>)>, String> {
        self.conn
            .query_row(
                "SELECT status, started_at_ms FROM backup_runs WHERE id = ?1",
                params![run_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn get_backup_set_id_from_run(&self, run_id: i64) -> Result<Option<i64>, String> {
        self.conn
            .query_row(
                "SELECT backup_set_id FROM backup_runs WHERE id = ?1",
                params![run_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn create_backup_run(&self, backup_set_id: i64) -> Result<i64, BackupError> {
        let now = now_ms();
        self.conn
            .execute(
                "INSERT INTO backup_runs (backup_set_id, status, started_at_ms) VALUES (?1, ?2, ?3)",
                params![backup_set_id, "running", now],
            )
            .map_err(|e| BackupError::DatabaseError(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }
}

// ----------------------------------------------------------------------
// Row mapping helpers
// ----------------------------------------------------------------------

pub const FILE_COLS: &str =
    "f.id, f.session_id, f.path, f.file_name, f.extension, f.size_bytes, \
     f.created_at_ms, f.modified_at_ms, f.accessed_at_ms, f.is_empty, \
     f.hash, f.hash_algorithm, f.hash_status, f.scan_status, f.error";

pub fn map_file(row: &Row) -> Result<FileRecord, rusqlite::Error> {
    Ok(FileRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        path: row.get(2)?,
        file_name: row.get(3)?,
        extension: row.get(4)?,
        size_bytes: row.get::<_, i64>(5)? as u64,
        created_at_ms: row.get(6)?,
        modified_at_ms: row.get(7)?,
        accessed_at_ms: row.get(8)?,
        is_empty: row.get::<_, i64>(9)? != 0,
        hash: row.get(10)?,
        hash_algorithm: row.get(11)?,
        hash_status: HashStatus::parse(&row.get::<_, String>(12)?),
        scan_status: ScanStatus::parse(&row.get::<_, String>(13)?),
        error: row.get(14)?,
    })
}

fn map_session(row: &Row) -> Result<ScanSession, rusqlite::Error> {
    let roots_json: String = row.get(1)?;
    Ok(ScanSession {
        id: row.get(0)?,
        roots: serde_json::from_str(&roots_json).unwrap_or_default(),
        started_at_ms: row.get(2)?,
        finished_at_ms: row.get(3)?,
        status: SessionStatus::parse(&row.get::<_, String>(4)?),
    })
}

fn map_operation(row: &Row) -> Result<FileOperation, rusqlite::Error> {
    Ok(FileOperation {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        kind: match row.get::<_, String>(2)?.as_str() {
            "restore" => OperationKind::Restore,
            "delete_permanent" => OperationKind::DeletePermanent,
            "backup" => OperationKind::Backup,
            "backup_verify" => OperationKind::BackupVerify,
            "backup_restore" => OperationKind::BackupRestore,
            "backup_cancel" => OperationKind::BackupCancel,
            _ => OperationKind::Quarantine,
        },
        status: OperationStatus::parse(&row.get::<_, String>(3)?),
        started_at_ms: row.get(4)?,
        finished_at_ms: row.get(5)?,
        detail: row.get(6)?,
    })
}

fn map_quarantine_item(row: &Row) -> Result<QuarantineItem, rusqlite::Error> {
    let status: String = row.get(8)?;
    Ok(QuarantineItem {
        id: row.get(0)?,
        operation_id: row.get(1)?,
        original_path: row.get(2)?,
        quarantine_path: row.get(3)?,
        file_name: row.get(4)?,
        size_bytes: row.get::<_, i64>(5)? as u64,
        hash: row.get(6)?,
        quarantined_at_ms: row.get(7)?,
        status: match status.as_str() {
            "restored" => QuarantineItemStatus::Restored,
            "deleted" => QuarantineItemStatus::Deleted,
            _ => QuarantineItemStatus::Stored,
        },
    })
}

/// Input used when persisting computed duplicate groups.
#[derive(Debug, Clone)]
pub struct DuplicateGroupInput {
    pub hash: String,
    pub size_bytes: u64,
    pub member_file_ids: Vec<i64>,
    pub recommended_keep_file_id: i64,
}

