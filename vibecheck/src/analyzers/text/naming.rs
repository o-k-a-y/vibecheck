use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct NamingAnalyzer;

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
