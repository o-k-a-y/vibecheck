use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct AiSignalsAnalyzer;

impl Analyzer for AiSignalsAnalyzer {
    fn name(&self) -> &str {
        "ai_signals"
    }

    fn analyze(&self, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Absence of TODO/FIXME — AI rarely leaves these
        let has_todo = lines.iter().any(|l| {
            let upper = l.to_uppercase();
            upper.contains("TODO") || upper.contains("FIXME")
        });
        if !has_todo && total_lines > 30 {
            signals.push(Signal {
                source: self.name().into(),
                description: "No TODO/FIXME markers in a substantial file".into(),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
        }

        // No dead code markers (#[allow(dead_code)], #[allow(unused)])
        let dead_code_markers = ["allow(dead_code)", "allow(unused)", "#[cfg(dead_code)]"];
        let has_dead_code = lines
            .iter()
            .any(|l| dead_code_markers.iter().any(|m| l.contains(m)));
        if !has_dead_code && total_lines > 30 {
            signals.push(Signal {
                source: self.name().into(),
                description: "No dead code suppressions".into(),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
        }

        // Suspicious consistency: all functions have doc comments
        let fn_count = lines.iter().filter(|l| {
            let trimmed = l.trim();
            (trimmed.starts_with("pub fn ") || trimmed.starts_with("fn "))
                && !trimmed.starts_with("fn main")
        }).count();

        let documented_fn_count = lines.windows(2).filter(|w| {
            let prev = w[0].trim();
            let curr = w[1].trim();
            prev.starts_with("///")
                && (curr.starts_with("pub fn ") || curr.starts_with("fn "))
        }).count();

        if fn_count >= 3 && documented_fn_count == fn_count {
            signals.push(Signal {
                source: self.name().into(),
                description: "Every function has a doc comment — suspiciously thorough".into(),
                family: ModelFamily::Claude,
                weight: 2.0,
            });
        }

        // Consistent formatting: no trailing whitespace, consistent indentation
        let trailing_ws = lines.iter().filter(|l| !l.is_empty() && l.ends_with(' ')).count();
        if trailing_ws == 0 && total_lines > 20 {
            signals.push(Signal {
                source: self.name().into(),
                description: "Zero trailing whitespace — machine-perfect formatting".into(),
                family: ModelFamily::Gpt,
                weight: 0.5,
            });
        }

        // Commented-out code is a human signal
        let commented_code = lines.iter().filter(|l| {
            let trimmed = l.trim();
            trimmed.starts_with("// let ")
                || trimmed.starts_with("// fn ")
                || trimmed.starts_with("// use ")
                || trimmed.starts_with("// println!")
                || trimmed.starts_with("// pub ")
        }).count();
        if commented_code >= 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{commented_code} lines of commented-out code"),
                family: ModelFamily::Human,
                weight: 2.5,
            });
        }

        // Placeholder strings like "lorem ipsum" or "foo/bar/baz"
        let placeholder_count = lines.iter().filter(|l| {
            let lower = l.to_lowercase();
            lower.contains("lorem ipsum")
                || lower.contains("foo")
                || lower.contains("asdf")
                || lower.contains("placeholder")
        }).count();
        if placeholder_count == 0 && total_lines > 30 {
            signals.push(Signal {
                source: self.name().into(),
                description: "No placeholder values — polished code".into(),
                family: ModelFamily::Claude,
                weight: 0.5,
            });
        }

        signals
    }
}
