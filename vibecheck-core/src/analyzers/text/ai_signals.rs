use crate::analyzers::Analyzer;
use crate::heuristics::signal_ids;
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
        // 35 lines, no TODO/FIXME → Claude signal weight 0.8
        let source = (0..35).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 0.8),
            "expected no-TODO Claude signal (weight 0.8)"
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
        // 2+ commented-out code lines → Human signal weight 2.0
        let mut lines: Vec<&str> = vec![
            "// let old_value = compute();",
            "// let result = old_value * 2;",
        ];
        // Pad to 10+ lines so the guard passes
        for _ in 0..10 { lines.push("let x = 1;"); }
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 2.0),
            "expected commented-out code Human signal (weight 2.0)"
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

    fn large_clean_source(prefix: &str) -> String {
        // 35 lines of clean code with no TODO/FIXME/trailing whitespace
        let mut lines: Vec<String> = (0..35).map(|i| format!("{prefix}line_{i} = {i}")).collect();
        lines[0] = format!("{prefix}line_0 = 0");
        lines.join("\n")
    }

    #[test]
    fn python_no_todo_is_claude() {
        let source = large_clean_source("");
        let signals = AiSignalsAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for Python source with no TODO"
        );
    }

    #[test]
    fn javascript_no_todo_is_claude() {
        let source = large_clean_source("const ");
        let signals = AiSignalsAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for JS source with no TODO"
        );
    }

    #[test]
    fn go_no_todo_is_claude() {
        let source = large_clean_source("var ");
        let signals = AiSignalsAnalyzer.analyze_go(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for Go source with no TODO"
        );
    }
}

impl AiSignalsAnalyzer {
    /// Language-agnostic signals shared across Rust / Python / JS / Go.
    ///
    /// Each caller passes the language-specific signal ID constants so that
    /// the heuristics system can look them up by stable ID.
    fn analyze_common(
        no_todo_id: &str,
        no_trailing_ws_id: &str,
        no_placeholder_id: &str,
        triple_backtick_id: &str,
        source: &str,
    ) -> Vec<Signal> {
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
            signals.push(Signal::new(
                no_todo_id,
                "ai_signals",
                "No TODO/FIXME markers in a substantial file",
                ModelFamily::Claude,
                0.8,
            ));
        }

        // Zero trailing whitespace — machine-perfect formatting
        let trailing_ws = lines.iter().filter(|l| !l.is_empty() && l.ends_with(' ')).count();
        if trailing_ws == 0 && total_lines > 20 {
            signals.push(Signal::new(
                no_trailing_ws_id,
                "ai_signals",
                "Zero trailing whitespace — machine-perfect formatting",
                ModelFamily::Gpt,
                0.5,
            ));
        }

        // Placeholder strings
        let placeholder_count = lines
            .iter()
            .filter(|l| {
                let lower = l.to_lowercase();
                lower.contains("lorem ipsum")
                    || lower.contains("asdf")
                    || lower.contains("placeholder")
            })
            .count();
        if placeholder_count == 0 && total_lines > 30 {
            signals.push(Signal::new(
                no_placeholder_id,
                "ai_signals",
                "No placeholder values — polished code",
                ModelFamily::Gpt,
                0.3,
            ));
        }

