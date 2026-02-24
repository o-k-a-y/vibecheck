use crate::analyzers::Analyzer;
use crate::language::Language;
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
    fn rust_unwrap_signal_not_emitted_for_python_file() {
        // A Python file with `.unwrap()` text (unlikely but possible) should not
        // trigger Rust-specific error-handling signals.
        use crate::language::Language;
        let python_source = (0..35)
            .map(|i| format!("result_{i} = compute_{i}()  # no .unwrap() here"))
            .collect::<Vec<_>>()
            .join("\n");
        let signals = ErrorHandlingAnalyzer.analyze_with_language(&python_source, Some(Language::Python));
        assert!(
            !signals.iter().any(|s| s.description.contains("unwrap")),
            "Rust .unwrap() signal fired on a Python file"
        );
    }

    #[test]
    fn python_broad_except_is_human() {
        use crate::language::Language;
        let source = vec![
            "try:",
            "    do_thing()",
            "except Exception:",
            "    pass",
            "try:",
            "    do_other()",
            "except Exception:",
            "    pass",
        ]
        .into_iter()
        .chain((0..5).map(|_| "x = 1"))
        .collect::<Vec<_>>()
        .join("\n");
        let signals = ErrorHandlingAnalyzer.analyze_with_language(&source, Some(Language::Python));
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.description.contains("broad")),
            "expected broad except Human signal"
        );
    }

    #[test]
    fn go_fmt_errorf_wrap_is_claude() {
        use crate::language::Language;
        let source = (0..12)
            .map(|i| {
                if i < 2 {
                    format!("return fmt.Errorf(\"step {i}: %w\", err)")
                } else {
                    format!("x := step{i}()")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let signals = ErrorHandlingAnalyzer.analyze_with_language(&source, Some(Language::Go));
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("Errorf")),
            "expected fmt.Errorf Claude signal"
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

impl ErrorHandlingAnalyzer {
    fn analyze_python(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Bare or overly broad except clause — human shortcut
        let broad_except = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t == "except:" || t.starts_with("except Exception:")
            })
            .count();
        if broad_except >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{broad_except} broad except clauses — swallows all exceptions"),
                family: ModelFamily::Human,
                weight: 1.5,
            });
        }

        // Specific exception types — AI-like precision
        let specific_except = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("except ")
                    && !t.starts_with("except Exception")
                    && t != "except:"
            })
            .count();
        if specific_except >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{specific_except} specific exception types — precise error handling"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // No try/except in a large file
        let try_count = lines.iter().filter(|l| l.trim() == "try:").count();
        if try_count == 0 && total_lines > 40 {
            signals.push(Signal {
                source: "errors".into(),
                description: "No try/except blocks in a substantial file".into(),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
        }

        // raise ... from (idiomatic exception chaining)
        let raise_from = lines
            .iter()
            .filter(|l| l.trim().starts_with("raise ") && l.contains(" from "))
            .count();
        if raise_from >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{raise_from} raise…from patterns — idiomatic exception chaining"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        signals
    }

    fn analyze_javascript(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // console.error / console.warn left in code — human debugging artifact
        let console_err = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with("//")
                    && (t.contains("console.error(") || t.contains("console.warn("))
            })
            .count();
        if console_err >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{console_err} console.error/warn calls — debug artifacts"),
                family: ModelFamily::Human,
                weight: 1.0,
            });
        }

        // instanceof Error checks — typed error handling
        let typed_catch = lines
            .iter()
            .filter(|l| l.contains("instanceof ") && l.contains("Error"))
            .count();
        if typed_catch >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{typed_catch} instanceof Error checks — typed error handling"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // Promise .catch() vs try/catch: style indicator
        let promise_catch = lines.iter().filter(|l| l.contains(".catch(")).count();
        let try_catch_blocks = lines
            .iter()
            .filter(|l| l.trim().starts_with("} catch") || l.trim().starts_with("catch ("))
            .count();
        if promise_catch >= 2 && try_catch_blocks == 0 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{promise_catch} .catch() chains — promise-based error handling"),
                family: ModelFamily::Human,
                weight: 0.8,
            });
        } else if try_catch_blocks >= 2 && promise_catch == 0 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{try_catch_blocks} try/catch blocks — structured async error handling"),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
        }

        // Typed Error constructors: new TypeError(...), new RangeError(...), etc.
        let typed_throw = lines
            .iter()
            .filter(|l| {
                l.contains("new Error(")
                    || l.contains("new TypeError(")
                    || l.contains("new RangeError(")
                    || l.contains("new SyntaxError(")
            })
            .count();
        if typed_throw >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{typed_throw} typed Error constructions — specific error classes"),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
        }

        signals
    }

    fn analyze_go(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // Simple if err != nil { return err } — idiomatic but not AI-specific
        let simple_err_return = lines
            .iter()
            .filter(|l| l.contains("err != nil") && (l.contains("return err") || l.contains("return nil, err")))
            .count();
        if simple_err_return >= 3 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{simple_err_return} simple 'if err != nil' returns — idiomatic propagation"),
                family: ModelFamily::Human,
                weight: 0.8,
            });
        }

        // fmt.Errorf with %w — idiomatic error wrapping
        let errorf_wrap = lines
            .iter()
            .filter(|l| l.contains("fmt.Errorf(") && l.contains("%w"))
            .count();
        if errorf_wrap >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{errorf_wrap} fmt.Errorf(%w) wrappings — idiomatic error context"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // errors.Is / errors.As — modern structured error inspection
        let errors_sentinel = lines
            .iter()
            .filter(|l| l.contains("errors.Is(") || l.contains("errors.As("))
            .count();
        if errors_sentinel >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{errors_sentinel} errors.Is/As calls — structured error inspection"),
                family: ModelFamily::Claude,
                weight: 1.2,
            });
        }

        // panic() in Go — non-idiomatic for recoverable errors
        let panic_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with("//") && t.contains("panic(")
            })
            .count();
        if panic_count >= 2 {
            signals.push(Signal {
                source: "errors".into(),
                description: format!("{panic_count} panic() calls — non-recoverable or human shortcut"),
                family: ModelFamily::Human,
                weight: 1.5,
            });
        }

        signals
    }
}

impl Analyzer for ErrorHandlingAnalyzer {
    fn name(&self) -> &str {
        "errors"
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
