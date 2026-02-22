use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct AiSignalsAnalyzer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::Analyzer;
    use crate::report::ModelFamily;

    fn run(source: &str) -> Vec<Signal> {
        AiSignalsAnalyzer.analyze(source)
    }

    #[test]
    fn short_source_no_signals() {
        let source = (0..5).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
        assert!(run(&source).is_empty());
    }

    #[test]
    fn no_todo_in_large_file_is_claude() {
        // 35 lines, no TODO/FIXME → Claude signal weight 1.5
        let source = (0..35).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected no-TODO Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn todo_present_suppresses_no_todo_signal() {
        let mut lines: Vec<String> = (0..35).map(|i| format!("let x{i} = {i};")).collect();
        lines.push("// TODO: fix this later".to_string());
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            !signals.iter().any(|s| s.description.contains("TODO/FIXME") && s.weight == 1.5),
            "should not emit no-TODO signal when TODO is present"
        );
    }

    #[test]
    fn commented_out_code_is_human() {
        // 2+ commented-out code lines → Human signal weight 2.5
        let mut lines: Vec<&str> = vec![
            "// let old_value = compute();",
            "// let result = old_value * 2;",
        ];
        // Pad to 10+ lines so the guard passes
        for _ in 0..10 { lines.push("let x = 1;"); }
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 2.5),
            "expected commented-out code Human signal (weight 2.5)"
        );
    }

    #[test]
    fn all_functions_documented_is_claude() {
        let source = "\
// padding\n// padding\n// padding\n// padding\n// padding\n\
/// Does thing one.\npub fn thing_one() {}\n\
/// Does thing two.\npub fn thing_two() {}\n\
/// Does thing three.\npub fn thing_three() {}";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 2.0),
            "expected all-documented-functions Claude signal (weight 2.0)"
        );
    }
}

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
