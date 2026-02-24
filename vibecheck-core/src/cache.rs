use std::path::{Path, PathBuf};

use redb::{Database, TableDefinition};
use sha2::{Digest, Sha256};

use crate::report::Report;

const CACHE_TABLE: TableDefinition<&[u8], &str> = TableDefinition::new("cache");

pub struct Cache {
    db: Database,
}

impl Cache {
    /// Open (or create) the cache database at `dir/cache.redb`.
    pub fn open(dir: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        std::fs::create_dir_all(dir)?;
        let db_path = dir.join("cache.redb");
        let db = Database::create(&db_path)?;
        Ok(Self { db })
    }

    /// Default cache directory: `$CACHE_DIR/vibecheck/`.
    pub fn default_path() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("vibecheck")
    }

    /// Compute the SHA-256 hash of `content`.
    pub fn hash_content(content: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Look up a cached `Report` by file-content hash.
    pub fn get(&self, hash: &[u8; 32]) -> Option<Report> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(CACHE_TABLE).ok()?;
        let guard = table.get(hash.as_slice()).ok()??;
        serde_json::from_str(guard.value()).ok()
    }

    /// Store a `Report` under the given file-content hash.
    pub fn put(
        &self,
        hash: &[u8; 32],
        report: &Report,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string(report)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CACHE_TABLE)?;
            table.insert(hash.as_slice(), json.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}
