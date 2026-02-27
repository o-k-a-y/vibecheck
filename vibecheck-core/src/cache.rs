use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use redb::{Database, TableDefinition};
use sha2::{Digest, Sha256};

use crate::merkle::DirNode;
use crate::report::{Report, SymbolReport};

/// SHA-256 of the embedded heuristics.toml, computed once.
/// Mixed into every content hash so cache entries auto-invalidate
/// when signal definitions change.
fn heuristics_epoch() -> &'static [u8; 32] {
    static EPOCH: OnceLock<[u8; 32]> = OnceLock::new();
    EPOCH.get_or_init(|| {
        let mut h = Sha256::new();
        h.update(include_str!("../heuristics.toml").as_bytes());
        let result = h.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    })
}

const NS_REPORT: u8 = b'r';
const NS_SYMBOL: u8 = b's';
const NS_DIR: u8 = b'd';

#[derive(Debug)]
pub enum CacheError {
    Backend(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::Backend(e) => write!(f, "cache backend error: {e}"),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CacheError::Backend(e) => Some(&**e),
        }
    }
}

/// Low-level key-value cache backend. Implementations handle raw bytes;
/// higher-level typed access is provided by [`Cache`].
pub trait CacheBackend: Send + Sync {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, CacheError>;
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), CacheError>;
    fn delete(&self, key: &[u8]) -> Result<(), CacheError>;
    fn contains(&self, key: &[u8]) -> Result<bool, CacheError>;
}

// ---------------------------------------------------------------------------
// RedbBackend
// ---------------------------------------------------------------------------

const KV_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("kv_v2");

/// Persistent cache backend backed by a redb embedded database.
pub struct RedbBackend {
    db: Database,
}

impl RedbBackend {
    pub fn open(dir: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        std::fs::create_dir_all(dir)?;
        let db_path = dir.join("cache.redb");
        let db = Database::create(&db_path)?;
        Ok(Self { db })
    }
}

impl CacheBackend for RedbBackend {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, CacheError> {
        let read_txn = self.db.begin_read().map_err(|e| CacheError::Backend(e.into()))?;
        let table = match read_txn.open_table(KV_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        match table.get(key) {
            Ok(Some(guard)) => Ok(Some(guard.value().to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(CacheError::Backend(e.into())),
        }
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), CacheError> {
        let write_txn = self.db.begin_write().map_err(|e| CacheError::Backend(e.into()))?;
        {
            let mut table = write_txn.open_table(KV_TABLE).map_err(|e| CacheError::Backend(e.into()))?;
            table.insert(key, value).map_err(|e| CacheError::Backend(e.into()))?;
        }
        write_txn.commit().map_err(|e| CacheError::Backend(e.into()))?;
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), CacheError> {
        let write_txn = self.db.begin_write().map_err(|e| CacheError::Backend(e.into()))?;
        {
            let mut table = match write_txn.open_table(KV_TABLE) {
                Ok(t) => t,
                Err(_) => return Ok(()),
            };
            table.remove(key).map_err(|e| CacheError::Backend(e.into()))?;
        }
        write_txn.commit().map_err(|e| CacheError::Backend(e.into()))?;
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> Result<bool, CacheError> {
        self.get(key).map(|v| v.is_some())
    }
}

// ---------------------------------------------------------------------------
// InMemoryBackend
// ---------------------------------------------------------------------------

/// In-memory cache backend using a `HashMap`.
/// When `max_entries` is reached, new insertions for unknown keys are dropped.
pub struct InMemoryBackend {
    store: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
    max_entries: usize,
}

impl InMemoryBackend {
    pub fn new(max_entries: usize) -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
            max_entries,
        }
    }
}

impl CacheBackend for InMemoryBackend {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, CacheError> {
        let store = self.store.lock().unwrap();
        Ok(store.get(key).cloned())
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), CacheError> {
        let mut store = self.store.lock().unwrap();
        if store.len() >= self.max_entries && !store.contains_key(key) {
            return Ok(());
        }
        store.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), CacheError> {
        let mut store = self.store.lock().unwrap();
        store.remove(key);
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> Result<bool, CacheError> {
        let store = self.store.lock().unwrap();
        Ok(store.contains_key(key))
    }
}

// ---------------------------------------------------------------------------
// TieredBackend
// ---------------------------------------------------------------------------

/// Two-tier cache: fast in-memory hot tier with persistent cold tier.
/// Reads check hot first, promoting cold hits. Writes go to both tiers.
pub struct TieredBackend {
    hot: InMemoryBackend,
    cold: RedbBackend,
}

impl TieredBackend {
    pub fn new(hot: InMemoryBackend, cold: RedbBackend) -> Self {
        Self { hot, cold }
    }
}

impl CacheBackend for TieredBackend {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, CacheError> {
        if let Some(val) = self.hot.get(key)? {
            return Ok(Some(val));
        }
        if let Some(val) = self.cold.get(key)? {
            let _ = self.hot.put(key, &val);
            return Ok(Some(val));
        }
        Ok(None)
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), CacheError> {
        let _ = self.hot.put(key, value);
        self.cold.put(key, value)
    }

