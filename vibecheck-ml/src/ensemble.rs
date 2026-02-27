use std::collections::HashMap;

use linfa::prelude::*;
use ndarray::{Array1, Array2};
use vibecheck_core::language::Language;
use vibecheck_core::pipeline::PostScorer;
use vibecheck_core::report::{Attribution, FamilyId, ModelFamily, Signal};

use crate::classifier::Classifier;
use crate::features::FeatureVector;
use crate::training::{build_dataset, LabelEncoder};

// ---------------------------------------------------------------------------
// Linfa-backed classifier wrappers
// ---------------------------------------------------------------------------

/// Multinomial logistic regression classifier.
pub struct LogisticRegressionClassifier {
    model: linfa_logistic::MultiFittedLogisticRegression<f64, usize>,
    schema: Vec<String>,
    encoder: LabelEncoder,
}

impl LogisticRegressionClassifier {
    pub fn train(
        vectors: &[FeatureVector],
        labels: &[FamilyId],
        max_iterations: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (x, y, schema, encoder) = build_dataset(vectors, labels);
        let dataset = Dataset::new(x, y);
        let model = linfa_logistic::MultiLogisticRegression::default()
            .max_iterations(max_iterations)
            .fit(&dataset)?;
        Ok(Self {
            model,
            schema,
            encoder,
        })
    }

    fn vectorize(&self, features: &FeatureVector) -> Array2<f64> {
        vectorize_single(features, &self.schema)
    }
}

impl Classifier for LogisticRegressionClassifier {
    fn predict(&self, features: &FeatureVector) -> HashMap<FamilyId, f64> {
        let x = self.vectorize(features);
        let pred: Array1<usize> = self.model.predict(&x);
        let predicted_class = pred[0];
        class_to_distribution(predicted_class, &self.encoder)
    }

    fn name(&self) -> &str {
        "logistic_regression"
    }
}

/// Gaussian Naive Bayes classifier.
pub struct NaiveBayesClassifier {
    model: linfa_bayes::MultinomialNb<f64, usize>,
    schema: Vec<String>,
    encoder: LabelEncoder,
}

impl NaiveBayesClassifier {
    pub fn train(
        vectors: &[FeatureVector],
        labels: &[FamilyId],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (x, y, schema, encoder) = build_dataset(vectors, labels);
        // Shift features to non-negative for MultinomialNB
        let x = shift_non_negative(x);
        let dataset = Dataset::new(x, y);
        let model = linfa_bayes::MultinomialNbParams::new()
            .fit(&dataset)?;
        Ok(Self {
            model,
            schema,
            encoder,
        })
    }

    fn vectorize(&self, features: &FeatureVector) -> Array2<f64> {
        let x = vectorize_single(features, &self.schema);
        shift_non_negative(x)
    }
}

impl Classifier for NaiveBayesClassifier {
    fn predict(&self, features: &FeatureVector) -> HashMap<FamilyId, f64> {
        let x = self.vectorize(features);
        let pred: Array1<usize> = self.model.predict(&x);
        let predicted_class = pred[0];
        class_to_distribution(predicted_class, &self.encoder)
    }

    fn name(&self) -> &str {
        "naive_bayes"
    }
}

/// Decision tree classifier.
pub struct DecisionTreeClassifier {
    model: linfa_trees::DecisionTree<f64, usize>,
    schema: Vec<String>,
    encoder: LabelEncoder,
}

impl DecisionTreeClassifier {
    pub fn train(
        vectors: &[FeatureVector],
        labels: &[FamilyId],
        max_depth: Option<usize>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (x, y, schema, encoder) = build_dataset(vectors, labels);
        let dataset = Dataset::new(x, y);
        let mut params = linfa_trees::DecisionTree::params();
        if let Some(d) = max_depth {
            params = params.max_depth(Some(d));
        }
        let model = params.fit(&dataset)?;
        Ok(Self {
            model,
            schema,
            encoder,
        })
    }

    fn vectorize(&self, features: &FeatureVector) -> Array2<f64> {
        vectorize_single(features, &self.schema)
    }
}

impl Classifier for DecisionTreeClassifier {
    fn predict(&self, features: &FeatureVector) -> HashMap<FamilyId, f64> {
        let x = self.vectorize(features);
        let pred: Array1<usize> = self.model.predict(&x);
        let predicted_class = pred[0];
        class_to_distribution(predicted_class, &self.encoder)
    }

    fn name(&self) -> &str {
        "decision_tree"
    }
}

// ---------------------------------------------------------------------------
// EnsembleModel — weighted average of classifiers, implements PostScorer
// ---------------------------------------------------------------------------

