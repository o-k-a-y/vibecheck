use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use walkdir::WalkDir;

use vibecheck::output::{self, OutputFormat};
use vibecheck::report::Report;

#[derive(Parser)]
#[command(name = "vibecheck", about = "Detect AI-generated code")]
struct Cli {
    /// File or directory to analyze.
    path: PathBuf,

    /// Output format: pretty, text, or json.
    #[arg(long, default_value = "pretty")]
    format: String,
}

fn parse_format(s: &str) -> Result<OutputFormat> {
    match s {
        "pretty" => Ok(OutputFormat::Pretty),
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        other => anyhow::bail!("unknown format: {other} (expected pretty, text, or json)"),
    }
}

fn collect_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let p = entry.path();
        if p.extension().map_or(false, |ext| ext == "rs") {
            files.push(p.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

fn format_report(report: &Report, fmt: OutputFormat) -> String {
    match fmt {
        OutputFormat::Json => output::format_json(report),
        OutputFormat::Text => output::format_text(report),
        OutputFormat::Pretty => output::format_pretty(report),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fmt = parse_format(&cli.format)?;

    let files = collect_files(&cli.path).context("failed to collect files")?;

    if files.is_empty() {
        anyhow::bail!("no .rs files found in {}", cli.path.display());
    }

    let reports: Vec<Report> = files
        .iter()
        .map(|f| vibecheck::analyze_file(f))
        .collect::<std::io::Result<Vec<_>>>()
        .context("failed to analyze files")?;

    if fmt == OutputFormat::Json && reports.len() > 1 {
        // Emit a JSON array for multiple files
        let json = serde_json::to_string_pretty(&reports)?;
        println!("{json}");
    } else {
        for report in &reports {
            println!("{}", format_report(report, fmt));
        }
    }

    Ok(())
}
