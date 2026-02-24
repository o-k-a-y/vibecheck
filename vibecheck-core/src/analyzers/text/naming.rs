use crate::analyzers::Analyzer;
use crate::heuristics::signal_ids;
use crate::report::{ModelFamily, Signal};

pub struct NamingAnalyzer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::Analyzer;
    use crate::report::ModelFamily;

    fn run(source: &str) -> Vec<Signal> {
        NamingAnalyzer.analyze(source)
    }

    #[test]
    fn short_source_no_signals() {
        let source = (0..5).map(|i| format!("let x{i} = {i};")).collect::<Vec<_>>().join("\n");
        assert!(run(&source).is_empty());
    }

    #[test]
    fn long_variable_names_is_claude() {
        // avg name length > 12: all names are long descriptive identifiers
        let source = "\
let configuration_data = 1;\n\
let processed_result_value = 2;\n\
let transformation_output = 3;\n\
let initialization_state = 4;\n\
let connection_manager = 5;\n\
let error_description = 6;\n\
let request_handler = 7;\n\
let response_buffer = 8;\n\
let authentication_token = 9;\n\
let serialization_context = 10;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected long variable names Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn short_variable_names_is_human() {
        // avg name length < 4: use single/double char names
        let source = "\
let x = 1;\nlet y = 2;\nlet z = 3;\nlet a = 4;\nlet b = 5;\n\
let c = 6;\nlet d = 7;\nlet e = 8;\nlet f = 9;\nlet g = 0;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 1.5),
            "expected short variable names Human signal (weight 1.5)"
        );
    }

    #[test]
    fn three_single_char_names_is_human() {
        let source = "\
let x = 1;\nlet y = 2;\nlet z = 3;\n\
let value_one = 10;\nlet value_two = 20;\n\
let result = 30;\nlet output = 40;\nlet data = 50;\nlet item = 60;\nlet entry = 70;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 2.0),
            "expected 3+ single-char names Human signal (weight 2.0)"
        );
    }

    #[test]
    fn five_vars_no_single_char_is_claude() {
        let source = "\
let value_one = 1;\nlet value_two = 2;\nlet value_three = 3;\n\
let value_four = 4;\nlet value_five = 5;\n\
let extra_one = 6;\nlet extra_two = 7;\nlet extra_three = 8;\nlet extra_four = 9;\nlet extra_five = 0;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.0),
            "expected no single-char names Claude signal (weight 1.0)"
        );
    }
}