/// Ensemble classifier that combines multiple algorithms via weighted averaging.
///
/// Each algorithm produces a probability distribution over families.
/// The ensemble linearly combines them using per-algorithm weights,
/// then normalizes to produce a final distribution.
pub struct EnsembleModel {
    classifiers: Vec<(f64, Box<dyn Classifier>)>,
}

impl EnsembleModel {
    pub fn new() -> Self {
        Self {
            classifiers: Vec::new(),
        }
    }

    pub fn add(&mut self, weight: f64, classifier: Box<dyn Classifier>) {
        self.classifiers.push((weight, classifier));
    }

    pub fn algorithm_count(&self) -> usize {
        self.classifiers.len()
    }

    pub fn algorithm_names(&self) -> Vec<&str> {
        self.classifiers.iter().map(|(_, c)| c.name()).collect()
    }
}

impl Default for EnsembleModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier for EnsembleModel {
    fn predict(&self, features: &FeatureVector) -> HashMap<FamilyId, f64> {
        if self.classifiers.is_empty() {
            return HashMap::new();
        }

        let mut combined: HashMap<FamilyId, f64> = HashMap::new();
        let mut total_weight = 0.0;

        for (weight, clf) in &self.classifiers {
            let pred = clf.predict(features);
            for (family, score) in pred {
                *combined.entry(family).or_insert(0.0) += weight * score;
            }
            total_weight += weight;
        }

        if total_weight > 0.0 {
            for v in combined.values_mut() {
                *v /= total_weight;
            }
        }

        combined
    }

    fn name(&self) -> &str {
        "ensemble"
    }
}

impl PostScorer for EnsembleModel {
    fn rescore(
        &self,
        signals: &[Signal],
        metrics: &HashMap<String, f64>,
        _heuristic_attribution: &Attribution,
        language: Option<Language>,
        source: &str,
    ) -> Attribution {
        let lang_str = language
            .map(|l| match l {
                Language::Rust => "rust",
                Language::Python => "python",
                Language::JavaScript => "javascript",
                Language::Go => "go",
            })
            .unwrap_or("unknown");

        let fv = crate::features::extract_features(
            signals,
            metrics,
            lang_str,
            source.lines().count(),
        );

        let scores_fid = self.predict(&fv);

        let mut scores: HashMap<ModelFamily, f64> = HashMap::new();
        for family in ModelFamily::all() {
            let fid = FamilyId::from_model_family(*family);
            let score = scores_fid.get(&fid).copied().unwrap_or(0.0);
            scores.insert(*family, score);
        }

        // Handle unknown families: add their mass to the highest-scoring known family
        let known_total: f64 = scores.values().sum();
        if known_total < 1.0 {
            let residual = 1.0 - known_total;
            if let Some((best, _)) = scores.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()) {
                let best = *best;
                *scores.entry(best).or_insert(0.0) += residual;
            }
        }

        // Normalize
        let total: f64 = scores.values().sum();
        if total > 0.0 {
            for v in scores.values_mut() {
                *v /= total;
            }
        }

        let (primary, confidence) = scores
            .iter()
            .max_by(|a, b| {
                a.1.partial_cmp(b.1)
                    .unwrap()
                    .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
            })
            .map(|(&k, &v)| (k, v))
            .unwrap_or((ModelFamily::Human, 0.0));

