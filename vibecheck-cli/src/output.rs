use colored::Colorize;
use vibecheck::report::{ModelFamily, Report};

/// Format a report with terminal colors.
pub fn format_pretty(report: &Report) -> String {
    let mut out = String::new();

    if let Some(ref path) = report.metadata.file_path {
        out.push_str(&format!("{} {}\n", "File:".bold(), path.display()));
    }

    let verdict_color = match report.attribution.primary {
        ModelFamily::Claude => "magenta",
        ModelFamily::Gpt => "green",
        ModelFamily::Gemini => "blue",
        ModelFamily::Copilot => "cyan",
        ModelFamily::Human => "yellow",
    };
    let verdict_str = format!(
        "{} ({:.0}% confidence)",
        report.attribution.primary,
        report.attribution.confidence * 100.0
    );
    out.push_str(&format!(
        "{} {}\n",
        "Verdict:".bold(),
        verdict_str.color(verdict_color).bold()
    ));
    out.push_str(&format!(
        "{} {} | {} {}\n",
        "Lines:".dimmed(),
        report.metadata.lines_of_code,
        "Signals:".dimmed(),
        report.metadata.signal_count,
    ));

    out.push_str(&format!("\n{}\n", "Scores:".bold()));
    let mut sorted_scores: Vec<_> = report.attribution.scores.iter().collect();
    sorted_scores.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (family, score) in &sorted_scores {
        let bar_len = (*score * 30.0) as usize;
        let bar = "█".repeat(bar_len);
        let family_str = format!("{:<10}", family.to_string());
        let bar_color = match family {
            ModelFamily::Claude => "magenta",
            ModelFamily::Gpt => "green",
            ModelFamily::Gemini => "blue",
            ModelFamily::Copilot => "cyan",
            ModelFamily::Human => "yellow",
        };
        out.push_str(&format!(
            "  {} {} {:.1}%\n",
            family_str,
            bar.color(bar_color),
            *score * 100.0
        ));
    }

    if !report.signals.is_empty() {
        out.push_str(&format!("\n{}\n", "Signals:".bold()));
        for signal in &report.signals {
            let sign = if signal.weight >= 0.0 { "+" } else { "" };
            let weight_str = format!("{}{:.1}", sign, signal.weight);
            let colored_weight = if signal.weight >= 0.0 {
                weight_str.green()
            } else {
                weight_str.red()
            };
            out.push_str(&format!(
                "  {} {} {} — {}\n",
                format!("[{}]", signal.source).dimmed(),
                colored_weight,
                signal.family.to_string().bold(),
                signal.description,
            ));
        }
    }

    out
}

pub use vibecheck::output::{format_json, format_text};
