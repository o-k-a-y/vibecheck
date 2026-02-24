use crate::analyzers::Analyzer;
use crate::language::Language;
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

impl AiSignalsAnalyzer {
    /// Language-agnostic signals shared across all languages.
    fn analyze_common(name: &str, source: &str) -> Vec<Signal> {
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
                source: name.into(),
                description: "No TODO/FIXME markers in a substantial file".into(),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
        }

        // Zero trailing whitespace — machine-perfect formatting
        let trailing_ws = lines.iter().filter(|l| !l.is_empty() && l.ends_with(' ')).count();
        if trailing_ws == 0 && total_lines > 20 {
            signals.push(Signal {
                source: name.into(),
                description: "Zero trailing whitespace — machine-perfect formatting".into(),
                family: ModelFamily::Gpt,
                weight: 0.5,
            });
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
            signals.push(Signal {
                source: name.into(),
                description: "No placeholder values — polished code".into(),
                family: ModelFamily::Claude,
                weight: 0.5,
            });
        }

        signals
    }

    fn analyze_python(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_common("ai_signals", source);
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: "No linter suppressions (noqa/type: ignore)".into(),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: format!("{commented_code} lines of commented-out code"),
                family: ModelFamily::Human,
                weight: 2.5,
            });
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: "Every function has a docstring — suspiciously thorough".into(),
                family: ModelFamily::Claude,
                weight: 2.0,
            });
        }

        signals
    }

    fn analyze_javascript(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_common("ai_signals", source);
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: "No linter/type suppressions (eslint-disable/@ts-ignore)".into(),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: format!("{commented_code} lines of commented-out code"),
                family: ModelFamily::Human,
                weight: 2.5,
            });
        }

        // JSDoc on all exported functions
        let jsdoc_count = lines
            .iter()
            .filter(|l| l.trim().starts_with("/**"))
            .count();
        if jsdoc_count >= 3 {
            signals.push(Signal {
                source: "ai_signals".into(),
                description: format!("{jsdoc_count} JSDoc comment blocks — thorough documentation"),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: format!("{console_log} console.log calls — likely debugging artifacts"),
                family: ModelFamily::Human,
                weight: 2.0,
            });
        }

        signals
    }

    fn analyze_go(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_common("ai_signals", source);
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();

        // //nolint suppressions
        let suppression_count = lines
            .iter()
            .filter(|l| l.contains("//nolint") || l.contains("// nolint"))
            .count();
        if suppression_count == 0 && total_lines > 30 {
            signals.push(Signal {
                source: "ai_signals".into(),
                description: "No nolint suppressions — clean linter compliance".into(),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: format!("{commented_code} lines of commented-out code"),
                family: ModelFamily::Human,
                weight: 2.5,
            });
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
            signals.push(Signal {
                source: "ai_signals".into(),
                description: "All exported identifiers have doc comments — Go-idiomatic and thorough".into(),
                family: ModelFamily::Claude,
                weight: 2.0,
            });
        }

        signals
    }
}

impl Analyzer for AiSignalsAnalyzer {
    fn name(&self) -> &str {
        "ai_signals"
    }

    fn analyze_with_language(&self, source: &str, lang: Option<Language>) -> Vec<Signal> {
        match lang {
            None | Some(Language::Rust) => self.analyze(source),
            Some(Language::Python) => Self::analyze_python(source),
            Some(Language::JavaScript) => Self::analyze_javascript(source),
            Some(Language::Go) => Self::analyze_go(source),
        }
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
