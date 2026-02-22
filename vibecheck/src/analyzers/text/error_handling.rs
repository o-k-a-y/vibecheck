use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct ErrorHandlingAnalyzer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::Analyzer;
    use crate::report::ModelFamily;

    fn run(source: &str) -> Vec<Signal> {
        ErrorHandlingAnalyzer.analyze(source)
    }

    fn pad(base: &str, total: usize) -> String {
        let mut lines: Vec<String> = base.lines().map(|l| l.to_string()).collect();
        while lines.len() < total {
            lines.push("let padding = 0;".to_string());
        }
        lines.join("\n")
    }

    #[test]
    fn short_source_no_signals() {
        let source = (0..5).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
        assert!(run(&source).is_empty());
    }

    #[test]
    fn zero_unwrap_in_large_file_is_claude() {
        let source = pad("fn process() -> Result<(), String> { Ok(()) }", 35);
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected zero-unwrap Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn five_unwraps_is_human() {
        let lines: Vec<String> = (0..5)
            .map(|_| "let v = opt.unwrap();".to_string())
            .chain((0..30).map(|i| format!("let x{i} = {i};")))
            .collect();
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 1.5),
            "expected 5+ unwraps Human signal (weight 1.5)"
        );
    }

    #[test]
    fn one_to_three_unwraps_is_copilot() {
        let lines: Vec<String> = vec![
            "let v = opt.unwrap();".to_string(),
            "let w = other.unwrap();".to_string(),
        ]
        .into_iter()
        .chain((0..10).map(|i| format!("let x{i} = {i};")))
        .collect();
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Copilot && s.weight == 0.5),
            "expected 1-3 unwraps Copilot signal (weight 0.5)"
        );
    }

    #[test]
    fn two_expect_calls_is_claude() {
        let lines: Vec<String> = vec![
            r#"let v = file.expect("file missing");"#.to_string(),
            r#"let w = conn.expect("conn failed");"#.to_string(),
        ]
        .into_iter()
        .chain((0..10).map(|i| format!("let x{i} = {i};")))
        .collect();
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.0
                && s.description.contains("expect")),
            "expected .expect() Claude signal (weight 1.0)"
        );
    }

    #[test]
    fn three_question_marks_is_claude() {
        let lines: Vec<String> = vec![
            "let a = foo()?;".to_string(),
            "let b = bar()?;".to_string(),
            "let c = baz()?;".to_string(),
        ]
        .into_iter()
        .chain((0..10).map(|i| format!("let x{i} = {i};")))
        .collect();
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.0
                && s.description.contains("?")),
            "expected ? operator Claude signal (weight 1.0)"
        );
    }

    #[test]
    fn two_panics_is_human() {
        let lines: Vec<String> = vec![
            r#"panic!("something went wrong");"#.to_string(),
            r#"panic!("unreachable state");"#.to_string(),
        ]
        .into_iter()
        .chain((0..10).map(|i| format!("let x{i} = {i};")))
        .collect();
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 1.5
                && s.description.contains("panic")),
            "expected panic!() Human signal (weight 1.5)"
        );
    }
}

impl Analyzer for ErrorHandlingAnalyzer {
    fn name(&self) -> &str {
        "errors"
    }

    fn analyze(&self, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Count .unwrap() calls
        let unwrap_count = lines
            .iter()
            .filter(|l| l.contains(".unwrap()"))
            .count();

        if unwrap_count == 0 && total_lines > 30 {
            signals.push(Signal {
                source: self.name().into(),
                description: "Zero .unwrap() calls — careful error handling".into(),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
        } else if unwrap_count >= 5 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{unwrap_count} .unwrap() calls — pragmatic/quick style"),
                family: ModelFamily::Human,
                weight: 1.5,
            });
        } else if unwrap_count >= 1 && unwrap_count <= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{unwrap_count} .unwrap() calls — moderate"),
                family: ModelFamily::Copilot,
                weight: 0.5,
            });
        }

        // .expect() usage — Claude and GPT prefer this over unwrap
        let expect_count = lines
            .iter()
            .filter(|l| l.contains(".expect("))
            .count();
        if expect_count >= 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{expect_count} .expect() calls — descriptive error handling"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // ? operator usage — idiomatic Rust error propagation
        let question_mark_count = lines
            .iter()
            .filter(|l| l.contains('?') && !l.trim_start().starts_with("//"))
            .count();
        if question_mark_count >= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{question_mark_count} uses of ? operator — idiomatic error propagation"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // Exhaustive match patterns (match with _ => arm at the end)
        let match_count = lines.iter().filter(|l| l.trim().starts_with("match ") || l.trim().ends_with("match {")).count();
        let wildcard_arm = lines.iter().filter(|l| {
            let trimmed = l.trim();
            trimmed.starts_with("_ =>") || trimmed.starts_with("_ =")
        }).count();

        if match_count >= 2 && wildcard_arm <= match_count / 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: "Match expressions prefer exhaustive patterns over wildcards".into(),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // panic!() usage — typically a human shortcut
        let panic_count = lines
            .iter()
            .filter(|l| l.contains("panic!(") && !l.trim_start().starts_with("//"))
            .count();
        if panic_count >= 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{panic_count} panic!() calls"),
                family: ModelFamily::Human,
                weight: 1.5,
            });
        }

        signals
    }
}
