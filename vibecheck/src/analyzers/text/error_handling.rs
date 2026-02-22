use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct ErrorHandlingAnalyzer;

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
