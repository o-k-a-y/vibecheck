use std::path::PathBuf;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use vibecheck_core::ignore_rules::{IgnoreConfig, IgnoreRules};
use vibecheck_core::output::OutputFormat;
use vibecheck_core::report::{ModelFamily, Report};

use crate::output;

/// Collect all supported source files under `path`, respecting `ignore`.
///
/// When `path` is a single file it is returned directly (no filtering
/// applied).  When it is a directory the tree is walked, skipping any entry
/// for which `ignore` returns `true`.
pub fn collect_files(path: &PathBuf, ignore: &dyn IgnoreRules) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }

    let supported_exts = ["rs", "py", "js", "ts", "jsx", "tsx", "go"];
    let mut files = Vec::new();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| !ignore.is_ignored_dir(e.path()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| !ignore.is_ignored(e.path()))
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
        OutputFormat::Pretty => output::format_pretty(report, &vibecheck_core::colors::DefaultTheme),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibecheck_core::ignore_rules::PatternIgnore;

    #[test]
    fn parse_format_pretty() {
        assert_eq!(parse_format("pretty").unwrap(), OutputFormat::Pretty);
    }

    #[test]
    fn parse_format_text() {
        assert_eq!(parse_format("text").unwrap(), OutputFormat::Text);
    }

    #[test]
    fn parse_format_json() {
        assert_eq!(parse_format("json").unwrap(), OutputFormat::Json);
    }

    #[test]
    fn parse_format_unknown_is_error() {
        assert!(parse_format("csv").is_err());
    }

    #[test]
    fn parse_families_known() {
        let input = vec!["claude".into(), "gpt".into(), "human".into()];
        let result = parse_families(&input).unwrap();
        assert_eq!(result, vec![ModelFamily::Claude, ModelFamily::Gpt, ModelFamily::Human]);
    }

    #[test]
    fn parse_families_case_insensitive() {
        let input = vec!["Claude".into(), "GPT".into()];
        let result = parse_families(&input).unwrap();
        assert_eq!(result, vec![ModelFamily::Claude, ModelFamily::Gpt]);
    }

    #[test]
    fn parse_families_unknown_is_error() {
        let input = vec!["deepseek".into()];
        assert!(parse_families(&input).is_err());
    }

    #[test]
    fn collect_files_single_file() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../vibecheck-core/tests/fixtures/lru_cache/claude.rs");
        let ignore = PatternIgnore(vec![]);
        let files = collect_files(&fixture, &ignore).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("claude.rs"));
    }

    #[test]
    fn collect_files_filters_by_extension() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../vibecheck-core/tests/fixtures/lru_cache");
        let ignore = PatternIgnore(vec![]);
        let files = collect_files(&fixture_dir, &ignore).unwrap();
        assert!(files.len() >= 20, "should find all fixture files; got {}", files.len());
        for f in &files {
            let ext = f.extension().unwrap().to_str().unwrap();
            assert!(
                ["rs", "py", "js", "go"].contains(&ext),
                "unexpected extension: {ext}"
            );
        }
    }

    #[test]
    fn collect_files_respects_ignore() {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../vibecheck-core/tests/fixtures/lru_cache");
        let ignore = PatternIgnore(vec!["claude".into()]);
        let files = collect_files(&fixture_dir, &ignore).unwrap();
        for f in &files {
            assert!(
                !f.to_string_lossy().contains("claude"),
                "should have been ignored: {}",
                f.display()
            );
        }
    }

    #[test]
    fn format_report_text_contains_verdict() {
        let report = vibecheck_core::analyze("fn main() { println!(\"hello\"); }");
        let output = format_report(&report, OutputFormat::Text);
        assert!(output.contains("Verdict:"), "text output should have Verdict");
    }

    #[test]
    fn format_report_json_is_valid() {
        let report = vibecheck_core::analyze("fn main() {}");
        let output = format_report(&report, OutputFormat::Json);
        let _: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
    }

    #[test]
    fn format_report_pretty_contains_verdict() {
        let report = vibecheck_core::analyze("fn main() { println!(\"hello\"); }");
        let output = format_report(&report, OutputFormat::Pretty);
        assert!(output.contains("Verdict:"), "pretty output should have Verdict");
    }
}

pub fn run(
    path: &PathBuf,
    format: &str,
    no_cache: bool,
    symbols: bool,
    assert_family: Option<Vec<String>>,
    ignore_file: Option<&PathBuf>,
) -> Result<()> {
    let fmt = parse_format(format)?;
    let allowed_families = assert_family
        .as_ref()
        .map(|f| parse_families(f))
        .transpose()?;

    let ignore: Box<dyn IgnoreRules> = match ignore_file {
        Some(f) => Box::new(IgnoreConfig::from_file(f)?),
        None => Box::new(IgnoreConfig::load(path)),
    };

    let files = collect_files(path, ignore.as_ref()).context("failed to collect files")?;

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
            .map(|f| symbol_fn(f).map_err(|e| std::io::Error::other(e.to_string())))
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