    fn delete(&self, key: &[u8]) -> Result<(), CacheError> {
        let _ = self.hot.delete(key);
        self.cold.delete(key)
    }

    fn contains(&self, key: &[u8]) -> Result<bool, CacheError> {
        if self.hot.contains(key)? {
            return Ok(true);
        }
        self.cold.contains(key)
    }
}

// ---------------------------------------------------------------------------
// Cache — public API (unchanged signatures)
// ---------------------------------------------------------------------------

/// Content-addressed cache for analysis reports, symbol data, and directory
/// hashes. Backed by a [`CacheBackend`] (default: [`TieredBackend`]).
pub struct Cache {
    backend: Box<dyn CacheBackend>,
}

impl Cache {
    /// Open (or create) the cache database at `dir/cache.redb`.
    pub fn open(dir: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let cold = RedbBackend::open(dir)?;
        let hot = InMemoryBackend::new(1024);
        Ok(Self {
            backend: Box::new(TieredBackend::new(hot, cold)),
        })
    }

    /// Construct a cache with a custom backend.
    pub fn with_backend(backend: Box<dyn CacheBackend>) -> Self {
        Self { backend }
    }

    /// Resolve the cache directory, checking (in priority order):
    /// 1. Explicit override (e.g. from `.vibecheck` config `[cache] dir`)
    /// 2. `VIBECHECK_CACHE_DIR` environment variable
    /// 3. Platform default: `~/.cache/vibecheck/`
    pub fn resolve_path(config_override: Option<&Path>) -> PathBuf {
        if let Some(dir) = config_override {
            return dir.to_path_buf();
        }
        if let Ok(dir) = std::env::var("VIBECHECK_CACHE_DIR") {
            return PathBuf::from(dir);
        }
        Self::default_path()
    }

    /// Platform default cache directory: `~/.cache/vibecheck/` (Linux),
    /// `~/Library/Caches/vibecheck/` (macOS), `%LOCALAPPDATA%/vibecheck/` (Windows).
    pub fn default_path() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("vibecheck")
    }

