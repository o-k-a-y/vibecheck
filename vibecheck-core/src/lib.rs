#![deny(dead_code)]

pub mod analyzers;
pub mod cache;
pub mod colors;
pub mod language;
pub mod merkle;
pub mod output;
pub mod pipeline;
pub mod report;

#[cfg(feature = "corpus")]
pub mod store;

use std::path::{Path, PathBuf};

use cache::Cache;
use merkle::walk_and_hash;
use pipeline::Pipeline;
use report::Report;

/// Analyze a source code string and return a report.
pub fn analyze(source: &str) -> Report {
    let pipeline = Pipeline::with_defaults();
    pipeline.run(source, None)
}

/// Analyze a file, using the content-addressed cache to skip re-analysis of unchanged files.
pub fn analyze_file(path: &Path) -> std::io::Result<Report> {
    let bytes = std::fs::read(path)?;
    let hash = Cache::hash_content(&bytes);

    let cache = Cache::open(&Cache::default_path()).ok();

    if let Some(ref c) = cache {
        if let Some(mut cached) = c.get(&hash) {
            // Always use the caller's path, not the path stored when the cache entry was written.
            cached.metadata.file_path = Some(path.to_path_buf());
            return Ok(cached);
        }
    }

    let source = String::from_utf8(bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let pipeline = Pipeline::with_defaults();
    let report = pipeline.run(&source, Some(path.to_path_buf()));

    if let Some(ref c) = cache {
        let _ = c.put(&hash, &report);
    }

    Ok(report)
}

/// Analyze a file without consulting or updating the cache.
pub fn analyze_file_no_cache(path: &Path) -> std::io::Result<Report> {
    let source = std::fs::read_to_string(path)?;
    let pipeline = Pipeline::with_defaults();
    Ok(pipeline.run(&source, Some(path.to_path_buf())))
}

/// Analyze every supported source file under `dir`, using a Merkle hash tree
/// to skip unchanged subtrees when `use_cache` is `true`.
///
/// Returns `(file_path, Report)` pairs for all files that were (re-)analyzed.
/// Files whose content hash has not changed since the last run are returned
/// from the flat file cache without re-running the pipeline.
pub fn analyze_directory(
    dir: &Path,
    use_cache: bool,
) -> anyhow::Result<Vec<(PathBuf, Report)>> {
    let supported_exts = ["rs", "py", "js", "ts", "jsx", "tsx", "go"];
    let cache = if use_cache {
        Cache::open(&Cache::default_path()).ok()
    } else {
        None
    };

    // Build the Merkle tree for the directory.
    let current_node = walk_and_hash(dir)?;

    // If the directory hash matches the cached hash, every file is unchanged.
    let unchanged = if use_cache {
        cache
            .as_ref()
            .and_then(|c| c.get_dir(dir))
            .map(|cached| cached.hash == current_node.hash)
            .unwrap_or(false)
    } else {
        false
    };

    let mut results = Vec::new();

    if unchanged {
        // Collect reports from the file cache — no pipeline work needed.
        collect_cached_reports(dir, &supported_exts, cache.as_ref(), &mut results);
    } else {
        // Walk and analyze, relying on the per-file cache to avoid re-parsing
        // individual unchanged files.
        walk_and_analyze(dir, &supported_exts, cache.as_ref(), &mut results)?;

        // Persist the updated directory node.
        if let Some(ref c) = cache {
            let _ = c.set_dir(dir, &current_node);
        }
    }

    Ok(results)
}

fn collect_cached_reports(
    dir: &Path,
    supported_exts: &[&str],
    cache: Option<&Cache>,
    results: &mut Vec<(PathBuf, Report)>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            collect_cached_reports(&path, supported_exts, cache, results);
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !supported_exts.contains(&ext) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&path) {
                let hash = Cache::hash_content(&bytes);
                if let Some(ref c) = cache {
                    if let Some(mut report) = c.get(&hash) {
                        report.metadata.file_path = Some(path.clone());
                        results.push((path, report));
                    }
                }
            }
        }
    }
}

fn walk_and_analyze(
    dir: &Path,
    supported_exts: &[&str],
    cache: Option<&Cache>,
    results: &mut Vec<(PathBuf, Report)>,
) -> anyhow::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            walk_and_analyze(&path, supported_exts, cache, results)?;
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !supported_exts.contains(&ext) {
                continue;
            }
            let report = analyze_file(&path)
                .map_err(|e| anyhow::anyhow!("failed to analyze {}: {}", path.display(), e))?;
            results.push((path, report));
        }
    }
    Ok(())
}

/// Analyze a source file and return a `Report` with `symbol_reports` populated.
///
/// Both the base report and the symbol list are served from the
/// content-addressed cache when available, and written back on a miss.
pub fn analyze_file_symbols(file_path: &Path) -> anyhow::Result<Report> {
    let bytes = std::fs::read(file_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", file_path.display(), e))?;
    let hash = Cache::hash_content(&bytes);
    let cache = Cache::open(&Cache::default_path()).ok();

    // Fast path: both layers cached.
    if let Some(ref c) = cache {
        if let (Some(mut base), Some(syms)) = (c.get(&hash), c.get_symbols(&hash)) {
            base.metadata.file_path = Some(file_path.to_path_buf());
            base.symbol_reports = Some(syms);
            return Ok(base);
        }
    }

    let source_str = std::str::from_utf8(&bytes)
        .map_err(|e| anyhow::anyhow!("non-UTF-8 file: {e}"))?;
    let pipeline = Pipeline::with_defaults();
    let mut report = pipeline.run(source_str, Some(file_path.to_path_buf()));
    let symbol_reports = pipeline.run_symbols(&bytes, file_path)?;
    report.symbol_reports = Some(symbol_reports.clone());

    if let Some(ref c) = cache {
        let _ = c.put(&hash, &report);
        let _ = c.put_symbols(&hash, &symbol_reports);
    }

    Ok(report)
}

/// Analyze a source file at symbol level, bypassing the cache entirely.
pub fn analyze_file_symbols_no_cache(file_path: &Path) -> anyhow::Result<Report> {
    let bytes = std::fs::read(file_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", file_path.display(), e))?;
    let source_str = std::str::from_utf8(&bytes)
        .map_err(|e| anyhow::anyhow!("non-UTF-8 file: {e}"))?;
    let pipeline = Pipeline::with_defaults();
    let mut report = pipeline.run(source_str, Some(file_path.to_path_buf()));
    let symbol_reports = pipeline.run_symbols(&bytes, file_path)?;
    report.symbol_reports = Some(symbol_reports);
    Ok(report)
}
