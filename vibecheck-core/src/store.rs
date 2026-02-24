#![cfg(feature = "corpus")]

use std::path::Path;

use rusqlite::{Connection, Result, params};

/// A persistent corpus and trend store backed by SQLite.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) the store at `path`, applying WAL mode and creating tables.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            // best-effort: if this fails, Connection::open will surface the error
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS corpus_entries (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                file_hash    TEXT    NOT NULL,
                path         TEXT,
                attribution  TEXT    NOT NULL,
                confidence   REAL    NOT NULL,
                created_at   TEXT    NOT NULL DEFAULT (datetime('now'))
            );
            CREATE UNIQUE INDEX IF NOT EXISTS corpus_entries_hash
                ON corpus_entries(file_hash);

            CREATE TABLE IF NOT EXISTS trend_entries (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                file_hash    TEXT    NOT NULL,
                attribution  TEXT    NOT NULL,
                confidence   REAL    NOT NULL,
                recorded_at  TEXT    NOT NULL DEFAULT (datetime('now'))
            );",
        )?;
        Ok(Self { conn })
    }

    /// Insert a corpus entry. Silently ignores duplicates (same file_hash).
    pub fn insert_corpus(
        &self,
        file_hash: &str,
        path: Option<&str>,
        attribution: &str,
        confidence: f64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO corpus_entries (file_hash, path, attribution, confidence)
             VALUES (?1, ?2, ?3, ?4)",
            params![file_hash, path, attribution, confidence],
        )?;
        Ok(())
    }

    /// Record a trend entry (always inserts, does not deduplicate).
    pub fn record_trend(
        &self,
        file_hash: &str,
        attribution: &str,
        confidence: f64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trend_entries (file_hash, attribution, confidence)
             VALUES (?1, ?2, ?3)",
            params![file_hash, attribution, confidence],
        )?;
        Ok(())
    }
}
