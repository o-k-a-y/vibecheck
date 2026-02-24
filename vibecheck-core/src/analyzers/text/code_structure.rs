use crate::analyzers::Analyzer;
use crate::heuristics::signal_ids;
use crate::report::{ModelFamily, Signal};

pub struct CodeStructureAnalyzer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::Analyzer;
    use crate::report::ModelFamily;

    fn run(source: &str) -> Vec<Signal> {
        CodeStructureAnalyzer.analyze(source)
    }

    #[test]
    fn sorted_imports_is_claude() {
        let source = "\
use std::collections::HashMap;\n\
use std::fmt;\n\
use std::path::PathBuf;\n\
let x = 1;\nlet y = 2;\nlet z = 3;\nlet a = 4;\nlet b = 5;\nlet c = 6;\nlet d = 7;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.0
                && s.description.contains("sorted")),
            "expected sorted imports Claude signal (weight 1.0)"
        );
    }

    #[test]
    fn high_annotation_ratio_is_gpt() {
        // 8 annotated out of 10 total = 80% > 70%
        let source = "\
let x: i32 = 1;\n\
let y: String = String::new();\n\
let z: Vec<u8> = vec![];\n\
let w: bool = true;\n\
let a: u64 = 0;\n\
let b: f64 = 0.0;\n\
let c: usize = 0;\n\
let d: i64 = 0;\n\
let v = 0;\n\
let u = 0;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Gpt && s.weight == 1.0),
            "expected high annotation ratio Gpt signal (weight 1.0)"
        );
    }

    #[test]
    fn low_annotation_ratio_is_claude() {
        // <20% annotated with 5+ let bindings
        let source = "\
let value_one = 1;\n\
let value_two = 2;\n\
let value_three = 3;\n\
let value_four = 4;\n\
let value_five = 5;\n\
let value_six = 6;\n\
let x: i32 = 0;\n\
let y = 0;\nlet z = 0;\nlet a = 0;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 0.8
                && s.description.contains("inference")),
            "expected low annotation ratio Claude signal (weight 0.8)"
        );
    }

    #[test]
    fn all_lines_under_100_chars_is_claude() {
        // 10+ non-empty lines, all ≤ 100 chars
        let source = (0..12)
            .map(|i| format!("let value_{i} = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 0.8
                && s.description.contains("100")),
            "expected all-lines-under-100 Claude signal (weight 0.8)"
        );
    }

    fn make_lines(n: usize, prefix: &str) -> String {
        (0..n).map(|i| format!("{prefix}line_{i} = {i}")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn python_short_lines_is_claude() {
        let source = make_lines(12, "");
        let signals = CodeStructureAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for short Python lines"
        );
    }

    #[test]
    fn python_sorted_imports_is_claude() {
        let mut lines: Vec<String> = vec![
            "import abc".into(),
            "import collections".into(),
            "import sys".into(),
        ];
        lines.extend((0..10).map(|i| format!("x_{i} = {i}")));
        let source = lines.join("\n");
        let signals = CodeStructureAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for sorted Python imports"
        );
    }

    #[test]
    fn javascript_short_lines_is_claude() {
        let source = make_lines(12, "const ");
        let signals = CodeStructureAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for short JS lines"
        );
    }

    #[test]
    fn go_short_lines_is_claude() {
        let source = make_lines(12, "var ");
        let signals = CodeStructureAnalyzer.analyze_go(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for short Go lines"
        );
    }
}

