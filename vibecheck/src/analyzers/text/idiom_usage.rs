use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct IdiomUsageAnalyzer;

impl Analyzer for IdiomUsageAnalyzer {
    fn name(&self) -> &str {
        "idioms"
    }

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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{iterator_count} iterator chain usages — textbook-idiomatic Rust"),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{builder_chain} method chain continuation lines — builder pattern"),
                family: ModelFamily::Gpt,
                weight: 1.0,
            });
        }

        // impl Display / impl std::fmt::Display
        if source.contains("impl std::fmt::Display") || source.contains("impl Display for") {
            signals.push(Signal {
                source: self.name().into(),
                description: "Implements Display trait — thorough API design".into(),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // From/Into implementations
        let from_impl = lines
            .iter()
            .filter(|l| l.contains("impl From<") || l.contains("impl Into<"))
            .count();
        if from_impl >= 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{from_impl} From/Into implementations — conversion-rich design"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // Self:: usage in impl blocks (textbook Rust)
        let self_usage = lines
            .iter()
            .filter(|l| l.contains("Self::") || l.contains("Self {"))
            .count();
        if self_usage >= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{self_usage} uses of Self — consistent self-referencing"),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{pattern_match_count} if-let/while-let patterns"),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: "Uses format!() exclusively, no string concatenation".into(),
                family: ModelFamily::Claude,
                weight: 0.8,
            });
        } else if string_concat >= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{string_concat} string concatenations — less idiomatic"),
                family: ModelFamily::Human,
                weight: 1.0,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{trait_count} trait definitions — heavy abstraction"),
                family: ModelFamily::Gpt,
                weight: 1.5,
            });
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
}
