#![deny(dead_code)]

pub mod analyzers;
pub mod cache;
pub mod colors;
pub mod heuristics;
pub mod ignore_rules;
pub mod language;
pub mod merkle;
pub mod output;
pub mod pipeline;
pub mod report;

#[cfg(feature = "corpus")]
pub mod store;

use std::path::{Path, PathBuf};

use cache::Cache;
use heuristics::{ConfiguredHeuristics, HeuristicsProvider};
use ignore_rules::{IgnoreConfig, IgnoreRules};
use merkle::walk_and_hash_with;
use pipeline::Pipeline;
use report::Report;

/// Load heuristics from a `.vibecheck` config rooted at `dir`.
fn load_heuristics(dir: &std::path::Path) -> Box<dyn HeuristicsProvider> {
    let cfg = IgnoreConfig::load(dir);
    Box::new(ConfiguredHeuristics::from_config(cfg.heuristics_map()))
}

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
    let dir = path.parent().unwrap_or(path);
    let pipeline = Pipeline::with_heuristics(
        crate::analyzers::default_analyzers(),
        crate::analyzers::default_cst_analyzers(),
        load_heuristics(dir),
    );
    let report = pipeline.run(&source, Some(path.to_path_buf()));

    if let Some(ref c) = cache {
        let _ = c.put(&hash, &report);
    }

    Ok(report)
}

/// Analyze a file without consulting or updating the cache.
pub fn analyze_file_no_cache(path: &Path) -> std::io::Result<Report> {
    let source = std::fs::read_to_string(path)?;
    let dir = path.parent().unwrap_or(path);
    let pipeline = Pipeline::with_heuristics(
        crate::analyzers::default_analyzers(),
        crate::analyzers::default_cst_analyzers(),
        load_heuristics(dir),
    );
    Ok(pipeline.run(&source, Some(path.to_path_buf())))
}

/// Analyze every supported source file under `dir`, using a Merkle hash tree
/// to skip unchanged subtrees when `use_cache` is `true`.
///
/// Ignore rules are loaded automatically from the nearest `.vibecheck` config
/// file (walking upward to the git root) and from `.gitignore`.
///
/// Returns `(file_path, Report)` pairs for all files that were (re-)analyzed.
/// Files whose content hash has not changed since the last run are returned
/// from the flat file cache without re-running the pipeline.
///
/// To supply custom ignore rules (e.g. in tests), use [`analyze_directory_with`].
pub fn analyze_directory(
    dir: &Path,
    use_cache: bool,
) -> anyhow::Result<Vec<(PathBuf, Report)>> {
    let ignore = IgnoreConfig::load(dir);
    analyze_directory_with(dir, use_cache, &ignore)
}

/// Like [`analyze_directory`], but accepts any [`IgnoreRules`] implementation.
///
/// This is the primary extension point for dependency injection: pass
/// [`ignore_rules::AllowAll`] to disable all filtering,
/// [`ignore_rules::PatternIgnore`] for substring matching in tests, or any
/// other [`IgnoreRules`] implementation.
///
/// Heuristics are loaded automatically from the nearest `.vibecheck` config
/// file relative to `dir`.
pub fn analyze_directory_with(
    dir: &Path,
    use_cache: bool,
    ignore: &dyn IgnoreRules,
) -> anyhow::Result<Vec<(PathBuf, Report)>> {
    let supported_exts = ["rs", "py", "js", "ts", "jsx", "tsx", "go"];
    let cache = if use_cache {
        Cache::open(&Cache::default_path()).ok()
    } else {
        None
    };

    // Build the Merkle tree for the directory, honouring ignore rules so that
    // ignored files do not contribute to the hash (and thus do not trigger
    // unnecessary re-analysis when they change).
    let current_node = walk_and_hash_with(dir, ignore)?;

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
        collect_cached_reports(dir, &supported_exts, cache.as_ref(), &mut results, ignore);
    } else {
        // Walk and analyze, relying on the per-file cache to avoid re-parsing
        // individual unchanged files (analyze_file handles per-file caching).
        walk_and_analyze(dir, &supported_exts, &mut results, ignore)?;

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
    ignore: &dyn IgnoreRules,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            if ignore.is_ignored_dir(&path) {
                continue;
            }
            collect_cached_reports(&path, supported_exts, cache, results, ignore);
        } else if path.is_file() {
            if ignore.is_ignored(&path) {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !supported_exts.contains(&ext) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&path) {
                let hash = Cache::hash_content(&bytes);
                let cached = cache.and_then(|c| c.get(&hash));
                if let Some(mut report) = cached {
                    report.metadata.file_path = Some(path.clone());
                    results.push((path, report));
                } else if let Ok(report) = analyze_file(&path) {
                    results.push((path, report));
                }
            }
        }
    }
}