        Attribution {
            primary,
            confidence,
            scores,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a single FeatureVector to a 1×n_features ndarray matrix.
fn vectorize_single(features: &FeatureVector, schema: &[String]) -> Array2<f64> {
    let mut row = vec![0.0f64; schema.len()];
    for (j, name) in schema.iter().enumerate() {
        let value = if let Some(metric_name) = name.strip_prefix("metric:") {
            features
                .metric_features
                .get(metric_name)
                .copied()
                .unwrap_or(0.0)
        } else {
            features
                .signal_features
                .get(name)
                .copied()
                .unwrap_or(0.0)
        };
        row[j] = value;
    }
    Array2::from_shape_vec((1, schema.len()), row).unwrap()
}

/// Convert a single predicted class to a probability distribution.
///
/// Assigns 1.0 to the predicted class, 0.0 to all others.
/// (Linfa classifiers generally don't expose class probabilities
/// natively, so we use hard predictions.)
fn class_to_distribution(class: usize, encoder: &LabelEncoder) -> HashMap<FamilyId, f64> {
    let mut dist = HashMap::new();
    for i in 0..encoder.n_classes() {
        let fid = encoder.decode(i).cloned().unwrap_or(FamilyId("unknown".into()));
        dist.insert(fid, if i == class { 1.0 } else { 0.0 });
    }
    dist
}

/// Shift all values in the feature matrix so the minimum is 0.
/// Required for MultinomialNB which expects non-negative features.
fn shift_non_negative(mut x: Array2<f64>) -> Array2<f64> {
    let min_val = x.iter().copied().fold(f64::INFINITY, f64::min);
    if min_val < 0.0 {
        x.mapv_inplace(|v| v - min_val);
    }
    x
}

/// Evaluate accuracy of a classifier on held-out data.
pub fn evaluate_accuracy(
    classifier: &dyn Classifier,
    test_vectors: &[FeatureVector],
    test_labels: &[FamilyId],
) -> f64 {
    if test_vectors.is_empty() {
        return 0.0;
    }
    let correct = test_vectors
        .iter()
        .zip(test_labels.iter())
        .filter(|(fv, expected)| {
            let pred = classifier.predict(fv);
            pred.iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(fid, _)| fid == *expected)
                .unwrap_or(false)
        })
        .count();
    correct as f64 / test_vectors.len() as f64
}

/// Train a full ensemble from feature vectors and labels.
///
/// Trains logistic regression, naive bayes, and decision tree classifiers,
/// assigns equal weight to each, and returns the combined ensemble.
pub fn train_default_ensemble(
    vectors: &[FeatureVector],
    labels: &[FamilyId],
) -> Result<EnsembleModel, Box<dyn std::error::Error + Send + Sync>> {
    let mut ensemble = EnsembleModel::new();

    if let Ok(lr) = LogisticRegressionClassifier::train(vectors, labels, 100) {
        ensemble.add(1.0, Box::new(lr));
    }

    if let Ok(nb) = NaiveBayesClassifier::train(vectors, labels) {
        ensemble.add(1.0, Box::new(nb));
    }

    if let Ok(dt) = DecisionTreeClassifier::train(vectors, labels, Some(10)) {
        ensemble.add(1.0, Box::new(dt));
    }

    Ok(ensemble)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fv(signal_val: f64, metric_val: f64) -> FeatureVector {
        let mut fv = FeatureVector::empty("rust", 100);
        fv.signal_features.insert("s1".into(), signal_val);
        fv.signal_features.insert("s2".into(), 1.0 - signal_val);
        fv.metric_features.insert("m1".into(), metric_val);
        fv
    }

    fn fid(s: &str) -> FamilyId {
        FamilyId(s.into())
    }

    fn training_data() -> (Vec<FeatureVector>, Vec<FamilyId>) {
        let mut vectors = Vec::new();
        let mut labels = Vec::new();

        // Claude-like: high s1, high metric
        for i in 0..10 {
            vectors.push(make_fv(0.8 + (i as f64) * 0.01, 5.0 + i as f64));
            labels.push(fid("claude"));
        }
        // GPT-like: low s1, low metric
        for i in 0..10 {
            vectors.push(make_fv(0.1 + (i as f64) * 0.01, 1.0 + i as f64 * 0.1));
            labels.push(fid("gpt"));
        }
        // Human-like: medium s1, medium metric
        for i in 0..10 {
            vectors.push(make_fv(0.4 + (i as f64) * 0.01, 3.0 + i as f64 * 0.2));
            labels.push(fid("human"));
        }

        (vectors, labels)
    }

    #[test]
    fn logistic_regression_trains_and_predicts() {
        let (vectors, labels) = training_data();
        let clf = LogisticRegressionClassifier::train(&vectors, &labels, 100).unwrap();
        let pred = clf.predict(&make_fv(0.85, 8.0));
        assert!(!pred.is_empty());
        let total: f64 = pred.values().sum();
        assert!((total - 1.0).abs() < 1e-6, "should sum to 1.0, got {total}");
    }

    #[test]
    fn naive_bayes_trains_and_predicts() {
        let (vectors, labels) = training_data();
        let clf = NaiveBayesClassifier::train(&vectors, &labels).unwrap();
        let pred = clf.predict(&make_fv(0.15, 1.0));
        assert!(!pred.is_empty());
    }

    #[test]
    fn decision_tree_trains_and_predicts() {
        let (vectors, labels) = training_data();
        let clf = DecisionTreeClassifier::train(&vectors, &labels, Some(5)).unwrap();
        let pred = clf.predict(&make_fv(0.45, 3.0));
        assert!(!pred.is_empty());
    }

    #[test]
    fn ensemble_combines_classifiers() {
        let (vectors, labels) = training_data();
        let ensemble = train_default_ensemble(&vectors, &labels).unwrap();
        assert!(ensemble.algorithm_count() >= 1);

        let pred = ensemble.predict(&make_fv(0.85, 8.0));
        assert!(!pred.is_empty());
        let total: f64 = pred.values().sum();
        assert!(
            (total - 1.0).abs() < 1e-6,
            "ensemble should sum to ~1.0; got {total}"
        );
    }

    #[test]
    fn ensemble_as_post_scorer() {
        let (vectors, labels) = training_data();
        let ensemble = train_default_ensemble(&vectors, &labels).unwrap();

        let heuristic = Attribution {
            primary: ModelFamily::Human,
            confidence: 0.5,
            scores: ModelFamily::all()
                .iter()
                .map(|&f| (f, 0.2))
                .collect(),
        };

        let result = ensemble.rescore(&[], &HashMap::new(), &heuristic, None, "fn main() {}");
        assert!(result.confidence > 0.0);
        let total: f64 = result.scores.values().sum();
        assert!(
            (total - 1.0).abs() < 1e-6,
            "PostScorer result should sum to ~1.0; got {total}"
        );
    }

    #[test]
    fn evaluate_accuracy_computes_correct_ratio() {
        let (vectors, labels) = training_data();
        let ensemble = train_default_ensemble(&vectors, &labels).unwrap();
        let acc = evaluate_accuracy(&ensemble, &vectors, &labels);
        assert!(
            acc > 0.2,
            "accuracy on training data should be above chance; got {acc}"
        );
    }

    #[test]
    fn empty_ensemble_returns_empty_prediction() {
        let ensemble = EnsembleModel::new();
        let fv = FeatureVector::empty("rust", 10);
        let pred = ensemble.predict(&fv);
        assert!(pred.is_empty());
    }

    #[test]
    fn vectorize_single_correct_shape() {
        let fv = make_fv(1.0, 2.0);
        let schema = vec!["m1".into(), "s1".into(), "s2".into()];
        let x = vectorize_single(&fv, &schema);
        assert_eq!(x.shape(), &[1, 3]);
    }

    #[test]
    fn classifier_name_methods() {
        let (vectors, labels) = training_data();
        let lr = LogisticRegressionClassifier::train(&vectors, &labels, 100).unwrap();
        assert_eq!(lr.name(), "logistic_regression");

        let nb = NaiveBayesClassifier::train(&vectors, &labels).unwrap();
        assert_eq!(nb.name(), "naive_bayes");

        let dt = DecisionTreeClassifier::train(&vectors, &labels, Some(5)).unwrap();
        assert_eq!(dt.name(), "decision_tree");
    }

    #[test]
    fn ensemble_name_and_algorithm_names() {
        let (vectors, labels) = training_data();
        let ensemble = train_default_ensemble(&vectors, &labels).unwrap();
        assert_eq!(ensemble.name(), "ensemble");
        let names = ensemble.algorithm_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"logistic_regression"));
        assert!(names.contains(&"naive_bayes"));
        assert!(names.contains(&"decision_tree"));
    }

    #[test]
    fn ensemble_default_is_empty() {
        let ensemble = EnsembleModel::default();
        assert_eq!(ensemble.algorithm_count(), 0);
    }

    #[test]
    fn post_scorer_with_known_language() {
        let (vectors, labels) = training_data();
        let ensemble = train_default_ensemble(&vectors, &labels).unwrap();

        let heuristic = Attribution {
            primary: ModelFamily::Human,
            confidence: 0.5,
            scores: ModelFamily::all().iter().map(|&f| (f, 0.2)).collect(),
        };

        for lang in [Language::Rust, Language::Python, Language::JavaScript, Language::Go] {
            let result = ensemble.rescore(
                &[],
                &HashMap::new(),
                &heuristic,
                Some(lang),
                "fn main() {}",
            );
            let total: f64 = result.scores.values().sum();
            assert!(
                (total - 1.0).abs() < 1e-6,
                "PostScorer with {lang:?} should normalize to ~1.0; got {total}"
            );
        }
    }

    #[test]
    fn shift_non_negative_with_negative_values() {
        let x = Array2::from_shape_vec((2, 2), vec![-3.0, 1.0, -1.0, 5.0]).unwrap();
        let shifted = shift_non_negative(x);
        let min_val = shifted.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(min_val >= 0.0, "all values should be non-negative; min={min_val}");
        // Original min was -3.0, so shifted: [0, 4, 2, 8]
        assert_eq!(shifted[[0, 0]], 0.0);
        assert_eq!(shifted[[0, 1]], 4.0);
    }

    #[test]
    fn evaluate_accuracy_empty_returns_zero() {
        let ensemble = EnsembleModel::new();
        let acc = evaluate_accuracy(&ensemble, &[], &[]);
        assert_eq!(acc, 0.0);
    }
}
