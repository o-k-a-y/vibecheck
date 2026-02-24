use std::path::PathBuf;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use vibecheck_core::output::OutputFormat;
use vibecheck_core::report::{ModelFamily, Report};

use crate::output;

pub fn collect_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
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

pub fn parse_format(s: &str) -> Result<OutputFormat> {
    match s {
        "pretty" => Ok(OutputFormat::Pretty),
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        other => anyhow::bail!("unknown format: {other} (expected pretty, text, or json)"),
    }
}

pub fn parse_families(names: &[String]) -> Result<Vec<ModelFamily>> {
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

pub fn format_report(report: &Report, fmt: OutputFormat) -> String {
    match fmt {
        OutputFormat::Json => output::format_json(report),
        OutputFormat::Text => output::format_text(report),
        OutputFormat::Pretty => output::format_pretty(report),
    }
}

pub fn run(
    path: &PathBuf,
    format: &str,
    no_cache: bool,
    symbols: bool,
    assert_family: Option<Vec<String>>,
) -> Result<()> {
    let fmt = parse_format(format)?;
    let allowed_families = assert_family
        .as_ref()
        .map(|f| parse_families(f))
        .transpose()?;

    let files = collect_files(path).context("failed to collect files")?;

    if files.is_empty() {
        anyhow::bail!("no supported source files found in {}", path.display());
    }

    let reports: Vec<Report> = if symbols {
        let symbol_fn: fn(&std::path::Path) -> anyhow::Result<Report> = if no_cache {
            vibecheck_core::analyze_file_symbols_no_cache
        } else {
            vibecheck_core::analyze_file_symbols
        };
        files
            .iter()
            .map(|f| symbol_fn(f).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
            .collect::<std::io::Result<Vec<_>>>()
            .context("failed to analyze files")?
    } else {
        let analyze_fn: fn(&std::path::Path) -> std::io::Result<Report> = if no_cache {
            vibecheck_core::analyze_file_no_cache
        } else {
            vibecheck_core::analyze_file
        };
        files
            .iter()
            .map(|f| analyze_fn(f))
            .collect::<std::io::Result<Vec<_>>>()
            .context("failed to analyze files")?
    };

    if fmt == OutputFormat::Json && reports.len() > 1 {
        let json = serde_json::to_string_pretty(&reports)?;
        println!("{json}");
    } else if symbols {
        for report in &reports {
            println!("{}", format_report(report, fmt));
            if let Some(ref sym_reports) = report.symbol_reports {
                if !sym_reports.is_empty() {
                    println!("  Symbol-level attribution:");
                    for sr in sym_reports {
                        println!(
                            "    {:>4}–{:<4}  {:<40}  {} ({:.0}%)",
                            sr.metadata.start_line,
                            sr.metadata.end_line,
                            format!("{}  [{}]", sr.metadata.name, sr.metadata.kind),
                            sr.attribution.primary,
                            sr.attribution.confidence * 100.0,
                        );
                    }
                }
            }
        }
    } else {
        for report in &reports {
            println!("{}", format_report(report, fmt));
        }
    }

    if let Some(ref allowed) = allowed_families {
        let mut failures = Vec::new();
        for report in &reports {
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