        // GPT: markdown triple-backtick in code comments
        let backtick_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("//") || t.starts_with('#')) && t.contains("```")
            })
            .count();
        if backtick_count >= 1 {
            signals.push(Signal::new(
                triple_backtick_id,
                "ai_signals",
                format!("{backtick_count} triple-backtick(s) in comments — markdown artifact"),
                ModelFamily::Gpt,
                1.5,
            ));
        }

        signals
    }

    fn analyze_python_impl(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_common(
            signal_ids::PYTHON_AI_SIGNALS_NO_TODO,
            signal_ids::PYTHON_AI_SIGNALS_NO_TRAILING_WS,
            signal_ids::PYTHON_AI_SIGNALS_NO_PLACEHOLDER,
            signal_ids::PYTHON_AI_SIGNALS_TRIPLE_BACKTICK,
            source,
        );
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();

        // Linter suppression comments — human workaround
        let suppression_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.contains("# noqa") || t.contains("# type: ignore") || t.contains("# pylint: disable")
            })
            .count();
        if suppression_count == 0 && total_lines > 30 {
            signals.push(Signal::new(
                signal_ids::PYTHON_AI_SIGNALS_NO_LINTER_SUPPRESSION,
                "ai_signals",
                "No linter suppressions (noqa/type: ignore)",
                ModelFamily::Claude,
                0.5,
            ));
        }

        // Commented-out code (Python style)
        let commented_code = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("# def ")
                    || t.starts_with("# class ")
                    || t.starts_with("# import ")
                    || t.starts_with("# return ")
                    || t.starts_with("# print(")
            })
            .count();
        if commented_code >= 2 {
            signals.push(Signal::new(
                signal_ids::PYTHON_AI_SIGNALS_COMMENTED_OUT_CODE,
                "ai_signals",
                format!("{commented_code} lines of commented-out code"),
                ModelFamily::Human,
                2.0,
            ));
        }

        // All functions have docstrings — AI is very thorough
        let fn_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("def ") || t.starts_with("async def ")
            })
            .count();
        let docstring_count = lines.windows(2).filter(|w| {
            let curr = w[0].trim();
            let next = w[1].trim();
            (curr.starts_with("def ") || curr.starts_with("async def "))
                && (next.starts_with("\"\"\"") || next.starts_with("'''"))
        }).count();
        if fn_count >= 3 && docstring_count == fn_count {
            signals.push(Signal::new(
                signal_ids::PYTHON_AI_SIGNALS_ALL_FNS_DOCUMENTED,
                "ai_signals",
                "Every function has a docstring — suspiciously thorough",
                ModelFamily::Claude,
                2.0,
            ));
        }

        // Human: pragma/lint overrides present
        let pragma_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.contains("# type: ignore") || t.contains("# noqa") || t.contains("# pylint: disable")
            })
            .count();
        if pragma_count >= 1 {
            signals.push(Signal::new(
                signal_ids::PYTHON_AI_SIGNALS_PRAGMA,
                "ai_signals",
                format!("{pragma_count} pragma/lint override(s)"),
                ModelFamily::Human,
                1.5,
            ));
        }

        signals
    }

    fn analyze_javascript_impl(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_common(
            signal_ids::JS_AI_SIGNALS_NO_TODO,
            signal_ids::JS_AI_SIGNALS_NO_TRAILING_WS,
            signal_ids::JS_AI_SIGNALS_NO_PLACEHOLDER,
            signal_ids::JS_AI_SIGNALS_TRIPLE_BACKTICK,
            source,
        );
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();

        // Linter/type-checker suppressions
        let suppression_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.contains("// eslint-disable")
                    || t.contains("@ts-ignore")
                    || t.contains("@ts-nocheck")
            })
            .count();
        if suppression_count == 0 && total_lines > 30 {
            signals.push(Signal::new(
                signal_ids::JS_AI_SIGNALS_NO_LINTER_SUPPRESSION,
                "ai_signals",
                "No linter/type suppressions (eslint-disable/@ts-ignore)",
                ModelFamily::Claude,
                0.5,
            ));
        }

        // Commented-out code (JS style)
        let commented_code = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("// const ")
                    || t.starts_with("// let ")
                    || t.starts_with("// function ")
                    || t.starts_with("// import ")
                    || t.starts_with("// return ")
                    || t.starts_with("// console.")
            })
            .count();
        if commented_code >= 2 {
            signals.push(Signal::new(
                signal_ids::JS_AI_SIGNALS_COMMENTED_OUT_CODE,
                "ai_signals",
                format!("{commented_code} lines of commented-out code"),
                ModelFamily::Human,
                2.0,
            ));
        }

        // Pragma/lint override directives (Human indicator)
        if suppression_count >= 1 {
            signals.push(Signal::new(
                signal_ids::JS_AI_SIGNALS_PRAGMA,
                "ai_signals",
                format!("{suppression_count} linter/type pragma directives"),
                ModelFamily::Human,
                1.5,
            ));
        }

        // JSDoc on all exported functions
        let jsdoc_count = lines
            .iter()
            .filter(|l| l.trim().starts_with("/**"))
            .count();
        if jsdoc_count >= 3 {
            signals.push(Signal::new(
                signal_ids::JS_AI_SIGNALS_JSDOC_BLOCKS,
                "ai_signals",
                format!("{jsdoc_count} JSDoc comment blocks — thorough documentation"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        // console.log left in code — debugging artifact
        let console_log = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with("//") && t.contains("console.log(")
            })
            .count();
        if console_log >= 3 {
            signals.push(Signal::new(
                signal_ids::JS_AI_SIGNALS_CONSOLE_LOG,
                "ai_signals",
                format!("{console_log} console.log calls — likely debugging artifacts"),
                ModelFamily::Human,
                2.0,
            ));
        }

        signals
    }

    fn analyze_go_impl(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_common(
            signal_ids::GO_AI_SIGNALS_NO_TODO,
            signal_ids::GO_AI_SIGNALS_NO_TRAILING_WS,
            signal_ids::GO_AI_SIGNALS_NO_PLACEHOLDER,
            signal_ids::GO_AI_SIGNALS_TRIPLE_BACKTICK,
            source,
        );
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();

        // //nolint suppressions
        let suppression_count = lines
            .iter()
            .filter(|l| l.contains("//nolint") || l.contains("// nolint"))
            .count();
        if suppression_count == 0 && total_lines > 30 {
            signals.push(Signal::new(
                signal_ids::GO_AI_SIGNALS_NO_NOLINT,
                "ai_signals",
                "No nolint suppressions — clean linter compliance",
                ModelFamily::Claude,
                0.5,
            ));
        }

        // Pragma/lint override directives (Human indicator)
        if suppression_count >= 1 {
            signals.push(Signal::new(
                signal_ids::GO_AI_SIGNALS_PRAGMA,
                "ai_signals",
                format!("{suppression_count} nolint pragma directives"),
                ModelFamily::Human,
                1.5,
            ));
        }

        // Commented-out code (Go style)
        let commented_code = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("// func ")
                    || t.starts_with("// var ")
                    || t.starts_with("// type ")
                    || t.starts_with("// import ")
                    || t.starts_with("// return ")
                    || t.starts_with("// fmt.")
            })
            .count();
        if commented_code >= 2 {
            signals.push(Signal::new(
                signal_ids::GO_AI_SIGNALS_COMMENTED_OUT_CODE,
                "ai_signals",
                format!("{commented_code} lines of commented-out code"),
                ModelFamily::Human,
                2.0,
            ));
        }

        // All exported identifiers have doc comments — Go convention
        let exported_fn = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("func ") && t.len() > 5
                    && t.chars().nth(5).map(|c| c.is_uppercase()).unwrap_or(false))
                    || (t.starts_with("type ") && t.contains(" struct") || t.contains(" interface"))
            })
            .count();
        let doc_before_exported = lines.windows(2).filter(|w| {
            let prev = w[0].trim();
            let curr = w[1].trim();
            prev.starts_with("// ") && !prev.starts_with("// nolint")
                && (curr.starts_with("func ") || curr.starts_with("type "))
                && curr.chars().nth(if curr.starts_with("func ") { 5 } else { 5 })
                    .map(|c| c.is_uppercase()).unwrap_or(false)
        }).count();
        if exported_fn >= 3 && doc_before_exported == exported_fn {
            signals.push(Signal::new(
                signal_ids::GO_AI_SIGNALS_ALL_EXPORTED_DOCUMENTED,
                "ai_signals",
                "All exported identifiers have doc comments — Go-idiomatic and thorough",
                ModelFamily::Claude,
                2.0,
            ));
        }

        signals
    }
}