impl NamingAnalyzer {
    /// Extract identifier names from Python assignments and definitions.
    fn python_names(lines: &[&str]) -> Vec<String> {
        let mut names = Vec::new();
        for line in lines {
            let t = line.trim();
            // Variable assignments: name = ... or name: type = ...
            if !t.starts_with('#') && !t.starts_with("def ") && !t.starts_with("class ")
                && !t.starts_with("import ") && !t.starts_with("from ")
                && !t.starts_with("return ") && !t.starts_with("if ")
                && !t.starts_with("for ") && !t.starts_with("while ")
            {
                if let Some(name) = t.split([' ', ':', '=']).next().map(|s| s.trim()) {
                    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        names.push(name.to_string());
                    }
                }
            }
            // Function names
            if t.starts_with("def ") || t.starts_with("async def ") {
                let after = if t.starts_with("async def ") { &t[10..] } else { &t[4..] };
                if let Some(name) = after.split('(').next().map(|s| s.trim()) {
                    if !name.is_empty() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        names
    }

    fn analyze_names(
        source_name: &str,
        very_descriptive_id: &str,
        descriptive_id: &str,
        short_names_id: &str,
        many_single_char_id: &str,
        no_single_char_id: &str,
        names: &[String],
    ) -> Vec<Signal> {
        let mut signals = Vec::new();
        if names.is_empty() {
            return signals;
        }
        let avg_len: f64 =
            names.iter().map(|n| n.len() as f64).sum::<f64>() / names.len() as f64;

        if avg_len > 12.0 {
            signals.push(Signal::new(
                very_descriptive_id,
                source_name,
                format!("Very descriptive names (avg {avg_len:.1} chars)"),
                ModelFamily::Claude,
                1.5,
            ));
        } else if avg_len > 8.0 {
            signals.push(Signal::new(
                descriptive_id,
                source_name,
                format!("Descriptive names (avg {avg_len:.1} chars)"),
                ModelFamily::Gpt,
                1.0,
            ));
        } else if avg_len < 4.0 {
            signals.push(Signal::new(
                short_names_id,
                source_name,
                format!("Short names (avg {avg_len:.1} chars)"),
                ModelFamily::Human,
                1.5,
            ));
        }

        let single_char = names.iter().filter(|n| n.len() == 1).count();
        if single_char >= 3 {
            signals.push(Signal::new(
                many_single_char_id,
                source_name,
                format!("{single_char} single-character names"),
                ModelFamily::Human,
                2.0,
            ));
        } else if single_char == 0 && names.len() >= 5 {
            signals.push(Signal::new(
                no_single_char_id,
                source_name,
                "No single-character names",
                ModelFamily::Claude,
                1.0,
            ));
        }

        signals
    }

    fn analyze_python_impl(source: &str) -> Vec<Signal> {
        let lines: Vec<&str> = source.lines().collect();
        if lines.len() < 10 {
            return vec![];
        }
        let names = Self::python_names(&lines);
        Self::analyze_names(
            "naming",
            signal_ids::PYTHON_NAMING_VERY_DESCRIPTIVE,
            signal_ids::PYTHON_NAMING_DESCRIPTIVE,
            signal_ids::PYTHON_NAMING_SHORT_NAMES,
            signal_ids::PYTHON_NAMING_MANY_SINGLE_CHAR,
            signal_ids::PYTHON_NAMING_NO_SINGLE_CHAR,
            &names,
        )
    }

    fn analyze_javascript_impl(source: &str) -> Vec<Signal> {
        let lines: Vec<&str> = source.lines().collect();
        if lines.len() < 10 {
            return vec![];
        }

        // Extract names from const/let/var and function declarations
        let names: Vec<String> = lines
            .iter()
            .filter_map(|l| {
                let t = l.trim();
                let after = if t.starts_with("const ") {
                    Some(&t[6..])
                } else if t.starts_with("let ") {
                    Some(&t[4..])
                } else if t.starts_with("var ") {
                    Some(&t[4..])
                } else if t.starts_with("function ") {
                    Some(&t[9..])
                } else {
                    None
                };
                after.and_then(|s| {
                    s.split(|c: char| c == ' ' || c == '=' || c == '(' || c == ':')
                        .next()
                        .map(|n| n.trim().to_string())
                })
            })
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_'))
            .collect();

        Self::analyze_names(
            "naming",
            signal_ids::JS_NAMING_VERY_DESCRIPTIVE,
            signal_ids::JS_NAMING_DESCRIPTIVE,
            signal_ids::JS_NAMING_SHORT_NAMES,
            signal_ids::JS_NAMING_MANY_SINGLE_CHAR,
            signal_ids::JS_NAMING_NO_SINGLE_CHAR,
            &names,
        )
    }

    fn analyze_go_impl(source: &str) -> Vec<Signal> {
        let lines: Vec<&str> = source.lines().collect();
        if lines.len() < 10 {
            return vec![];
        }

        // Extract names from var, :=, func declarations
        let mut names: Vec<String> = Vec::new();
        for line in &lines {
            let t = line.trim();
            // Short variable declarations: name := ...
            if let Some(pos) = t.find(" := ") {
                let before = &t[..pos];
                // Could be "name, err := ..." — take all identifiers
                for part in before.split(',') {
                    let n = part.trim();
                    if !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        names.push(n.to_string());
                    }
                }
            }
            // func names
            if t.starts_with("func ") {
                let after = &t[5..];
                // Could be "func (r *Receiver) MethodName(" — get the actual name
                let name_part = if after.starts_with('(') {
                    // method: skip receiver
                    after.find(')').and_then(|p| after[p + 1..].trim().split('(').next())
                } else {
                    after.split('(').next()
                };
                if let Some(n) = name_part.map(|s| s.trim()) {
                    if !n.is_empty() {
                        names.push(n.to_string());
                    }
                }
            }
        }

        Self::analyze_names(
            "naming",
            signal_ids::GO_NAMING_VERY_DESCRIPTIVE,
            signal_ids::GO_NAMING_DESCRIPTIVE,
            signal_ids::GO_NAMING_SHORT_NAMES,
            signal_ids::GO_NAMING_MANY_SINGLE_CHAR,
            signal_ids::GO_NAMING_NO_SINGLE_CHAR,
            &names,
        )
    }
}

impl Analyzer for NamingAnalyzer {
    fn name(&self) -> &str {
        "naming"
    }

