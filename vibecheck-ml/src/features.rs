use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use vibecheck_core::heuristics::all_heuristics;
use vibecheck_core::report::Signal;

/// Numeric feature vector extracted from a source file's analysis.
///
/// Dimensions:
/// - `signal_features`: one per heuristic signal ID (fired weight or 0.0)
/// - `metric_features`: raw CST metric values
/// - `language`: source language string
/// - `lines_of_code`: file length
///
/// This is the input to all [`crate::classifier::Classifier`] implementations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub signal_features: HashMap<String, f64>,
    pub metric_features: HashMap<String, f64>,
    pub language: String,
    pub lines_of_code: usize,
}

impl FeatureVector {
    pub fn empty(language: &str, lines_of_code: usize) -> Self {
        Self {
            signal_features: HashMap::new(),
            metric_features: HashMap::new(),
            language: language.into(),
            lines_of_code,
        }
    }

    /// Total feature dimensions (signal + metric).
    pub fn dimensions(&self) -> usize {
        self.signal_features.len() + self.metric_features.len()
    }

    /// Flatten into a deterministic `(names, values)` pair sorted by feature name.
    ///
    /// Useful for feeding into linear algebra routines that need a flat `Vec<f64>`.
    pub fn to_flat(&self) -> (Vec<String>, Vec<f64>) {
        let mut pairs: Vec<(String, f64)> = self
            .signal_features
            .iter()
            .chain(self.metric_features.iter())
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs.into_iter().unzip()
    }
}

/// Extract a feature vector from fired signals and raw CST metrics.
///
/// Every signal ID known to `all_heuristics()` gets a dimension.  Fired
/// signals contribute their weight; unfired signals contribute 0.0.  Raw
/// metric values are passed through as-is.
pub fn extract_features(
    signals: &[Signal],
    metrics: &HashMap<String, f64>,
    language: &str,
    lines_of_code: usize,
) -> FeatureVector {
    let mut signal_features: HashMap<String, f64> = HashMap::new();

    for spec in all_heuristics() {
        signal_features.insert(spec.id.to_string(), 0.0);
    }

    for signal in signals {
        if !signal.id.is_empty() {
            signal_features.insert(signal.id.clone(), signal.weight);
        }
    }

    FeatureVector {
        signal_features,
        metric_features: metrics.clone(),
        language: language.into(),
        lines_of_code,
    }
}

/// Build the canonical schema: ordered list of all feature names.
///
/// Signal IDs first (sorted), then metric names (sorted).  This defines
/// the column order for training matrices.
pub fn feature_schema(metric_names: &[String]) -> Vec<String> {
    let mut names: Vec<String> = all_heuristics()
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    names.sort();
    let mut metrics_sorted: Vec<String> = metric_names.to_vec();
    metrics_sorted.sort();
    names.extend(metrics_sorted.into_iter().map(|m| format!("metric:{m}")));
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibecheck_core::report::ModelFamily;

    fn make_signal(id: &str, weight: f64) -> Signal {
        Signal::new(id, "test", "test signal", ModelFamily::Claude, weight)
    }

    #[test]
    fn extract_features_includes_all_heuristic_ids() {
        let fv = extract_features(&[], &HashMap::new(), "rust", 100);
        let total_signals = all_heuristics().len();
        assert_eq!(
            fv.signal_features.len(),
            total_signals,
            "every signal ID should have a dimension"
        );
        assert!(
            fv.signal_features.values().all(|&v| v == 0.0),
            "unfired signals should be 0.0"
        );
    }

    #[test]
    fn extract_features_fired_signal_has_weight() {
        let first_id = all_heuristics()[0].id;
        let signals = vec![make_signal(first_id, 1.5)];
        let fv = extract_features(&signals, &HashMap::new(), "rust", 50);
        assert_eq!(fv.signal_features[first_id], 1.5);
    }

    #[test]
    fn extract_features_passes_metrics_through() {
        let mut metrics = HashMap::new();
        metrics.insert("fn_count".into(), 5.0);
        metrics.insert("avg_fn_len".into(), 12.3);
        let fv = extract_features(&[], &metrics, "python", 200);
        assert_eq!(fv.metric_features["fn_count"], 5.0);
        assert_eq!(fv.metric_features["avg_fn_len"], 12.3);
    }

    #[test]
    fn extract_features_language_and_loc() {
        let fv = extract_features(&[], &HashMap::new(), "go", 42);
        assert_eq!(fv.language, "go");
        assert_eq!(fv.lines_of_code, 42);
    }

    #[test]
    fn to_flat_is_deterministic() {
        let mut metrics = HashMap::new();
        metrics.insert("b_metric".into(), 2.0);
        metrics.insert("a_metric".into(), 1.0);
        let fv = extract_features(&[], &metrics, "rust", 10);
        let (names1, vals1) = fv.to_flat();
        let (names2, vals2) = fv.to_flat();
        assert_eq!(names1, names2);
        assert_eq!(vals1, vals2);
        let metric_start = names1.iter().position(|n| n.starts_with("a_metric") || n.starts_with("b_metric"));
        assert!(metric_start.is_some(), "metrics should appear in flat vector");
    }

    #[test]
    fn feature_vector_json_roundtrip() {
        let signals = vec![make_signal(all_heuristics()[0].id, 1.2)];
        let mut metrics = HashMap::new();
        metrics.insert("complexity".into(), 3.5);
        let fv = extract_features(&signals, &metrics, "rust", 100);

        let json = serde_json::to_string(&fv).unwrap();
        let back: FeatureVector = serde_json::from_str(&json).unwrap();
        assert_eq!(back.language, "rust");
        assert_eq!(back.lines_of_code, 100);
        assert_eq!(
            back.signal_features[all_heuristics()[0].id],
            1.2
        );
        assert_eq!(back.metric_features["complexity"], 3.5);
    }

    #[test]
    fn dimensions_counts_both_signal_and_metric() {
        let mut metrics = HashMap::new();
        metrics.insert("m1".into(), 1.0);
        metrics.insert("m2".into(), 2.0);
        let fv = extract_features(&[], &metrics, "rust", 10);
        assert_eq!(fv.dimensions(), all_heuristics().len() + 2);
    }

    #[test]
    fn feature_schema_is_sorted() {
        let metric_names = vec!["z_metric".into(), "a_metric".into()];
        let schema = feature_schema(&metric_names);
        let signal_count = all_heuristics().len();
        assert!(schema.len() >= signal_count + 2);
        for i in 1..signal_count {
            assert!(schema[i - 1] <= schema[i], "signal IDs should be sorted");
        }
        assert_eq!(schema[signal_count], "metric:a_metric");
        assert_eq!(schema[signal_count + 1], "metric:z_metric");
    }

    #[test]
    fn empty_constructor() {
        let fv = FeatureVector::empty("js", 50);
        assert!(fv.signal_features.is_empty());
        assert!(fv.metric_features.is_empty());
        assert_eq!(fv.language, "js");
        assert_eq!(fv.lines_of_code, 50);
    }
}
