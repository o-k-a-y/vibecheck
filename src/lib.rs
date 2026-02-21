pub mod analyzers;
pub mod output;
pub mod pipeline;
pub mod report;

use std::path::Path;

use pipeline::Pipeline;
use report::Report;

/// Analyze a source code string and return a report.
pub fn analyze(source: &str) -> Report {
    let pipeline = Pipeline::with_defaults();
    pipeline.run(source, None)
}

/// Analyze a file at the given path and return a report.
pub fn analyze_file(path: &Path) -> std::io::Result<Report> {
    let source = std::fs::read_to_string(path)?;
    let pipeline = Pipeline::with_defaults();
    Ok(pipeline.run(&source, Some(path.to_path_buf())))
}