    fn analyze_python(&self, source: &str) -> Vec<Signal> { Self::analyze_python_impl(source) }
    fn analyze_javascript(&self, source: &str) -> Vec<Signal> { Self::analyze_javascript_impl(source) }
    fn analyze_go(&self, source: &str) -> Vec<Signal> { Self::analyze_go_impl(source) }

    fn analyze(&self, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        if lines.len() < 10 {
            return signals;
        }

        // Extract variable/binding names from let statements
        let let_names: Vec<&str> = lines
            .iter()
            .filter_map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with("let ") || trimmed.starts_with("let mut ") {
                    let after_let = if trimmed.starts_with("let mut ") {
                        &trimmed[8..]
                    } else {
                        &trimmed[4..]
                    };
                    let name = after_let
                        .split(|c: char| c == ':' || c == '=' || c == ' ')
                        .next()
                        .map(|s| s.trim());
                    name
                } else {
                    None
                }
            })
            .collect();

        if let_names.is_empty() {
            return signals;
        }

        // Analyze variable name lengths
        let avg_len: f64 =
            let_names.iter().map(|n| n.len() as f64).sum::<f64>() / let_names.len() as f64;

        if avg_len > 12.0 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_VERY_DESCRIPTIVE_VARS,
                self.name(),
                format!(
                    "Very descriptive variable names (avg {:.1} chars)",
                    avg_len
                ),
                ModelFamily::Claude,
                1.5,
            ));
        } else if avg_len > 8.0 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_DESCRIPTIVE_VARS,
                self.name(),
                format!("Descriptive variable names (avg {:.1} chars)", avg_len),
                ModelFamily::Gpt,
                1.0,
            ));
        } else if avg_len < 4.0 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_SHORT_VARS,
                self.name(),
                format!("Short variable names (avg {:.1} chars)", avg_len),
                ModelFamily::Human,
                1.5,
            ));
        }

        // Single-character variable names
        let single_char = let_names.iter().filter(|n| n.len() == 1).count();
        if single_char >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_MANY_SINGLE_CHAR_VARS,
                self.name(),
                format!("{single_char} single-character variable names"),
                ModelFamily::Human,
                2.0,
            ));
        } else if single_char == 0 && let_names.len() >= 5 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_NO_SINGLE_CHAR_VARS,
                self.name(),
                "No single-character variable names",
                ModelFamily::Claude,
                1.0,
            ));
        }

        // Underscore-prefixed unused bindings (let _foo = ...)
        let underscore_bindings = let_names.iter().filter(|n| n.starts_with('_') && n.len() > 1).count();
        if underscore_bindings >= 2 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_UNDERSCORE_BINDINGS,
                self.name(),
                format!("{underscore_bindings} underscore-prefixed bindings (acknowledging unused)"),
                ModelFamily::Human,
                1.0,
            ));
        }

        // Function name analysis
        let fn_names: Vec<&str> = lines
            .iter()
            .filter_map(|l| {
                let trimmed = l.trim();
                let after_fn = if trimmed.starts_with("pub fn ") {
                    Some(&trimmed[7..])
                } else if trimmed.starts_with("fn ") {
                    Some(&trimmed[3..])
                } else {
                    None
                };
                after_fn.and_then(|s| s.split('(').next()).map(|s| s.trim())
            })
            .collect();

        let avg_fn_len: f64 = if fn_names.is_empty() {
            0.0
        } else {
            fn_names.iter().map(|n| n.len() as f64).sum::<f64>() / fn_names.len() as f64
        };

        if avg_fn_len > 15.0 && fn_names.len() >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_NAMING_DESCRIPTIVE_FN_NAMES,
                self.name(),
                format!(
                    "Very descriptive function names (avg {:.1} chars)",
                    avg_fn_len
                ),
                ModelFamily::Claude,
                1.0,
            ));
        }

        signals
    }
}
