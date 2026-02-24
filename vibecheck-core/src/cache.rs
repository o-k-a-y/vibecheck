use std::path::{Path, PathBuf};

use redb::{Database, TableDefinition};
use sha2::{Digest, Sha256};

use crate::merkle::DirNode;
use crate::report::{Report, SymbolReport};

const CACHE_TABLE: TableDefinition<&[u8], &str> = TableDefinition::new("cache");

/// Stores `Vec<SymbolReport>` keyed by the same SHA-256 hash as `CACHE_TABLE`.
/// Kept separate so that regular `analyze_file` results are never inflated with
/// symbol data that was never requested.
const SYMBOL_TABLE: TableDefinition<&[u8], &str> = TableDefinition::new("symbol_cache");

/// Key: canonical directory path (as a UTF-8 string).
/// Value: JSON-serialised `DirNode`.
const DIR_TABLE: TableDefinition<&str, &str> = TableDefinition::new("dir_cache");

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

    /// Look up cached `SymbolReport`s by file-content hash.
    pub fn get_symbols(&self, hash: &[u8; 32]) -> Option<Vec<SymbolReport>> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(SYMBOL_TABLE).ok()?;
        let guard = table.get(hash.as_slice()).ok()??;
        serde_json::from_str(guard.value()).ok()
    }

    /// Store `SymbolReport`s under the given file-content hash.
    pub fn put_symbols(
        &self,
        hash: &[u8; 32],
        symbols: &[SymbolReport],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string(symbols)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(SYMBOL_TABLE)?;
            table.insert(hash.as_slice(), json.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Look up a cached `DirNode` by directory path.
    pub fn get_dir(&self, dir: &Path) -> Option<DirNode> {
        let key = dir.to_str()?;
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(DIR_TABLE).ok()?;
        let guard = table.get(key).ok()??;
        serde_json::from_str(guard.value()).ok()
    }

    /// Store a `DirNode` under the given directory path.
    pub fn set_dir(
        &self,
        dir: &Path,
        node: &DirNode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = dir.to_str().ok_or("non-UTF-8 path")?;
        let json = serde_json::to_string(node)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(DIR_TABLE)?;
            table.insert(key, json.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::DirNode;

    #[test]
    fn file_cache_round_trip() {
        use crate::report::{Attribution, ModelFamily, Report, ReportMetadata};
        use std::collections::HashMap;

        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();

        let hash = [7u8; 32];
        let report = Report {
            attribution: Attribution {
                primary: ModelFamily::Claude,
                confidence: 0.9,
                scores: HashMap::from([(ModelFamily::Claude, 0.9), (ModelFamily::Human, 0.1)]),
            },
            signals: vec![],
            metadata: ReportMetadata {
                file_path: None,
                lines_of_code: 10,
                signal_count: 0,
            },
            symbol_reports: None,
        };

        cache.put(&hash, &report).unwrap();
        let retrieved = cache.get(&hash).unwrap();
        assert_eq!(retrieved.metadata.lines_of_code, 10);
        assert_eq!(retrieved.attribution.primary, ModelFamily::Claude);
    }

    #[test]
    fn file_cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        assert!(cache.get(&[0u8; 32]).is_none());
    }

    #[test]
    fn dir_cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();

        let key = dir.path().join("myproject");
        std::fs::create_dir(&key).unwrap();
        let node = DirNode {
            hash: [42u8; 32],
            children: vec!["a.rs".to_string(), "b.rs".to_string()],
        };

        cache.set_dir(&key, &node).unwrap();
        let retrieved = cache.get_dir(&key).unwrap();
        assert_eq!(retrieved.hash, [42u8; 32]);
        assert_eq!(retrieved.children, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn dir_cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        assert!(cache.get_dir(&dir.path().join("nonexistent")).is_none());
    }

    #[test]
    fn symbol_cache_round_trip() {
        use crate::report::{Attribution, ModelFamily, Signal, SymbolMetadata, SymbolReport};
        use std::collections::HashMap;

        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let hash = [9u8; 32];

        let syms = vec![SymbolReport {
            metadata: SymbolMetadata {
                name: "my_fn".to_string(),
                kind: "function".to_string(),
                start_line: 1,
                end_line: 5,
            },
            attribution: Attribution {
                primary: ModelFamily::Claude,
                confidence: 0.85,
                scores: HashMap::from([(ModelFamily::Claude, 0.85)]),
            },
            signals: vec![Signal {
                source: "test".to_string(),
                description: "test signal".to_string(),
                family: ModelFamily::Claude,
                weight: 1.0,
            }],
        }];

        cache.put_symbols(&hash, &syms).unwrap();
        let retrieved = cache.get_symbols(&hash).unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].metadata.name, "my_fn");
    }

    #[test]
    fn symbol_cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        assert!(cache.get_symbols(&[0u8; 32]).is_none());
    }
}
