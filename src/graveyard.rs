use anyhow::Result;
use chrono::{Duration, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zstd::encode_all;

use crate::config::GraveyardConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraveyardEntry {
    pub id: i64,
    pub original_path: String,
    pub object_hash: String,
    pub original_hash: String,
    pub size_bytes: i64,
    pub compressed_bytes: i64,
    pub replaced_by: String,
    pub replaced_at: String,
    pub peer_node_id: Option<String>,
    pub expires_at: String,
    pub tags: Option<String>,
    pub summary: Option<String>,
}

pub struct GraveyardManager {
    conn: Arc<Mutex<Connection>>,
    config: GraveyardConfig,
}

impl GraveyardManager {
    pub fn new(config: &GraveyardConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.path)?;
        std::fs::create_dir_all(config.path.join("objects"))?;

        let db_path = config.path.join("graveyard.db");
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS entries (
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
            CREATE INDEX IF NOT EXISTS idx_entries_path ON entries(original_path);
            CREATE INDEX IF NOT EXISTS idx_entries_expires ON entries(expires_at);
        "#)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            config: config.clone(),
        })
    }

    pub fn entomb(
        &self,
        path: &Path,
        original_hash: &str,
        replaced_by_hash: &str,
        peer_node_id: Option<&str>,
        tags: Option<&str>,
        summary: Option<&str>,
    ) -> Result<GraveyardEntry> {
        let content = std::fs::read(path)?;
        let size_bytes = content.len() as i64;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let object_hash = hex::encode(hasher.finalize());

        let objects_dir = self.config.path.join("objects");
        let obj_subdir = objects_dir.join(&object_hash[..2]);
        std::fs::create_dir_all(&obj_subdir)?;
        let obj_path = obj_subdir.join(&object_hash);

        let compressed = encode_all(content.as_slice(), 3)?;
        let compressed_bytes = compressed.len() as i64;
        
        std::fs::write(&obj_path, &compressed)?;

        let now = Utc::now();
        let replaced_at = now.to_rfc3339();
        let expires_at = (now + Duration::days(self.config.ttl_days)).to_rfc3339();

        let conn = self.conn.lock();
        conn.execute(
            r#"INSERT INTO entries 
               (original_path, object_hash, original_hash, size_bytes, compressed_bytes,
                replaced_by, replaced_at, peer_node_id, expires_at, tags, summary)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                path.to_string_lossy().to_string(),
                object_hash,
                original_hash,
                size_bytes,
                compressed_bytes,
                replaced_by_hash,
                replaced_at,
                peer_node_id,
                expires_at,
                tags,
                summary,
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(GraveyardEntry {
            id,
            original_path: path.to_string_lossy().to_string(),
            object_hash,
            original_hash: original_hash.to_string(),
            size_bytes,
            compressed_bytes,
            replaced_by: replaced_by_hash.to_string(),
            replaced_at,
            peer_node_id: peer_node_id.map(String::from),
            expires_at,
            tags: tags.map(String::from),
            summary: summary.map(String::from),
        })
    }

    pub fn restore(&self, entry_id: i64) -> Result<PathBuf> {
        let conn = self.conn.lock();

        let entry: GraveyardEntry = conn.query_row(
            "SELECT * FROM entries WHERE id = ?1",
            params![entry_id],
            |row| {
                Ok(GraveyardEntry {
                    id: row.get(0)?,
                    original_path: row.get(1)?,
                    object_hash: row.get(2)?,
                    original_hash: row.get(3)?,
                    size_bytes: row.get(4)?,
                    compressed_bytes: row.get(5)?,
                    replaced_by: row.get(6)?,
                    replaced_at: row.get(7)?,
                    peer_node_id: row.get(8)?,
                    expires_at: row.get(9)?,
                    tags: row.get(10)?,
                    summary: row.get(11)?,
                })
            },
        )?;

        drop(conn);

        let objects_dir = self.config.path.join("objects");
        let obj_subdir = objects_dir.join(&entry.object_hash[..2]);
        let obj_path = obj_subdir.join(&entry.object_hash);

        let compressed = std::fs::read(&obj_path)?;
        let decompressed = zstd::decode_all(compressed.as_slice())?;

        let original_path = PathBuf::from(&entry.original_path);
        if let Some(parent) = original_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&original_path, &decompressed)?;

        tracing::info!("Restored {} from graveyard", entry.original_path);

        Ok(original_path)
    }

    pub fn reap(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, object_hash FROM entries WHERE expires_at < ?1"
        )?;
        
        let expired: Vec<(i64, String)> = stmt.query_map(params![now], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?.collect::<Result<Vec<_>, _>>()?;

        let mut deleted = 0;
        for (id, object_hash) in expired {
            conn.execute("DELETE FROM entries WHERE id = ?1", params![id])?;

            let still_exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entries WHERE object_hash = ?1",
                params![object_hash],
                |row| row.get(0),
            )?;

            if still_exists == 0 {
                let objects_dir = self.config.path.join("objects");
                let obj_subdir = objects_dir.join(&object_hash[..2]);
                let obj_path = obj_subdir.join(&object_hash);
                if obj_path.exists() {
                    std::fs::remove_file(&obj_path)?;
                }
            }

            deleted += 1;
        }

        tracing::info!("Reaped {} expired entries", deleted);
        Ok(deleted)
    }

    pub fn get_versions(&self, original_path: &str) -> Result<Vec<GraveyardEntry>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM entries WHERE original_path = ?1 ORDER BY replaced_at DESC"
        )?;

        let entries = stmt.query_map(params![original_path], |row| {
            Ok(GraveyardEntry {
                id: row.get(0)?,
                original_path: row.get(1)?,
                object_hash: row.get(2)?,
                original_hash: row.get(3)?,
                size_bytes: row.get(4)?,
                compressed_bytes: row.get(5)?,
                replaced_by: row.get(6)?,
                replaced_at: row.get(7)?,
                peer_node_id: row.get(8)?,
                expires_at: row.get(9)?,
                tags: row.get(10)?,
                summary: row.get(11)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }
}