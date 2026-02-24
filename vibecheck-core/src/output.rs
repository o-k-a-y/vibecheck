use crate::report::Report;

/// Output format for CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pretty,
    Text,
    Json,
}

/// Format a report as JSON.
pub fn format_json(report: &Report) -> String {
    serde_json::to_string_pretty(report).expect("report should be serializable")
}

/// Format a report as plain text (no colors).
pub fn format_text(report: &Report) -> String {
    let mut out = String::new();

    if let Some(ref path) = report.metadata.file_path {
        out.push_str(&format!("File: {}\n", path.display()));
    }
    out.push_str(&format!(
        "Verdict: {} ({:.0}% confidence)\n",
        report.attribution.primary,
        report.attribution.confidence * 100.0
    ));
    out.push_str(&format!(
        "Lines: {} | Signals: {}\n",
        report.metadata.lines_of_code, report.metadata.signal_count
    ));

    out.push_str("\nScores:\n");
    let mut sorted_scores: Vec<_> = report.attribution.scores.iter().collect();
    sorted_scores.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (family, score) in &sorted_scores {
        out.push_str(&format!("  {:<10} {:.1}%\n", family.to_string(), *score * 100.0));
    }

    if !report.signals.is_empty() {
        out.push_str("\nSignals:\n");
        for signal in &report.signals {
            let sign = if signal.weight >= 0.0 { "+" } else { "" };
            out.push_str(&format!(
                "  [{:<10}] {}{:.1} {} — {}\n",
                signal.source, sign, signal.weight, signal.family, signal.description
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Attribution, ModelFamily, ReportMetadata, Signal};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_report(with_path: bool, with_signals: bool) -> Report {
        let mut scores = HashMap::new();
        scores.insert(ModelFamily::Claude, 0.8);
        scores.insert(ModelFamily::Human, 0.2);
        let signals = if with_signals {
            vec![Signal::new("rust.errors.zero_unwrap", "errors", "No .unwrap() calls", ModelFamily::Claude, 1.5)]
        } else {
            vec![]
        };
        Report {
            attribution: Attribution {
                primary: ModelFamily::Claude,
                confidence: 0.8,
                scores,
            },
            signals,
            metadata: ReportMetadata {
                file_path: if with_path { Some(PathBuf::from("src/main.rs")) } else { None },
                lines_of_code: 42,
                signal_count: if with_signals { 1 } else { 0 },
            },
            symbol_reports: None,
        }
    }

    #[test]
    fn format_text_contains_verdict() {
        let report = make_report(false, false);
        let out = format_text(&report);
        assert!(out.contains("Verdict: Claude"));
        assert!(out.contains("80%"));
    }

    #[test]
    fn format_text_with_file_path() {
        let report = make_report(true, false);
        let out = format_text(&report);
        assert!(out.contains("File: src/main.rs"));
    }

    #[test]
    fn format_text_with_signals() {
        let report = make_report(false, true);
        let out = format_text(&report);
        assert!(out.contains("Signals:"));
        assert!(out.contains("No .unwrap() calls"));
        assert!(out.contains("+1.5"));
    }

    #[test]
    fn format_json_is_valid() {
        let report = make_report(false, true);
        let json = format_json(&report);
        assert!(json.contains("\"primary\""));
        assert!(json.contains("claude"));
    }

    #[test]
    fn output_format_eq() {
        assert_eq!(OutputFormat::Pretty, OutputFormat::Pretty);
        assert_ne!(OutputFormat::Json, OutputFormat::Text);
    }
}
