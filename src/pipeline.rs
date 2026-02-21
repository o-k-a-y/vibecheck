use std::collections::HashMap;
use std::path::PathBuf;

use crate::analyzers::{default_analyzers, Analyzer};
use crate::report::{Attribution, ModelFamily, Report, ReportMetadata, Signal};

/// Orchestrates analyzers and aggregates their signals into a report.
pub struct Pipeline {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl Pipeline {
    pub fn new(analyzers: Vec<Box<dyn Analyzer>>) -> Self {
        Self { analyzers }
    }

    pub fn with_defaults() -> Self {
        Self::new(default_analyzers())
    }

    pub fn run(&self, source: &str, file_path: Option<PathBuf>) -> Report {
        let signals: Vec<Signal> = self
            .analyzers
            .iter()
            .flat_map(|a| a.analyze(source))
            .collect();

        let attribution = self.aggregate(&signals);
        let lines_of_code = source.lines().count();
        let signal_count = signals.len();

        Report {
            attribution,
            signals,
            metadata: ReportMetadata {
                file_path,
                lines_of_code,
                signal_count,
            },
        }
    }

    fn aggregate(&self, signals: &[Signal]) -> Attribution {
        let mut raw_scores: HashMap<ModelFamily, f64> = HashMap::new();
        for family in ModelFamily::all() {
            raw_scores.insert(*family, 0.0);
        }

        for signal in signals {
            *raw_scores.entry(signal.family).or_insert(0.0) += signal.weight;
        }

        // Shift all scores so the minimum is 0
        let min_score = raw_scores.values().cloned().fold(f64::INFINITY, f64::min);
        let mut shifted: HashMap<ModelFamily, f64> = raw_scores
            .iter()
            .map(|(&k, &v)| (k, (v - min_score).max(0.0)))
            .collect();

        // Normalize to a distribution summing to 1.0
        let total: f64 = shifted.values().sum();
        if total > 0.0 {
            for v in shifted.values_mut() {
                *v /= total;
            }
        } else {
            // No signals at all — uniform distribution
            let uniform = 1.0 / ModelFamily::all().len() as f64;
            for v in shifted.values_mut() {
                *v = uniform;
            }
        }

        let (primary, confidence) = shifted
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(&k, &v)| (k, v))
            .unwrap();

        Attribution {
            primary,
            confidence,
            scores: shifted,
        }
    }
}