impl Analyzer for AiSignalsAnalyzer {
    fn name(&self) -> &str {
        "ai_signals"
    }

    fn analyze_python(&self, source: &str) -> Vec<Signal> {
        Self::analyze_python_impl(source)
    }

    fn analyze_javascript(&self, source: &str) -> Vec<Signal> {
        Self::analyze_javascript_impl(source)
    }

    fn analyze_go(&self, source: &str) -> Vec<Signal> {
        Self::analyze_go_impl(source)
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
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_NO_TODO,
                self.name(),
                "No TODO/FIXME markers in a substantial file",
                ModelFamily::Claude,
                0.8,
            ));
        }

        // No dead code markers (#[allow(dead_code)], #[allow(unused)])
        let dead_code_markers = ["allow(dead_code)", "allow(unused)", "#[cfg(dead_code)]"];
        let has_dead_code = lines
            .iter()
            .any(|l| dead_code_markers.iter().any(|m| l.contains(m)));
        if !has_dead_code && total_lines > 30 {
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_NO_DEAD_CODE,
                self.name(),
                "No dead code suppressions",
                ModelFamily::Claude,
                0.5,
            ));
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
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_ALL_FNS_DOCUMENTED,
                self.name(),
                "Every function has a doc comment — suspiciously thorough",
                ModelFamily::Claude,
                2.0,
            ));
        }

        // Consistent formatting: no trailing whitespace, consistent indentation
        let trailing_ws = lines.iter().filter(|l| !l.is_empty() && l.ends_with(' ')).count();
        if trailing_ws == 0 && total_lines > 20 {
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_NO_TRAILING_WS,
                self.name(),
                "Zero trailing whitespace — machine-perfect formatting",
                ModelFamily::Gpt,
                0.5,
            ));
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
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_COMMENTED_OUT_CODE,
                self.name(),
                format!("{commented_code} lines of commented-out code"),
                ModelFamily::Human,
                2.0,
            ));
        }

        // Pragma/lint override directives (Human indicator)
        let rust_pragma_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("#[allow(")
                    || t.starts_with("#[cfg(")
                    || t.contains("#![allow(")
                    || t.contains("// SAFETY:")
            })
            .count();
        if rust_pragma_count >= 2 {
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_PRAGMA,
                self.name(),
                format!("{rust_pragma_count} allow/cfg pragma directives"),
                ModelFamily::Human,
                1.5,
            ));
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
            signals.push(Signal::new(
                signal_ids::RUST_AI_SIGNALS_NO_PLACEHOLDER,
                self.name(),
                "No placeholder values — polished code",
                ModelFamily::Gpt,
                0.3,
            ));
        }

        signals
    }
}