impl CodeStructureAnalyzer {
    fn analyze_python_impl(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Sorted imports (import x before import y)
        let import_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.trim().starts_with("import ") || l.trim().starts_with("from "))
            .copied()
            .collect();
        if import_lines.len() >= 3 {
            let is_sorted = import_lines.windows(2).all(|w| w[0] <= w[1]);
            if is_sorted {
                signals.push(Signal::new(
                    signal_ids::PYTHON_STRUCTURE_SORTED_IMPORTS,
                    "structure",
                    "Import statements are alphabetically sorted",
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        // Consistent blank lines (PEP 8: 2 between top-level, 1 between methods)
        let mut blank_runs = Vec::new();
        let mut current_run = 0usize;
        for line in &lines {
            if line.trim().is_empty() {
                current_run += 1;
            } else {
                if current_run > 0 {
                    blank_runs.push(current_run);
                }
                current_run = 0;
            }
        }
        if blank_runs.len() >= 3 {
            let all_same = blank_runs.iter().all(|&r| r == blank_runs[0]);
            if all_same {
                signals.push(Signal::new(
                    signal_ids::PYTHON_STRUCTURE_CONSISTENT_BLANK_LINES,
                    "structure",
                    "Perfectly consistent blank line spacing",
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        // Line length discipline
        let non_empty: Vec<usize> = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len())
            .collect();
        if non_empty.len() >= 10 {
            let over_88 = non_empty.iter().filter(|&&l| l > 88).count();
            if over_88 == 0 {
                signals.push(Signal::new(
                    signal_ids::PYTHON_STRUCTURE_LINES_UNDER_88,
                    "structure",
                    "All lines under 88 chars — PEP 8 / Black-style discipline",
                    ModelFamily::Claude,
                    0.8,
                ));
            }
        }

        signals
    }

    fn analyze_javascript_impl(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Sorted imports
        let import_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.trim().starts_with("import "))
            .copied()
            .collect();
        if import_lines.len() >= 3 {
            let is_sorted = import_lines.windows(2).all(|w| w[0] <= w[1]);
            if is_sorted {
                signals.push(Signal::new(
                    signal_ids::JS_STRUCTURE_SORTED_IMPORTS,
                    "structure",
                    "Import statements are alphabetically sorted",
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        // Consistent blank lines
        let mut blank_runs = Vec::new();
        let mut current_run = 0usize;
        for line in &lines {
            if line.trim().is_empty() {
                current_run += 1;
            } else {
                if current_run > 0 {
                    blank_runs.push(current_run);
                }
                current_run = 0;
            }
        }
        if blank_runs.len() >= 3 && blank_runs.iter().all(|&r| r == blank_runs[0]) {
            signals.push(Signal::new(
                signal_ids::JS_STRUCTURE_CONSISTENT_BLANK_LINES,
                "structure",
                "Perfectly consistent blank line spacing",
                ModelFamily::Claude,
                1.0,
            ));
        }

        // Line length
        let non_empty: Vec<usize> = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len())
            .collect();
        if non_empty.len() >= 10 {
            let over_100 = non_empty.iter().filter(|&&l| l > 100).count();
            if over_100 == 0 {
                signals.push(Signal::new(
                    signal_ids::JS_STRUCTURE_LINES_UNDER_100,
                    "structure",
                    "All lines under 100 chars — disciplined formatting",
                    ModelFamily::Claude,
                    0.8,
                ));
            } else if over_100 >= 5 {
                signals.push(Signal::new(
                    signal_ids::JS_STRUCTURE_MANY_LONG_LINES,
                    "structure",
                    format!("{over_100} lines over 100 chars"),
                    ModelFamily::Human,
                    1.0,
                ));
            }
        }

        signals
    }

    fn analyze_go_impl(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Sorted imports (Go typically groups stdlib + third-party)
        let import_block: Vec<&str> = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with('"') || (t.starts_with("\"") && t.ends_with('"'))
            })
            .copied()
            .collect();
        if import_block.len() >= 3 {
            let is_sorted = import_block.windows(2).all(|w| w[0] <= w[1]);
            if is_sorted {
                signals.push(Signal::new(
                    signal_ids::GO_STRUCTURE_SORTED_IMPORTS,
                    "structure",
                    "Import strings are sorted — goimports-style",
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        // Consistent blank lines
        let mut blank_runs = Vec::new();
        let mut current_run = 0usize;
        for line in &lines {
            if line.trim().is_empty() {
                current_run += 1;
            } else {
                if current_run > 0 {
                    blank_runs.push(current_run);
                }
                current_run = 0;
            }
        }
        if blank_runs.len() >= 3 && blank_runs.iter().all(|&r| r == blank_runs[0]) {
            signals.push(Signal::new(
                signal_ids::GO_STRUCTURE_CONSISTENT_BLANK_LINES,
                "structure",
                "Perfectly consistent blank line spacing",
                ModelFamily::Claude,
                1.0,
            ));
        }

        // Line length (Go convention: 80-120 chars)
        let non_empty: Vec<usize> = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len())
            .collect();
        if non_empty.len() >= 10 {
            let over_120 = non_empty.iter().filter(|&&l| l > 120).count();
            if over_120 == 0 {
                signals.push(Signal::new(
                    signal_ids::GO_STRUCTURE_LINES_UNDER_120,
                    "structure",
                    "All lines under 120 chars — gofmt-style discipline",
                    ModelFamily::Claude,
                    0.8,
                ));
            }
        }

        signals
    }
}

impl Analyzer for CodeStructureAnalyzer {
    fn name(&self) -> &str {
        "structure"
    }

    fn analyze_python(&self, source: &str) -> Vec<Signal> { Self::analyze_python_impl(source) }
    fn analyze_javascript(&self, source: &str) -> Vec<Signal> { Self::analyze_javascript_impl(source) }
    fn analyze_go(&self, source: &str) -> Vec<Signal> { Self::analyze_go_impl(source) }

    fn analyze(&self, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Type annotations on let bindings
        let let_lines: Vec<&&str> = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("let ") || trimmed.starts_with("let mut ")
            })
            .collect();
        let annotated = let_lines
            .iter()
            .filter(|l| {
                if let Some(eq_pos) = l.find('=') {
                    l[..eq_pos].contains(':')
                } else {
                    l.contains(':')
                }
            })
            .count();

        if !let_lines.is_empty() {
            let annotation_ratio = annotated as f64 / let_lines.len() as f64;
            if annotation_ratio > 0.7 {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_HIGH_TYPE_ANNOTATION,
                    self.name(),
                    format!(
                        "Explicit type annotations on {:.0}% of let bindings",
                        annotation_ratio * 100.0
                    ),
                    ModelFamily::Gpt,
                    1.0,
                ));
            } else if annotation_ratio < 0.2 && let_lines.len() >= 5 {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_LOW_TYPE_ANNOTATION,
                    self.name(),
                    "Relies on type inference — minimal annotations",
                    ModelFamily::Claude,
                    0.8,
                ));
            }
        }

        // Import ordering: check if use statements are sorted
        let use_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.trim().starts_with("use "))
            .map(|l| l.trim())
            .collect();
        if use_lines.len() >= 3 {
            let is_sorted = use_lines.windows(2).all(|w| w[0] <= w[1]);
            if is_sorted {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_SORTED_IMPORTS,
                    self.name(),
                    "Import statements are alphabetically sorted",
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        // Consistent blank line usage between functions
        let mut blank_runs = Vec::new();
        let mut current_run = 0;
        for line in &lines {
            if line.trim().is_empty() {
                current_run += 1;
            } else {
                if current_run > 0 {
                    blank_runs.push(current_run);
                }
                current_run = 0;
            }
        }
        if blank_runs.len() >= 3 {
            let all_same = blank_runs.iter().all(|&r| r == blank_runs[0]);
            if all_same {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_CONSISTENT_BLANK_LINES,
                    self.name(),
                    "Perfectly consistent blank line spacing",
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        // Line length consistency
        let non_empty_lines: Vec<usize> = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len())
            .collect();
        if non_empty_lines.len() >= 10 {
            let max_len = non_empty_lines.iter().max().copied().unwrap_or(0);
            let over_100 = non_empty_lines.iter().filter(|&&l| l > 100).count();
            if over_100 == 0 && max_len <= 100 {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_LINES_UNDER_100,
                    self.name(),
                    "All lines under 100 chars — disciplined formatting",
                    ModelFamily::Claude,
                    0.8,
                ));
            } else if over_100 >= 5 {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_MANY_LONG_LINES,
                    self.name(),
                    format!("{over_100} lines over 100 chars"),
                    ModelFamily::Human,
                    1.0,
                ));
            }
        }

        // Derive macro usage (AI loves deriving everything)
        let derive_count = lines
            .iter()
            .filter(|l| l.contains("#[derive("))
            .count();
        if derive_count >= 3 {
            let avg_derives: f64 = lines
                .iter()
                .filter(|l| l.contains("#[derive("))
                .map(|l| l.matches(',').count() as f64 + 1.0)
                .sum::<f64>()
                / derive_count as f64;
            if avg_derives >= 4.0 {
                signals.push(Signal::new(
                    signal_ids::RUST_STRUCTURE_HEAVY_DERIVE,
                    self.name(),
                    format!(
                        "Heavy derive usage (avg {:.1} traits per derive)",
                        avg_derives
                    ),
                    ModelFamily::Claude,
                    1.0,
                ));
            }
        }

        signals
    }
}
