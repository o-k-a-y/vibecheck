use crate::analyzers::Analyzer;
use crate::heuristics::signal_ids;
use crate::report::{ModelFamily, Signal};

pub struct IdiomUsageAnalyzer;

impl IdiomUsageAnalyzer {
    fn analyze_python_impl(source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines < 10 {
            return signals;
        }

        // List/dict/set comprehensions — idiomatic Python
        let comprehension_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with('#') && t.contains(" for ") && t.contains(" in ")
                    && (t.contains('[') || t.contains('{'))
            })
            .count();
        if comprehension_count >= 3 {
            signals.push(Signal::new(
                signal_ids::PYTHON_IDIOMS_COMPREHENSIONS,
                "idioms",
                format!("{comprehension_count} list/dict/set comprehensions — pythonic style"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        // All function defs have return type annotations → AI thoroughness
        let total_defs = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("def ") || t.starts_with("async def ")
            })
            .count();
        let typed_defs = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("def ") || t.starts_with("async def ")) && t.contains("->")
            })
            .count();
        if total_defs >= 3 && typed_defs == total_defs {
            signals.push(Signal::new(
                signal_ids::PYTHON_IDIOMS_RETURN_TYPE_ANNOTATIONS,
                "idioms",
                "All function definitions have return type annotations",
                ModelFamily::Claude,
                1.5,
            ));
        }

        // Context managers (with statement)
        let with_count = lines.iter().filter(|l| l.trim().starts_with("with ")).count();
        if with_count >= 2 {
            signals.push(Signal::new(
                signal_ids::PYTHON_IDIOMS_CONTEXT_MANAGERS,
                "idioms",
                format!("{with_count} context manager usages (with statement) — safe resource handling"),
                ModelFamily::Claude,
                0.8,
            ));
        }

        // Functional builtins: enumerate, zip, any, all, map, filter, sorted
        let builtins = ["enumerate(", "zip(", "any(", "all(", "sorted(", "reversed(", "filter(", "map("];
        let builtin_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with('#') && builtins.iter().any(|b| t.contains(b))
            })
            .count();
        if builtin_count >= 4 {
            signals.push(Signal::new(
                signal_ids::PYTHON_IDIOMS_FUNCTIONAL_BUILTINS,
                "idioms",
                format!("{builtin_count} functional builtin usages — idiomatic Python"),
                ModelFamily::Claude,
                1.0,
            ));
        }

        // f-strings vs old-style formatting
        let fstring_count = lines
            .iter()
            .filter(|l| l.contains("f\"") || l.contains("f'"))
            .count();
        let old_format_count = lines
            .iter()
            .filter(|l| l.contains("% (") || l.contains("% \"") || l.contains(".format("))
            .count();
        if fstring_count >= 3 && old_format_count == 0 {
            signals.push(Signal::new(
                signal_ids::PYTHON_IDIOMS_FSTRINGS,
                "idioms",
                "Uses f-strings exclusively — modern string formatting",
                ModelFamily::Claude,
                0.8,
            ));
        } else if old_format_count >= 3 {
            signals.push(Signal::new(
                signal_ids::PYTHON_IDIOMS_OLD_FORMAT,
                "idioms",
                format!("{old_format_count} old-style format calls — legacy string formatting"),
                ModelFamily::Human,
                1.0,
            ));
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

        // Arrow functions vs regular function declarations
        let arrow_fn_count = lines.iter().filter(|l| l.contains("=>")).count();
        let regular_fn_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("function ") || t.contains(" function(") || t.contains(" function (")
            })
            .count();
        if arrow_fn_count >= 5 && regular_fn_count == 0 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_ARROW_FNS_ONLY,
                "idioms",
                format!("{arrow_fn_count} arrow functions, no regular functions — modern ES6+ style"),
                ModelFamily::Claude,
                1.5,
            ));
        } else if regular_fn_count >= 3 && arrow_fn_count == 0 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_REGULAR_FNS_ONLY,
                "idioms",
                format!("{regular_fn_count} traditional function declarations — older style"),
                ModelFamily::Human,
                1.0,
            ));
        }

        // var declarations — legacy
        let var_count = lines.iter().filter(|l| l.trim().starts_with("var ")).count();
        let const_count = lines.iter().filter(|l| l.trim().starts_with("const ")).count();
        if var_count >= 3 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_VAR_DECLARATIONS,
                "idioms",
                format!("{var_count} var declarations — legacy hoisting style"),
                ModelFamily::Human,
                1.5,
            ));
        } else if const_count >= 5 && var_count == 0 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_CONST_DECLARATIONS,
                "idioms",
                format!("{const_count} const declarations — immutability-first approach"),
                ModelFamily::Claude,
                1.0,
            ));
        }

        // Optional chaining (?.) and nullish coalescing (??)
        let null_safe_count = lines
            .iter()
            .filter(|l| l.contains("?.") || l.contains("??"))
            .count();
        if null_safe_count >= 3 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_NULL_SAFE_OPS,
                "idioms",
                format!("{null_safe_count} optional chaining/nullish ops — modern null safety"),
                ModelFamily::Claude,
                1.0,
            ));
        }

        // Destructuring assignments
        let destructure_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                (t.starts_with("const {") || t.starts_with("let {")
                    || t.starts_with("const [") || t.starts_with("let ["))
                    && t.contains('=')
            })
            .count();
        if destructure_count >= 3 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_DESTRUCTURING,
                "idioms",
                format!("{destructure_count} destructuring assignments — idiomatic ES6+"),
                ModelFamily::Claude,
                0.8,
            ));
        }

        // async/await
        let async_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                !t.starts_with("//") && (t.contains("async ") || t.contains("await "))
            })
            .count();
        if async_count >= 3 {
            signals.push(Signal::new(
                signal_ids::JS_IDIOMS_ASYNC_AWAIT,
                "idioms",
                format!("{async_count} async/await usages — modern asynchronous style"),
                ModelFamily::Claude,
                0.8,
            ));
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

        // Compile-time interface satisfaction check: var _ Interface = (*Impl)(nil)
        let interface_check = lines
            .iter()
            .filter(|l| l.contains("var _") && l.contains(")(nil)"))
            .count();
        if interface_check >= 1 {
            signals.push(Signal::new(
                signal_ids::GO_IDIOMS_INTERFACE_CHECKS,
                "idioms",
                format!("{interface_check} compile-time interface checks — thorough Go design"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        // Goroutines
        let goroutine_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.contains("go func") || (t.starts_with("go ") && !t.starts_with("// "))
            })
            .count();
        if goroutine_count >= 2 {
            signals.push(Signal::new(
                signal_ids::GO_IDIOMS_GOROUTINES,
                "idioms",
                format!("{goroutine_count} goroutine launches — concurrent design"),
                ModelFamily::Gpt,
                0.8,
            ));
        }

        // defer — idiomatic cleanup
        let defer_count = lines.iter().filter(|l| l.trim().starts_with("defer ")).count();
        if defer_count >= 2 {
            signals.push(Signal::new(
                signal_ids::GO_IDIOMS_DEFER_STMTS,
                "idioms",
                format!("{defer_count} defer statements — idiomatic resource cleanup"),
                ModelFamily::Claude,
                0.8,
            ));
        }

        // Table-driven test pattern
        let table_driven = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.contains("testCases") || t.contains("testcases") || t == "tests := []struct {"
                    || t.contains("cases := []struct {")
            })
            .count();
        if table_driven >= 1 {
            signals.push(Signal::new(
                signal_ids::GO_IDIOMS_TABLE_DRIVEN_TESTS,
                "idioms",
                "Table-driven test pattern detected — idiomatic Go testing",
                ModelFamily::Claude,
                1.5,
            ));
        }

        // iota constants
        let iota_count = lines.iter().filter(|l| l.contains("iota")).count();
        if iota_count >= 1 {
            signals.push(Signal::new(
                signal_ids::GO_IDIOMS_IOTA_CONSTANTS,
                "idioms",
                format!("{iota_count} iota constant(s) — idiomatic Go enumeration"),
                ModelFamily::Claude,
                0.8,
            ));
        }

        signals
    }
}