fn walk_and_analyze(
    dir: &Path,
    supported_exts: &[&str],
    results: &mut Vec<(PathBuf, Report)>,
    ignore: &dyn IgnoreRules,
) -> anyhow::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            if ignore.is_ignored_dir(&path) {
                continue;
            }
            walk_and_analyze(&path, supported_exts, results, ignore)?;
        } else if path.is_file() {
            if ignore.is_ignored(&path) {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ignore_rules::{AllowAll, PatternIgnore};
    use std::io::Write;

    fn sample_rust_source(n_lines: usize) -> String {
        (0..n_lines).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn analyze_string_returns_report() {
        let source = sample_rust_source(40);
        let report = analyze(&source);
        assert!(report.metadata.lines_of_code > 0);
        assert!(report.metadata.signal_count > 0 || report.signals.is_empty()); // either is fine
    }

    #[test]
    fn analyze_file_no_cache_works() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "{}", sample_rust_source(40)).unwrap();
        let path = f.path().to_path_buf();
        let report = analyze_file_no_cache(&path).unwrap();
        assert!(report.metadata.lines_of_code > 0);
        assert_eq!(report.metadata.file_path, Some(path));
    }

    #[test]
    fn analyze_file_works() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "{}", sample_rust_source(40)).unwrap();
        let report = analyze_file(f.path()).unwrap();
        assert!(report.metadata.lines_of_code > 0);
    }

    #[test]
    fn analyze_directory_with_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let results = analyze_directory_with(dir.path(), false, &AllowAll).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn analyze_directory_with_rust_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, sample_rust_source(40)).unwrap();
        let results = analyze_directory_with(dir.path(), false, &AllowAll).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, path);
    }

    #[test]
    fn analyze_directory_ignores_non_source_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# hello").unwrap();
        let results = analyze_directory_with(dir.path(), false, &AllowAll).unwrap();
        assert!(results.is_empty(), "markdown files should not be analyzed");
    }

    #[test]
    fn analyze_directory_recurses_into_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("lib.py"), sample_rust_source(40)).unwrap();
        let results = analyze_directory_with(dir.path(), false, &AllowAll).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn analyze_file_symbols_no_cache_works() {
        let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(f, "fn hello() {{}}\nfn world() {{}}\n{}", sample_rust_source(40)).unwrap();
        let report = analyze_file_symbols_no_cache(f.path()).unwrap();
        assert!(report.metadata.lines_of_code > 0);
    }

    #[test]
    fn analyze_file_cache_hit_returns_consistent_result() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "{}", sample_rust_source(40)).unwrap();
        let path = f.path().to_path_buf();
        let r1 = analyze_file(&path).unwrap();
        // Second call — same content hash, should serve from cache.
        let r2 = analyze_file(&path).unwrap();
        assert_eq!(r1.attribution.primary, r2.attribution.primary);
        assert_eq!(r2.metadata.file_path, Some(path));
    }

    #[test]
    fn analyze_file_symbols_works() {
        let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(f, "fn hello() {{}}\nfn world() {{}}\n{}", sample_rust_source(40)).unwrap();
        let report = analyze_file_symbols(f.path()).unwrap();
        assert!(report.symbol_reports.is_some());
        assert!(report.metadata.lines_of_code > 0);
    }

    #[test]
    fn analyze_file_symbols_cache_hit_returns_consistent_result() {
        let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(f, "fn hello() {{}}\nfn world() {{}}\n{}", sample_rust_source(40)).unwrap();
        let path = f.path().to_path_buf();
        let r1 = analyze_file_symbols(&path).unwrap();
        // Second call — both base report and symbol list should be cached.
        let r2 = analyze_file_symbols(&path).unwrap();
        assert_eq!(
            r1.symbol_reports.as_ref().map(|s| s.len()),
            r2.symbol_reports.as_ref().map(|s| s.len()),
        );
    }

    #[test]
    fn analyze_directory_public_wrapper_finds_rust_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), sample_rust_source(40)).unwrap();
        let results = analyze_directory(dir.path(), false).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn analyze_directory_with_cache_second_run_uses_dir_cache() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), sample_rust_source(40)).unwrap();
        // First run populates the dir cache.
        let r1 = analyze_directory_with(dir.path(), true, &AllowAll).unwrap();
        // Second run — dir hash unchanged, exercises collect_cached_reports.
        let r2 = analyze_directory_with(dir.path(), true, &AllowAll).unwrap();
        assert_eq!(r1.len(), r2.len());
    }

    #[test]
    fn analyze_directory_with_cache_ignores_pattern_matched_dir() {
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        std::fs::create_dir(&vendor).unwrap();
        std::fs::write(vendor.join("lib.rs"), sample_rust_source(40)).unwrap();
        // First run: populates dir cache (with the file absent due to ignore).
        let r1 = analyze_directory_with(dir.path(), true, &PatternIgnore(vec!["vendor".into()])).unwrap();
        // Second run: dir hash unchanged, collect_cached_reports also respects ignore.
        let r2 = analyze_directory_with(dir.path(), true, &PatternIgnore(vec!["vendor".into()])).unwrap();
        assert!(r1.is_empty());
        assert!(r2.is_empty());
    }

    #[test]
    fn analyze_directory_with_ignores_pattern_matched_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("generated.rs"), sample_rust_source(40)).unwrap();
        let results =
            analyze_directory_with(dir.path(), false, &PatternIgnore(vec!["generated".into()])).unwrap();
        assert!(results.is_empty());
    }
}
