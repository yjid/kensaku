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
    pub fn open() -> Result<Self, String> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| "Could not determine config directory".to_string())?;
        let db_dir = config_dir.join("kensaku");
        std::fs::create_dir_all(&db_dir)
            .map_err(|e| format!("Failed to create metadata directory: {}", e))?;

        let db_path = db_dir.join("metadata.db");
        let conn = Connection::open(&db_path)
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
