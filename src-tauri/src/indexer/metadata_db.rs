use crate::indexer::walker::FileEntry;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

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
             CREATE INDEX IF NOT EXISTS nodes_path ON nodes(path);",
        )
        .map_err(|e| format!("Failed to initialize metadata database: {}", e))?;

        // Migration for databases created before file identity was tracked.
        // `ALTER TABLE ... ADD COLUMN` fails with "duplicate column name" when
        // the column already exists, which is expected and safe to ignore.
        for ddl in [
            "ALTER TABLE nodes ADD COLUMN dev INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE nodes ADD COLUMN ino INTEGER NOT NULL DEFAULT 0",
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

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn load_all(&self) -> Result<Vec<FileEntry>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT path, name, extension, size, modified, is_dir, dev, ino FROM nodes",
            )
            .map_err(|e| format!("Failed to prepare metadata query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let name: String = row.get(1)?;
                let extension: Option<String> = row.get(2)?;
                let size: i64 = row.get(3)?;
                let modified: i64 = row.get(4)?;
                let is_dir: i64 = row.get(5)?;
                let dev: i64 = row.get(6)?;
                let ino: i64 = row.get(7)?;

                Ok(FileEntry {
                    path: PathBuf::from(path),
                    name,
                    extension,
                    size: u64::try_from(size).unwrap_or(0),
                    modified: u64::try_from(modified).unwrap_or(0),
                    is_dir: is_dir != 0,
                    dev: u64::try_from(dev).unwrap_or(0),
                    ino: u64::try_from(ino).unwrap_or(0),
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
               ino = excluded.ino",
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
        conn.execute(
            "DELETE FROM nodes WHERE path = ?1",
            params![path.to_string_lossy().to_string()],
        )
        .map_err(|e| format!("Failed to delete metadata entry: {}", e))?;

        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        let conn = self.conn.lock();
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
}
