use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use walkdir::WalkDir;

use vibecheck_core::output::OutputFormat;
use vibecheck_core::report::{ModelFamily, Report};

mod output;

#[derive(Parser)]
#[command(name = "vibecheck", about = "Detect AI-generated code")]
struct Cli {
    /// File or directory to analyze.
    path: PathBuf,

    /// Output format: pretty, text, or json.
    #[arg(long, default_value = "pretty")]
    format: String,

    /// Exit with code 1 if any file is NOT attributed to one of these families.
    /// Comma-separated, e.g. --assert-family claude,gpt
    #[arg(long, value_delimiter = ',')]
    assert_family: Option<Vec<String>>,

    /// Skip the content-addressed cache (always re-analyze).
    #[arg(long)]
    no_cache: bool,
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

    let supported_exts = ["rs", "py", "js", "ts", "jsx", "tsx", "go"];
    let mut files = Vec::new();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let p = entry.path();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|e| supported_exts.contains(&e))
            .unwrap_or(false)
        {
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

fn parse_families(names: &[String]) -> Result<Vec<ModelFamily>> {
    names
        .iter()
        .map(|s| match s.to_lowercase().as_str() {
            "claude" => Ok(ModelFamily::Claude),
            "gpt" => Ok(ModelFamily::Gpt),
            "gemini" => Ok(ModelFamily::Gemini),
            "copilot" => Ok(ModelFamily::Copilot),
            "human" => Ok(ModelFamily::Human),
            other => anyhow::bail!("unknown family: {other}"),
        })
        .collect()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fmt = parse_format(&cli.format)?;
    let allowed_families = cli
        .assert_family
        .as_ref()
        .map(|f| parse_families(f))
        .transpose()?;

    let files = collect_files(&cli.path).context("failed to collect files")?;

    if files.is_empty() {
        anyhow::bail!("no supported source files found in {}", cli.path.display());
    }

    let analyze_fn: fn(&std::path::Path) -> std::io::Result<Report> = if cli.no_cache {
        vibecheck_core::analyze_file_no_cache
    } else {
        vibecheck_core::analyze_file
    };

    let reports: Vec<Report> = files
        .iter()
        .map(|f| analyze_fn(f))
        .collect::<std::io::Result<Vec<_>>>()
        .context("failed to analyze files")?;

    if fmt == OutputFormat::Json && reports.len() > 1 {
        let json = serde_json::to_string_pretty(&reports)?;
        println!("{json}");
    } else {
        for report in &reports {
            println!("{}", format_report(report, fmt));
        }
    }

    if let Some(ref allowed) = allowed_families {
        let mut failures = Vec::new();
        for report in &reports {
            // Skip files too small to produce signals — attribution is arbitrary for them.
            if report.metadata.signal_count == 0 {
                continue;
            }
            if !allowed.contains(&report.attribution.primary) {
                failures.push(report);
            }
        }
        if !failures.is_empty() {
            eprintln!("\n--- VIBECHECK FAILED ---");
            for report in &failures {
                let path = report
                    .metadata
                    .file_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<stdin>".into());
                eprintln!(
                    "  {} — detected as {} ({:.0}%), expected one of: {}",
                    path,
                    report.attribution.primary,
                    report.attribution.confidence * 100.0,
                    allowed.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", "),
                );
            }
            std::process::exit(1);
        } else {
            eprintln!("\nAll files passed the vibe check.");
        }
    }

    Ok(())
}