impl Analyzer for IdiomUsageAnalyzer {
    fn name(&self) -> &str {
        "idioms"
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

        // Iterator chain usage (map, filter, flat_map, collect, fold)
        let iterator_methods = [".map(", ".filter(", ".flat_map(", ".collect()", ".fold(", ".filter_map("];
        let iterator_count = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.starts_with("//")
                    && iterator_methods.iter().any(|m| l.contains(m))
            })
            .count();
        if iterator_count >= 5 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_ITERATOR_CHAINS,
                self.name(),
                format!("{iterator_count} iterator chain usages — textbook-idiomatic Rust"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        // Builder pattern usage
        let builder_chain = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with('.') && !trimmed.starts_with("//")
            })
            .count();
        if builder_chain >= 8 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_BUILDER_PATTERN,
                self.name(),
                format!("{builder_chain} method chain continuation lines — builder pattern"),
                ModelFamily::Gpt,
                1.0,
            ));
        }

        // impl Display / impl std::fmt::Display
        if source.contains("impl std::fmt::Display") || source.contains("impl Display for") {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_IMPL_DISPLAY,
                self.name(),
                "Implements Display trait — thorough API design",
                ModelFamily::Claude,
                1.0,
            ));
        }

        // From/Into implementations
        let from_impl = lines
            .iter()
            .filter(|l| l.contains("impl From<") || l.contains("impl Into<"))
            .count();
        if from_impl >= 2 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_FROM_INTO_IMPLS,
                self.name(),
                format!("{from_impl} From/Into implementations — conversion-rich design"),
                ModelFamily::Claude,
                1.0,
            ));
        }

        // Self:: usage in impl blocks (textbook Rust)
        let self_usage = lines
            .iter()
            .filter(|l| l.contains("Self::") || l.contains("Self {"))
            .count();
        if self_usage >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_SELF_USAGE,
                self.name(),
                format!("{self_usage} uses of Self — consistent self-referencing"),
                ModelFamily::Claude,
                0.8,
            ));
        }

        // if let / while let (pattern matching idioms)
        let pattern_match_count = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("if let ") || trimmed.starts_with("while let ")
            })
            .count();
        if pattern_match_count >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_PATTERN_MATCHING,
                self.name(),
                format!("{pattern_match_count} if-let/while-let patterns"),
                ModelFamily::Claude,
                0.8,
            ));
        }

        // String formatting with format!() vs concatenation
        let format_macro = lines
            .iter()
            .filter(|l| l.contains("format!("))
            .count();
        let string_concat = lines
            .iter()
            .filter(|l| l.contains("+ \"") || l.contains("+ &"))
            .count();
        if format_macro >= 3 && string_concat == 0 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_FORMAT_MACRO,
                self.name(),
                "Uses format!() exclusively, no string concatenation",
                ModelFamily::Claude,
                0.8,
            ));
        } else if string_concat >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_STRING_CONCAT,
                self.name(),
                format!("{string_concat} string concatenations — less idiomatic"),
                ModelFamily::Human,
                1.0,
            ));
        }

        // Over-abstraction: many trait definitions in a single file
        let trait_count = lines
            .iter()
            .filter(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ")
            })
            .count();
        if trait_count >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_IDIOMS_MANY_TRAITS,
                self.name(),
                format!("{trait_count} trait definitions — heavy abstraction"),
                ModelFamily::Gpt,
                1.5,
            ));
        }

        signals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::Analyzer;
    use crate::report::ModelFamily;

    fn run(source: &str) -> Vec<Signal> {
        IdiomUsageAnalyzer.analyze(source)
    }

    fn pad(base: &str, total: usize) -> String {
        let mut lines: Vec<String> = base.lines().map(|l| l.to_string()).collect();
        while lines.len() < total {
            lines.push("let padding = 0;".to_string());
        }
        lines.join("\n")
    }

    #[test]
    fn five_iterator_methods_is_claude() {
        let source = pad(
            "let a = v.map(|x| x);\nlet b = v.filter(|x| true);\nlet c = v.flat_map(|x| x);\n\
             let d = v.collect();\nlet e = v.filter_map(|x| Some(x));",
            12,
        );
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected iterator methods Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn impl_display_for_is_claude() {
        let source = pad(
            "impl Display for MyType {\nfn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }\n}",
            12,
        );
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.0
                && s.description.contains("Display")),
            "expected impl Display Claude signal (weight 1.0)"
        );
    }

    #[test]
    fn two_impl_from_is_claude() {
        let source = pad(
            "impl From<String> for MyType {}\nimpl From<i32> for MyType {}",
            12,
        );
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.0
                && s.description.contains("From")),
            "expected From/Into impls Claude signal (weight 1.0)"
        );
    }

    #[test]
    fn three_if_let_is_claude() {
        let source = pad(
            "if let Some(x) = opt1 {}\nif let Some(y) = opt2 {}\nif let Some(z) = opt3 {}",
            12,
        );
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 0.8
                && s.description.contains("if-let")),
            "expected if-let pattern Claude signal (weight 0.8)"
        );
    }

    #[test]
    fn rust_iterator_signal_not_emitted_for_python_file() {
        use crate::language::Language;
        // Python file with no Rust iterators should not get iterator-chain signal
        let source = (0..12)
            .map(|i| format!("x_{i} = compute_{i}()"))
            .collect::<Vec<_>>()
            .join("\n");
        let signals = IdiomUsageAnalyzer.analyze_with_language(&source, Some(Language::Python));
        assert!(
            !signals.iter().any(|s| s.description.contains("iterator chain")),
            "Rust iterator-chain signal fired on Python file"
        );
    }

    #[test]
    fn python_list_comprehensions_is_claude() {
        use crate::language::Language;
        let source = vec![
            "a = [x * 2 for x in items]",
            "b = {k: v for k, v in pairs}",
            "c = [f(x) for x in range(10)]",
        ]
        .into_iter()
        .chain((0..10).map(|_| "pass"))
        .collect::<Vec<_>>()
        .join("\n");
        let signals = IdiomUsageAnalyzer.analyze_with_language(&source, Some(Language::Python));
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("comprehension")),
            "expected list comprehension Claude signal"
        );
    }

    #[test]
    fn javascript_var_declarations_is_human() {
        use crate::language::Language;
        let source = vec![
            "var x = 1;",
            "var y = 2;",
            "var z = 3;",
        ]
        .into_iter()
        .chain((0..10).map(|_| "doSomething();"))
        .collect::<Vec<_>>()
        .join("\n");
        let signals = IdiomUsageAnalyzer.analyze_with_language(&source, Some(Language::JavaScript));
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.description.contains("var")),
            "expected var declaration Human signal"
        );
    }

    #[test]
    fn go_interface_check_is_claude() {
        use crate::language::Language;
        let source = vec![
            "var _ MyInterface = (*MyImpl)(nil)",
            "var _ OtherInterface = (*OtherImpl)(nil)",
        ]
        .into_iter()
        .chain((0..10).map(|_| "x := 1"))
        .collect::<Vec<_>>()
        .join("\n");
        let signals = IdiomUsageAnalyzer.analyze_with_language(&source, Some(Language::Go));
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("interface")),
            "expected interface check Claude signal"
        );
    }

    #[test]
    fn three_string_concatenations_is_human() {
        let source = pad(
            "let a = s1 + \" world\";\nlet b = s2 + \" foo\";\nlet c = s3 + \" bar\";",
            12,
        );
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 1.0
                && s.description.contains("concat")),
            "expected string concatenation Human signal (weight 1.0)"
        );
    }

    // --- JavaScript branch coverage ---

    #[test]
    fn javascript_arrow_functions_only_is_claude() {
        let source: Vec<String> = (0..10)
            .map(|i| format!("const fn{i} = (x) => x + {i};"))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("arrow")),
            "expected arrow functions Claude signal"
        );
    }

    #[test]
    fn javascript_regular_functions_only_is_human() {
        let source: Vec<String> = (0..10)
            .map(|i| format!("function fn{i}(x) {{ return x + {i}; }}"))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.description.contains("traditional")),
            "expected traditional function declarations Human signal"
        );
    }

    #[test]
    fn javascript_const_declarations_is_claude() {
        let source: Vec<String> = (0..12)
            .map(|i| format!("const value{i} = {i};"))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("const")),
            "expected const declarations Claude signal"
        );
    }

    #[test]
    fn javascript_optional_chaining_is_claude() {
        let source: Vec<String> = (0..10)
            .map(|i| format!("const v{i} = obj?.prop{i} ?? defaultVal{i};"))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("null")),
            "expected optional chaining Claude signal"
        );
    }

    #[test]
    fn javascript_destructuring_is_claude() {
        let source: Vec<String> = (0..12).map(|i| {
            if i % 2 == 0 {
                format!("const {{ prop{i} }} = obj{i};")
            } else {
                format!("let [ item{i} ] = arr{i};")
            }
        }).collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("destructuring")),
            "expected destructuring Claude signal"
        );
    }

    #[test]
    fn javascript_async_await_is_claude() {
        let source: Vec<String> = (0..10)
            .map(|i| format!("async function step{i}() {{ const r = await fetch{i}(); return r; }}"))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("async")),
            "expected async/await Claude signal"
        );
    }

    // --- Go branch coverage ---

    #[test]
    fn go_goroutines_is_gpt() {
        let mut lines: Vec<String> = vec![
            "go func() { doWork() }()".into(),
            "go processItem(item)".into(),
        ];
        lines.extend((0..10).map(|i| format!("x{i} := step{i}()")));
        let source = lines.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_go(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Gpt && s.description.contains("goroutine")),
            "expected goroutines Gpt signal"
        );
    }

    #[test]
    fn go_defer_stmts_is_claude() {
        let mut lines: Vec<String> = vec![
            "defer f.Close()".into(),
            "defer mu.Unlock()".into(),
        ];
        lines.extend((0..10).map(|i| format!("x{i} := step{i}()")));
        let source = lines.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_go(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("defer")),
            "expected defer statements Claude signal"
        );
    }

    // --- Python remaining branch coverage ---

    #[test]
    fn python_fstrings_is_claude() {
        let source: Vec<String> = (0..12)
            .map(|i| format!("msg{i} = f\"value is {{val{i}}}\""))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("f-string")),
            "expected f-strings Claude signal"
        );
    }

    #[test]
    fn python_old_format_is_human() {
        let source: Vec<String> = (0..12)
            .map(|i| format!("msg{i} = \"value %d\" % (val{i},)"))
            .collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.description.contains("format")),
            "expected old-style format Human signal"
        );
    }

    #[test]
    fn python_context_managers_is_claude() {
        let mut lines: Vec<String> = vec![
            "with open(path) as f:".into(),
            "    data = f.read()".into(),
            "with lock:".into(),
            "    do_critical()".into(),
        ];
        lines.extend((0..10).map(|i| format!("x{i} = {i}")));
        let source = lines.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("context")),
            "expected context manager Claude signal"
        );
    }

    #[test]
    fn python_type_annotated_functions_is_claude() {
        let source: Vec<String> = (0..12).map(|i| {
            if i < 4 {
                format!("def fn{i}(x: int) -> str:\n    return str(x)")
            } else {
                format!("x{i} = {i}")
            }
        }).collect();
        let source = source.join("\n");
        let signals = IdiomUsageAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.description.contains("annotation")),
            "expected return type annotations Claude signal"
        );
    }
}
