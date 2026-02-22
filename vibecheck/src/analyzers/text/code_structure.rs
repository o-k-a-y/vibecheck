use crate::analyzers::Analyzer;
use crate::report::{ModelFamily, Signal};

pub struct CodeStructureAnalyzer;

impl Analyzer for CodeStructureAnalyzer {
    fn name(&self) -> &str {
        "structure"
    }

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
                signals.push(Signal {
                    source: self.name().into(),
                    description: format!(
                        "Explicit type annotations on {:.0}% of let bindings",
                        annotation_ratio * 100.0
                    ),
                    family: ModelFamily::Gpt,
                    weight: 1.0,
                });
            } else if annotation_ratio < 0.2 && let_lines.len() >= 5 {
                signals.push(Signal {
                    source: self.name().into(),
                    description: "Relies on type inference — minimal annotations".into(),
                    family: ModelFamily::Claude,
                    weight: 0.8,
                });
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
                signals.push(Signal {
                    source: self.name().into(),
                    description: "Import statements are alphabetically sorted".into(),
                    family: ModelFamily::Claude,
                    weight: 1.0,
                });
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
                signals.push(Signal {
                    source: self.name().into(),
                    description: "Perfectly consistent blank line spacing".into(),
                    family: ModelFamily::Claude,
                    weight: 1.0,
                });
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
                signals.push(Signal {
                    source: self.name().into(),
                    description: "All lines under 100 chars — disciplined formatting".into(),
                    family: ModelFamily::Claude,
                    weight: 0.8,
                });
            } else if over_100 >= 5 {
                signals.push(Signal {
                    source: self.name().into(),
                    description: format!("{over_100} lines over 100 chars"),
                    family: ModelFamily::Human,
                    weight: 1.0,
                });
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
                signals.push(Signal {
                    source: self.name().into(),
                    description: format!(
                        "Heavy derive usage (avg {:.1} traits per derive)",
                        avg_derives
                    ),
                    family: ModelFamily::Claude,
                    weight: 1.0,
                });
            }
        }

        signals
    }
}
