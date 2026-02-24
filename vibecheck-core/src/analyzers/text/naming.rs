use crate::analyzers::Analyzer;
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

impl Analyzer for NamingAnalyzer {
    fn name(&self) -> &str {
        "naming"
    }

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
            signals.push(Signal {
                source: self.name().into(),
                description: format!(
                    "Very descriptive variable names (avg {:.1} chars)",
                    avg_len
                ),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
        } else if avg_len > 8.0 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("Descriptive variable names (avg {:.1} chars)", avg_len),
                family: ModelFamily::Gpt,
                weight: 1.0,
            });
        } else if avg_len < 4.0 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("Short variable names (avg {:.1} chars)", avg_len),
                family: ModelFamily::Human,
                weight: 1.5,
            });
        }

        // Single-character variable names
        let single_char = let_names.iter().filter(|n| n.len() == 1).count();
        if single_char >= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{single_char} single-character variable names"),
                family: ModelFamily::Human,
                weight: 2.0,
            });
        } else if single_char == 0 && let_names.len() >= 5 {
            signals.push(Signal {
                source: self.name().into(),
                description: "No single-character variable names".into(),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // Underscore-prefixed unused bindings (let _foo = ...)
        let underscore_bindings = let_names.iter().filter(|n| n.starts_with('_') && n.len() > 1).count();
        if underscore_bindings >= 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{underscore_bindings} underscore-prefixed bindings (acknowledging unused)"),
                family: ModelFamily::Human,
                weight: 1.0,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: format!(
                    "Very descriptive function names (avg {:.1} chars)",
                    avg_fn_len
                ),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        signals
    }
}