    /// Compute the SHA-256 hash of `content`, mixed with the heuristics epoch
    /// so that cache entries auto-invalidate when signal definitions change.
    pub fn hash_content(content: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(heuristics_epoch());
        hasher.update(content);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    fn ns_key(ns: u8, key: &[u8]) -> Vec<u8> {
        let mut k = Vec::with_capacity(1 + key.len());
        k.push(ns);
        k.extend_from_slice(key);
        k
    }

    /// Look up a cached `Report` by file-content hash.
    pub fn get(&self, hash: &[u8; 32]) -> Option<Report> {
        let key = Self::ns_key(NS_REPORT, hash);
        let bytes = self.backend.get(&key).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    /// Store a `Report` under the given file-content hash.
    pub fn put(
        &self,
        hash: &[u8; 32],
        report: &Report,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = Self::ns_key(NS_REPORT, hash);
        let json = serde_json::to_vec(report)?;
        self.backend.put(&key, &json)?;
        Ok(())
    }

    /// Look up cached `SymbolReport`s by file-content hash.
    pub fn get_symbols(&self, hash: &[u8; 32]) -> Option<Vec<SymbolReport>> {
        let key = Self::ns_key(NS_SYMBOL, hash);
        let bytes = self.backend.get(&key).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    /// Store `SymbolReport`s under the given file-content hash.
    pub fn put_symbols(
        &self,
        hash: &[u8; 32],
        symbols: &[SymbolReport],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = Self::ns_key(NS_SYMBOL, hash);
        let json = serde_json::to_vec(symbols)?;
        self.backend.put(&key, &json)?;
        Ok(())
    }

    /// Look up a cached `DirNode` by directory path.
    pub fn get_dir(&self, dir: &Path) -> Option<DirNode> {
        let path_str = dir.to_str()?;
        let key = Self::ns_key(NS_DIR, path_str.as_bytes());
        let bytes = self.backend.get(&key).ok()??;
        serde_json::from_slice(&bytes).ok()
    }

    /// Store a `DirNode` under the given directory path.
    pub fn set_dir(
        &self,
        dir: &Path,
        node: &DirNode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path_str = dir.to_str().ok_or("non-UTF-8 path")?;
        let key = Self::ns_key(NS_DIR, path_str.as_bytes());
        let json = serde_json::to_vec(node)?;
        self.backend.put(&key, &json)?;
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
            signals: vec![Signal::new("", "test", "test signal", ModelFamily::Claude, 1.0)],
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

    #[test]
    fn in_memory_backend_round_trip() {
        let backend = InMemoryBackend::new(100);
        let key = b"test-key";
        let value = b"test-value";

        assert!(backend.get(key).unwrap().is_none());
        backend.put(key, value).unwrap();
        assert_eq!(backend.get(key).unwrap().unwrap(), value);
        assert!(backend.contains(key).unwrap());

        backend.delete(key).unwrap();
        assert!(backend.get(key).unwrap().is_none());
    }

    #[test]
    fn in_memory_backend_respects_max_entries() {
        let backend = InMemoryBackend::new(2);
        backend.put(b"a", b"1").unwrap();
        backend.put(b"b", b"2").unwrap();
        backend.put(b"c", b"3").unwrap();

        assert!(backend.get(b"a").unwrap().is_some());
        assert!(backend.get(b"b").unwrap().is_some());
        assert!(backend.get(b"c").unwrap().is_none());
    }

    #[test]
    fn in_memory_backend_allows_update_when_full() {
        let backend = InMemoryBackend::new(1);
        backend.put(b"a", b"1").unwrap();
        backend.put(b"a", b"2").unwrap();
        assert_eq!(backend.get(b"a").unwrap().unwrap(), b"2");
    }

    #[test]
    fn redb_backend_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let backend = RedbBackend::open(dir.path()).unwrap();
        let key = b"test-key";
        let value = b"test-value";

        assert!(backend.get(key).unwrap().is_none());
        backend.put(key, value).unwrap();
        assert_eq!(backend.get(key).unwrap().unwrap(), value);
        assert!(backend.contains(key).unwrap());

        backend.delete(key).unwrap();
        assert!(backend.get(key).unwrap().is_none());
    }

    #[test]
    fn tiered_backend_promotes_cold_to_hot() {
        let dir = tempfile::tempdir().unwrap();
        let cold = RedbBackend::open(dir.path()).unwrap();
        let key = b"promote-me";
        let value = b"data";

        cold.put(key, value).unwrap();

        let hot = InMemoryBackend::new(100);
        assert!(hot.get(key).unwrap().is_none());

        let tiered = TieredBackend::new(hot, cold);
        let result = tiered.get(key).unwrap().unwrap();
        assert_eq!(result, value);

        assert!(tiered.hot.get(key).unwrap().is_some());
    }

    #[test]
    fn custom_backend_via_with_backend() {
        let backend = InMemoryBackend::new(100);
        let cache = Cache::with_backend(Box::new(backend));

        let hash = [1u8; 32];
        assert!(cache.get(&hash).is_none());
    }

    #[test]
    fn namespaces_isolate_data() {
        let backend = InMemoryBackend::new(100);
        let cache = Cache::with_backend(Box::new(backend));

        use crate::report::{Attribution, ModelFamily, ReportMetadata};

        let hash = [5u8; 32];
        let report = Report {
            attribution: Attribution {
                primary: ModelFamily::Human,
                confidence: 0.5,
                scores: HashMap::from([(ModelFamily::Human, 0.5)]),
            },
            signals: vec![],
            metadata: ReportMetadata {
                file_path: None,
                lines_of_code: 1,
                signal_count: 0,
            },
            symbol_reports: None,
        };

        cache.put(&hash, &report).unwrap();
        assert!(cache.get(&hash).is_some());
        assert!(cache.get_symbols(&hash).is_none());
    }

    #[test]
    fn resolve_path_config_override_takes_priority() {
        let custom = Path::new("/tmp/my-custom-cache");
        let resolved = Cache::resolve_path(Some(custom));
        assert_eq!(resolved, PathBuf::from("/tmp/my-custom-cache"));
    }

    #[test]
    fn resolve_path_falls_back_to_default() {
        let resolved = Cache::resolve_path(None);
        assert!(resolved.ends_with("vibecheck"), "expected path ending with 'vibecheck', got: {resolved:?}");
    }

    #[test]
    fn default_path_ends_with_vibecheck() {
        let p = Cache::default_path();
        assert!(p.ends_with("vibecheck"), "expected path ending with 'vibecheck', got: {p:?}");
    }
}
