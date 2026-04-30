use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub size: i64,
    pub hash: String,
    pub mtime: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTagRecord {
    pub file_id: i64,
    pub tag_id: i64,
    pub assigned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSummary {
    pub summary: Option<String>,
    pub tag: String,
    pub file_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub files: Vec<FileRecord>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_files: i64,
    pub total_size: i64,
    pub total_tags: i64,
    pub untagged_files: i64,
}

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub async fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).context("Failed to open database")?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                hash TEXT NOT NULL,
                mtime INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
            CREATE INDEX IF NOT EXISTS idx_files_hash ON files(hash);
            CREATE INDEX IF NOT EXISTS idx_files_name ON files(name);
            
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL DEFAULT '#6366f1',
                count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            
            CREATE TABLE IF NOT EXISTS file_tags (
                file_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                assigned_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (file_id, tag_id),
                FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );
            
            CREATE INDEX IF NOT EXISTS idx_file_tags_file ON file_tags(file_id);
            CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag_id);
            
            CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                name,
                path,
                content='files',
                content_rowid='id'
            );
            
            CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
                INSERT INTO files_fts(rowid, name, path) VALUES (new.id, new.name, new.path);
            END;
            
            CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
                INSERT INTO files_fts(files_fts, rowid, name, path) VALUES ('delete', old.id, old.name, old.path);
            END;
            
            CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
                INSERT INTO files_fts(files_fts, rowid, name, path) VALUES ('delete', old.id, old.name, old.path);
                INSERT INTO files_fts(rowid, name, path) VALUES (new.id, new.name, new.path);
            END;
            
            -- Graveyard for conflict resolution
            CREATE TABLE IF NOT EXISTS graveyard (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                original_path TEXT NOT NULL,
                object_hash TEXT NOT NULL,
                original_hash TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                compressed_bytes INTEGER NOT NULL,
                replaced_by TEXT NOT NULL,
                replaced_at TEXT NOT NULL,
                peer_node_id TEXT,
                expires_at TEXT NOT NULL,
                tags TEXT,
                summary TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            
            CREATE INDEX IF NOT EXISTS idx_graveyard_path ON graveyard(original_path);
            CREATE INDEX IF NOT EXISTS idx_graveyard_expires ON graveyard(expires_at);
            
            -- Sync peers table
            CREATE TABLE IF NOT EXISTS peers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id TEXT NOT NULL UNIQUE,
                display_name TEXT,
                ip TEXT,
                port INTEGER,
                last_seen TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            
            CREATE INDEX IF NOT EXISTS idx_peers_node ON peers(node_id);
            
            -- Sync state tracking
            CREATE TABLE IF NOT EXISTS sync_state (
                peer_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                synced_at TEXT NOT NULL,
                PRIMARY KEY (peer_id, file_path)
            );
            
            -- Settings key-value store
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
        "#).context("Failed to run migrations")?;

        tracing::info!("Database migrations complete");
        Ok(())
    }

    pub async fn upsert_file(
        &self,
        path: &str,
        name: &str,
        size: i64,
        hash: &str,
        mtime: i64,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            r#"INSERT INTO files (path, name, size, hash, mtime, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(path) DO UPDATE SET
                   name = excluded.name,
                   size = excluded.size,
                   hash = excluded.hash,
                   mtime = excluded.mtime,
                   updated_at = excluded.updated_at"#,
            params![path, name, size, hash, mtime, now],
        )?;

        let id = conn.query_row(
            "SELECT id FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    pub async fn delete_file(&self, path: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock();

        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = id {
            conn.execute("DELETE FROM files WHERE id = ?1", params![id])?;
        }

        Ok(id)
    }

    pub async fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>> {
        let conn = self.conn.lock();

        let record = conn.query_row(
            "SELECT id, path, name, size, hash, mtime, created_at, updated_at FROM files WHERE path = ?1",
            params![path],
            |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    mtime: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    tags: Vec::new(),
                })
            },
        ).optional()?;

        Ok(record)
    }

    pub async fn get_file_by_hash(&self, hash: &str) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, path, name, size, hash, mtime, created_at, updated_at FROM files WHERE hash = ?1"
        )?;

        let records = stmt
            .query_map(params![hash], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    mtime: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    pub async fn list_files(&self, limit: i64, offset: i64) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, path, name, size, hash, mtime, created_at, updated_at 
             FROM files ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let records = stmt
            .query_map(params![limit, offset], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    mtime: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    pub async fn list_files_by_tag(
        &self,
        tag_name: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FileRecord>> {
        let tag_name = tag_name.to_lowercase();
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            r#"SELECT f.id, f.path, f.name, f.size, f.hash, f.mtime, f.created_at, f.updated_at
               FROM files f
               JOIN file_tags ft ON f.id = ft.file_id
               JOIN tags t ON ft.tag_id = t.id
               WHERE LOWER(t.name) = ?1
               ORDER BY f.updated_at DESC
               LIMIT ?2 OFFSET ?3"#,
        )?;

        let records = stmt
            .query_map(params![&tag_name, limit, offset], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    mtime: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    pub async fn search_files(&self, query: &str, limit: i64, offset: i64) -> Result<SearchResult> {
        let conn = self.conn.lock();

        let search_query = format!("{}*", query.replace('"', "\"\""));

        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM files_fts WHERE files_fts MATCH ?1",
            params![search_query],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            r#"SELECT f.id, f.path, f.name, f.size, f.hash, f.mtime, f.created_at, f.updated_at
               FROM files f
               JOIN files_fts fts ON f.id = fts.rowid
               WHERE files_fts MATCH ?1
               ORDER BY f.updated_at DESC
               LIMIT ?2 OFFSET ?3"#,
        )?;

        let files = stmt
            .query_map(params![search_query, limit, offset], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    mtime: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    tags: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SearchResult { files, total })
    }

    pub async fn create_tag(&self, name: &str, color: &str) -> Result<TagRecord> {
        let name = name.to_lowercase();
        let conn = self.conn.lock();

        conn.execute(
            "INSERT INTO tags (name, color) VALUES (?1, ?2) ON CONFLICT(name) DO UPDATE SET color = excluded.color",
            params![&name, color],
        )?;

        let record = conn.query_row(
            "SELECT id, name, color, count, created_at FROM tags WHERE name = ?1",
            params![&name],
            |row| {
                Ok(TagRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    count: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )?;

        Ok(record)
    }

    pub async fn list_tags(&self) -> Result<Vec<TagRecord>> {
        let conn = self.conn.lock();

        let mut stmt =
            conn.prepare("SELECT id, name, color, count, created_at FROM tags ORDER BY name")?;

        let records = stmt
            .query_map([], |row| {
                Ok(TagRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    count: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    pub async fn delete_tag(&self, name: &str) -> Result<bool> {
        let name = name.to_lowercase();
        let conn = self.conn.lock();

        let deleted = conn.execute("DELETE FROM tags WHERE name = ?1", params![name])?;

        Ok(deleted > 0)
    }

    pub async fn tag_file(&self, file_id: i64, tag_name: &str) -> Result<()> {
        let tag_name = tag_name.to_lowercase();
        let conn = self.conn.lock();

        conn.execute(
            "INSERT INTO tags (name, color) VALUES (?1, '#6366f1') ON CONFLICT(name) DO NOTHING",
            params![tag_name],
        )?;

        let tag_id: i64 = conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            params![tag_name],
            |row| row.get(0),
        )?;

        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO file_tags (file_id, tag_id, assigned_at) VALUES (?1, ?2, ?3) 
             ON CONFLICT(file_id, tag_id) DO NOTHING",
            params![file_id, tag_id, now],
        )?;

        conn.execute(
            "UPDATE tags SET count = (SELECT COUNT(*) FROM file_tags WHERE tag_id = ?1) WHERE id = ?1",
            params![tag_id],
        )?;

        Ok(())
    }

    pub async fn untag_file(&self, file_id: i64, tag_name: &str) -> Result<()> {
        let tag_name = tag_name.to_lowercase();
        let conn = self.conn.lock();

        let tag_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = ?1",
                params![tag_name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(tag_id) = tag_id {
            conn.execute(
                "DELETE FROM file_tags WHERE file_id = ?1 AND tag_id = ?2",
                params![file_id, tag_id],
            )?;

            conn.execute(
                "UPDATE tags SET count = MAX(0, count - 1) WHERE id = ?1",
                params![tag_id],
            )?;
        }

        Ok(())
    }

    pub async fn get_file_tags(&self, file_id: i64) -> Result<Vec<TagRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            r#"SELECT t.id, t.name, t.color, t.count, t.created_at
               FROM tags t
               JOIN file_tags ft ON t.id = ft.tag_id
               WHERE ft.file_id = ?1
               ORDER BY t.name"#,
        )?;

        let records = stmt
            .query_map(params![file_id], |row| {
                Ok(TagRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    count: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    pub async fn get_stats(&self) -> Result<Stats> {
        let conn = self.conn.lock();

        let total_files: i64 =
            conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;

        let total_size: i64 =
            conn.query_row("SELECT COALESCE(SUM(size), 0) FROM files", [], |row| {
                row.get(0)
            })?;

        let total_tags: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;

        let untagged_files: i64 = conn.query_row(
            r#"SELECT COUNT(*) FROM files f 
               WHERE NOT EXISTS (SELECT 1 FROM file_tags WHERE file_id = f.id)"#,
            [],
            |row| row.get(0),
        )?;

        Ok(Stats {
            total_files,
            total_size,
            total_tags,
            untagged_files,
        })
    }

    pub async fn get_manifest(&self) -> Result<Vec<(String, String, i64, i64)>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare("SELECT path, hash, size, mtime FROM files")?;

        let records = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();

        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        Ok(value)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now],
        )?;

        Ok(())
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}
