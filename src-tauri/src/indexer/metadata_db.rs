use crate::indexer::walker::FileEntry;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Durable metadata store backed by SQLite.
///
/// The connection is wrapped in a `Mutex` because `rusqlite::Connection` is
/// `Send` but not `Sync`; this keeps `MetadataDb` (and therefore
/// `MetadataStore` inside an `Arc<RwLock<..>>`) shareable across threads.
pub struct MetadataDb {
    conn: Mutex<Connection>,
}

impl MetadataDb {
    pub fn begin_transaction(&self) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("BEGIN", [])
            .map_err(|e| format!("Failed to begin transaction: {}", e))?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("COMMIT", [])
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;
        Ok(())
    }

    pub fn open() -> Result<Self, String> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| "Could not determine config directory".to_string())?;
        let db_dir = config_dir.join("kensaku");
        std::fs::create_dir_all(&db_dir)
            .map_err(|e| format!("Failed to create metadata directory: {}", e))?;

        Self::open_at(&db_dir.join("metadata.db"))
    }

    /// Opens (and initializes) a database at `<dir>/metadata.db`.
    ///
    /// Used by the headless (CLI/MCP) surface to target an explicit config
    /// directory, including temporary directories in tests.
    pub fn open_in_dir(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create metadata directory: {}", e))?;
        Self::open_at(&dir.join("metadata.db"))
    }

    /// Opens (and initializes) a database at an explicit path.
    ///
    /// Used by [`Self::open`] and by tests so they never touch the real config
    /// directory.
    fn open_at(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open metadata database: {}", e))?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA mmap_size=268435456;
             CREATE TABLE IF NOT EXISTS nodes (
               id INTEGER PRIMARY KEY,
               path TEXT NOT NULL UNIQUE,
               name TEXT NOT NULL,
               extension TEXT,
               size INTEGER NOT NULL,
               modified INTEGER NOT NULL,
               is_dir INTEGER NOT NULL,
               created_gen INTEGER NOT NULL DEFAULT 0,
               dev INTEGER NOT NULL DEFAULT 0,
               ino INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS nodes_path ON nodes(path);
             CREATE TABLE IF NOT EXISTS usage (
               path TEXT PRIMARY KEY,
               opens INTEGER NOT NULL DEFAULT 0,
               last_open INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS units (
               id INTEGER PRIMARY KEY,
               node_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
               byte_start INTEGER NOT NULL,
               byte_end INTEGER NOT NULL,
               model_id INTEGER NOT NULL,
               vec BLOB
             );
             CREATE INDEX IF NOT EXISTS units_node ON units(node_id);
             CREATE TABLE IF NOT EXISTS meta (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             INSERT OR IGNORE INTO meta (key, value) VALUES ('model_id', '1');
             INSERT OR IGNORE INTO meta (key, value) VALUES ('embedding_dim', '256');",
        )
        .map_err(|e| format!("Failed to initialize metadata database: {}", e))?;

        // Migration for databases created before file identity was tracked.
        // `ALTER TABLE ... ADD COLUMN` fails with "duplicate column name" when
        // the column already exists, which is expected and safe to ignore.
        for ddl in [
            "ALTER TABLE nodes ADD COLUMN dev INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE nodes ADD COLUMN ino INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE nodes ADD COLUMN indexed_gen INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE nodes ADD COLUMN content_hash TEXT",
            "ALTER TABLE units ADD COLUMN vec BLOB",
        ] {
            if let Err(e) = conn.execute(ddl, []) {
                let message = e.to_string();
                if !message.contains("duplicate column name") {
                    eprintln!("[Kensaku] Metadata migration warning: {}", message);
                }
            }
        }

        // Must run after the `dev`/`ino` columns are guaranteed to exist.
        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS nodes_dev_ino ON nodes(dev, ino)",
            [],
        ) {
            eprintln!("[Kensaku] Failed to create dev/ino index: {}", e);
        }

        // Partial index for embedding queue: fast lookup of unembedded nodes
        if let Err(e) = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_unembedded ON nodes(indexed_gen) WHERE indexed_gen = 0",
            [],
        ) {
            eprintln!("[Kensaku] Failed to create unembedded index: {}", e);
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn load_all(&self) -> Result<Vec<FileEntry>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, path, name, extension, size, modified, is_dir, dev, ino FROM nodes",
            )
            .map_err(|e| format!("Failed to prepare metadata query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let name: String = row.get(2)?;
                let extension: Option<String> = row.get(3)?;
                let size: i64 = row.get(4)?;
                let modified: i64 = row.get(5)?;
                let is_dir: i64 = row.get(6)?;
                let dev: i64 = row.get(7)?;
                let ino: i64 = row.get(8)?;

                let char_bloom = crate::search::fuzzy::compute_char_mask(&name);
                Ok(FileEntry {
                    id: id as u64,
                    path: PathBuf::from(path),
                    name,
                    extension,
                    size: u64::try_from(size).unwrap_or(0),
                    modified: u64::try_from(modified).unwrap_or(0),
                    is_dir: is_dir != 0,
                    dev: u64::try_from(dev).unwrap_or(0),
                    ino: u64::try_from(ino).unwrap_or(0),
                    char_bloom,
                })
            })
            .map_err(|e| format!("Failed to read metadata rows: {}", e))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row.map_err(|e| format!("Failed to decode metadata row: {}", e))?);
        }

        Ok(entries)
    }

    pub fn upsert(&self, entry: &FileEntry) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO nodes (path, name, extension, size, modified, is_dir, dev, ino)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(path) DO UPDATE SET
               name = excluded.name,
               extension = excluded.extension,
               size = excluded.size,
               modified = excluded.modified,
               is_dir = excluded.is_dir,
               dev = excluded.dev,
               ino = excluded.ino,
               indexed_gen = CASE WHEN nodes.modified != excluded.modified OR nodes.size != excluded.size THEN 0 ELSE nodes.indexed_gen END",
            params![
                entry.path.to_string_lossy().to_string(),
                entry.name,
                entry.extension,
                entry.size as i64,
                entry.modified as i64,
                entry.is_dir as i64,
                entry.dev as i64,
                entry.ino as i64,
            ],
        )
        .map_err(|e| format!("Failed to upsert metadata entry: {}", e))?;

        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<(), String> {
        let conn = self.conn.lock();
        let path_str = path.to_string_lossy().to_string();
        let _ = conn.execute(
            "DELETE FROM units WHERE node_id IN (SELECT id FROM nodes WHERE path = ?1)",
            params![path_str],
        );
        conn.execute(
            "DELETE FROM nodes WHERE path = ?1",
            params![path_str],
        )
        .map_err(|e| format!("Failed to delete metadata entry: {}", e))?;

        Ok(())
    }

    /// Rewrites every node that is `old` or lives under `old` so that it lives
    /// under `new` instead, preserving relative suffixes.
    ///
    /// The whole rewrite runs in one transaction so a crash cannot leave a
    /// moved directory half-indexed at its old location. Rows that already sit
    /// under the destination are dropped first: the filesystem rename replaced
    /// that subtree and the caller re-indexes it. Returns the number of rows
    /// moved.
    pub fn rename_prefix(&self, old: &Path, new: &Path) -> Result<usize, String> {
        let old_str = old.to_string_lossy().into_owned();
        let new_str = new.to_string_lossy().into_owned();
        if old_str.is_empty() || old_str == new_str {
            return Ok(0);
        }
        // Trailing separator keeps the match component-aligned (`/a/b` must not
        // match `/a/bc`).
        let old_prefix = format!("{}{}", old_str, std::path::MAIN_SEPARATOR);
        let new_prefix = format!("{}{}", new_str, std::path::MAIN_SEPARATOR);

        let conn = self.conn.lock();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("Failed to start rename transaction: {}", e))?;

        // Comparing paths in Rust avoids SQL `LIKE` metacharacter surprises
        // (`_` and `%` are legal in file names).
        let all_paths: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT path FROM nodes")
                .map_err(|e| format!("Failed to prepare rename query: {}", e))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| format!("Failed to read paths for rename: {}", e))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to decode paths for rename: {}", e))?
        };

        for path in &all_paths {
            if path == &new_str || path.starts_with(&new_prefix) {
                tx.execute("DELETE FROM nodes WHERE path = ?1", params![path])
                    .map_err(|e| format!("Failed to clear destination path: {}", e))?;
            }
        }

        let mut moved = 0usize;
        for path in &all_paths {
            let new_path = if path == &old_str {
                new_str.clone()
            } else if let Some(suffix) = path.strip_prefix(&old_prefix) {
                format!("{}{}{}", new_str, std::path::MAIN_SEPARATOR, suffix)
            } else {
                continue;
            };

            moved += tx
                .execute(
                    "UPDATE nodes SET path = ?1 WHERE path = ?2",
                    params![new_path, path],
                )
                .map_err(|e| format!("Failed to rename node path: {}", e))?;
        }

        // Only the moved root changes its display name; descendants keep theirs.
        if moved > 0
            && let Some(name) = new.file_name()
        {
            tx.execute(
                "UPDATE nodes SET name = ?1 WHERE path = ?2",
                params![name.to_string_lossy().to_string(), new_str],
            )
            .map_err(|e| format!("Failed to rename node: {}", e))?;
        }

        tx.commit()
            .map_err(|e| format!("Failed to commit rename: {}", e))?;
        Ok(moved)
    }

    pub fn clear(&self) -> Result<(), String> {
        let conn = self.conn.lock();
        let _ = conn.execute("DELETE FROM units", []);
        let _ = conn.execute("DELETE FROM usage", []);
        conn.execute("DELETE FROM nodes", [])
            .map_err(|e| format!("Failed to clear metadata: {}", e))?;

        Ok(())
    }

    pub fn count(&self) -> Result<usize, String> {
        let conn = self.conn.lock();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count metadata entries: {}", e))?;

        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Number of file (non-directory) nodes that still need semantic units.
    pub fn count_unembedded(&self) -> Result<usize, String> {
        let conn = self.conn.lock();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM nodes WHERE is_dir = 0 AND indexed_gen = 0",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to count unembedded nodes: {}", e))?;

        Ok(usize::try_from(count).unwrap_or(0))
    }

    pub fn record_open(&self, path: &Path) -> Result<(), String> {
        let conn = self.conn.lock();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO usage (path, opens, last_open) VALUES (?1, 1, ?2)
             ON CONFLICT(path) DO UPDATE SET opens = opens + 1, last_open = excluded.last_open",
            params![path.to_string_lossy().to_string(), now as i64],
        )
        .map_err(|e| format!("Failed to record file open: {}", e))?;
        Ok(())
    }

    pub fn load_usage(&self) -> Result<HashMap<PathBuf, (u32, u64)>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT path, opens, last_open FROM usage")
            .map_err(|e| format!("Failed to prepare usage query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let opens: i64 = row.get(1)?;
                let last_open: i64 = row.get(2)?;
                Ok((
                    PathBuf::from(path),
                    (opens.max(0) as u32, last_open.max(0) as u64),
                ))
            })
            .map_err(|e| format!("Failed to load usage rows: {}", e))?;

        let mut usage = HashMap::new();
        for (p, u) in rows.flatten() {
            usage.insert(p, u);
        }
        Ok(usage)
    }

    pub fn insert_unit(
        &self,
        node_id: u64,
        byte_start: usize,
        byte_end: usize,
        model_id: u32,
        vector: &[i8],
    ) -> Result<u64, String> {
        let conn = self.conn.lock();
        let u8_vec: Vec<u8> = vector.iter().map(|&b| b as u8).collect();
        conn.execute(
            "INSERT INTO units (node_id, byte_start, byte_end, model_id, vec) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                node_id as i64,
                byte_start as i64,
                byte_end as i64,
                model_id as i64,
                u8_vec,
            ],
        )
        .map_err(|e| format!("Failed to insert unit: {}", e))?;
        Ok(conn.last_insert_rowid() as u64)
    }

    pub fn load_all_units(&self) -> Result<Vec<(crate::embed::Unit, Vec<i8>)>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id, node_id, byte_start, byte_end, model_id, coalesce(vec, zeroblob(256)) FROM units")
            .map_err(|e| format!("Failed to prepare units query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let node_id: i64 = row.get(1)?;
                let byte_start: i64 = row.get(2)?;
                let byte_end: i64 = row.get(3)?;
                let model_id: i64 = row.get(4)?;
                let blob: Vec<u8> = row.get(5)?;
                let vec_i8: Vec<i8> = blob.into_iter().map(|b| b as i8).collect();
                Ok((
                    crate::embed::Unit {
                        id: id as u64,
                        node_id: node_id as u64,
                        byte_start: byte_start as usize,
                        byte_end: byte_end as usize,
                        model_id: model_id as u32,
                    },
                    vec_i8,
                ))
            })
            .map_err(|e| format!("Failed to load units: {}", e))?;

        let mut units = Vec::new();
        for u in rows.flatten() {
            units.push(u);
        }
        Ok(units)
    }

    pub fn delete_units_for_node(&self, node_id: u64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM units WHERE node_id = ?1", params![node_id as i64])
            .map_err(|e| format!("Failed to delete units for node: {}", e))?;
        Ok(())
    }

    /// Model id stored on the first semantic unit, if any units exist.
    pub fn units_model_id(&self) -> Result<Option<u32>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT DISTINCT model_id FROM units LIMIT 1")
            .map_err(|e| format!("Failed to prepare units model query: {}", e))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| format!("Failed to query units model id: {}", e))?;
        match rows
            .next()
            .map_err(|e| format!("Failed to read units model id: {}", e))?
        {
            Some(row) => {
                let model_id: i64 = row
                    .get(0)
                    .map_err(|e| format!("Failed to decode units model id: {}", e))?;
                Ok(Some(model_id as u32))
            }
            None => Ok(None),
        }
    }

    /// Drops every semantic unit and re-queues all file nodes for embedding.
    ///
    /// Used when the semantic model changed, because vectors from different
    /// models are not comparable. The in-memory [`crate::embed::UnitStore`] is
    /// cleared separately by the caller.
    pub fn reset_semantic_index(&self) -> Result<(), String> {
        let conn = self.conn.lock();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("Failed to start semantic reset: {}", e))?;
        tx.execute("DELETE FROM units", [])
            .map_err(|e| format!("Failed to clear semantic units: {}", e))?;
        tx.execute("UPDATE nodes SET indexed_gen = 0 WHERE is_dir = 0", [])
            .map_err(|e| format!("Failed to re-queue nodes for embedding: {}", e))?;
        tx.commit()
            .map_err(|e| format!("Failed to commit semantic reset: {}", e))?;
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    pub fn get_nodes_for_embedding(
        &self,
        limit: usize,
    ) -> Result<Vec<(u64, PathBuf, u64, u64, Option<String>)>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, path, size, modified, content_hash FROM nodes
                 WHERE is_dir = 0 AND (indexed_gen = 0 OR indexed_gen IS NULL)
                 ORDER BY modified DESC, size ASC
                 LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare embedding query: {}", e))?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let size: i64 = row.get(2)?;
                let modified: i64 = row.get(3)?;
                let content_hash: Option<String> = row.get(4)?;
                Ok((
                    id as u64,
                    PathBuf::from(path),
                    size as u64,
                    modified as u64,
                    content_hash,
                ))
            })
            .map_err(|e| format!("Failed to query nodes for embedding: {}", e))?;

        let mut nodes = Vec::new();
        for r in rows.flatten() {
            nodes.push(r);
        }
        Ok(nodes)
    }

    pub fn update_node_indexed_gen(
        &self,
        node_id: u64,
        indexed_gen: i64,
        content_hash: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE nodes SET indexed_gen = ?1, content_hash = coalesce(?2, content_hash) WHERE id = ?3",
            params![indexed_gen, content_hash, node_id as i64],
        )
        .map_err(|e| format!("Failed to update node indexed_gen: {}", e))?;
        Ok(())
    }

    pub fn get_node_id_by_path(&self, path: &Path) -> Result<Option<u64>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id FROM nodes WHERE path = ?1")
            .map_err(|e| e.to_string())?;

        let mut rows = stmt
            .query(params![path.to_string_lossy().to_string()])
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let id: i64 = row.get(0).map_err(|e| e.to_string())?;
            Ok(Some(id as u64))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDb {
        db: Option<MetadataDb>,
        dir: PathBuf,
    }

    impl TempDb {
        fn new(tag: &str) -> Self {
            let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "kensaku-metadata-test-{}-{}-{}",
                tag,
                std::process::id(),
                seq
            ));
            std::fs::create_dir_all(&dir).expect("create temp db dir");
            let db = MetadataDb::open_at(&dir.join("metadata.db")).expect("open temp db");
            Self {
                db: Some(db),
                dir,
            }
        }

        fn db(&self) -> &MetadataDb {
            self.db.as_ref().expect("db open")
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            // Close the connection before deleting the directory (Windows).
            self.db.take();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn entry(path: &Path, is_dir: bool) -> FileEntry {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        FileEntry {
            id: 0,
            path: path.to_path_buf(),
            name,
            extension: if is_dir {
                None
            } else {
                path.extension().map(|ext| ext.to_string_lossy().to_lowercase())
            },
            size: 1,
            modified: 1,
            is_dir,
            dev: 0,
            ino: 0,
            char_bloom: 0,
        }
    }

    fn paths(db: &MetadataDb) -> Vec<PathBuf> {
        db.load_all()
            .expect("load nodes")
            .into_iter()
            .map(|entry| entry.path)
            .collect()
    }

    #[test]
    fn rename_prefix_rewrites_subtree_and_preserves_siblings() {
        let temp = TempDb::new("rename-prefix");
        let db = temp.db();
        let old_root = temp.dir.join("old");
        let new_root = temp.dir.join("new");

        db.upsert(&entry(&old_root, true)).unwrap();
        db.upsert(&entry(&old_root.join("a.txt"), false)).unwrap();
        db.upsert(&entry(&old_root.join("nested"), true)).unwrap();
        db.upsert(&entry(&old_root.join("nested").join("b.rs"), false))
            .unwrap();
        // Component-aligned prefix: a sibling with a longer name must survive.
        let sibling = temp.dir.join("oldish").join("c.txt");
        db.upsert(&entry(&sibling, false)).unwrap();

        let moved = db
            .rename_prefix(&old_root, &new_root)
            .expect("rename prefix");
        assert_eq!(moved, 4);

        let after = paths(db);
        assert!(after.contains(&new_root));
        assert!(after.contains(&new_root.join("a.txt")));
        assert!(after.contains(&new_root.join("nested")));
        assert!(after.contains(&new_root.join("nested").join("b.rs")));
        assert!(after.contains(&sibling));
        assert!(!after.iter().any(|path| path.starts_with(&old_root)));

        let renamed_root = db
            .load_all()
            .expect("load nodes")
            .into_iter()
            .find(|entry| entry.path == new_root)
            .expect("renamed root row");
        assert_eq!(renamed_root.name, "new");
        assert!(renamed_root.is_dir);
    }
}
