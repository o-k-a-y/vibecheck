pub mod analyzers;
pub mod cache;
pub mod language;
pub mod output;
pub mod pipeline;
pub mod report;

#[cfg(feature = "corpus")]
pub mod store;

use std::path::Path;

use cache::Cache;
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
