use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use vibecheck_core::report::FamilyId;

use crate::features::FeatureVector;

/// A single classification algorithm that maps a feature vector to a
/// probability distribution over families.
pub trait Classifier: Send + Sync {
    fn predict(&self, features: &FeatureVector) -> HashMap<FamilyId, f64>;
    fn name(&self) -> &str;
}

/// Metadata shipped alongside a trained model artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub version: String,
    pub trained_at: String,
    pub algorithms: Vec<String>,
    pub training_samples: usize,
    pub feature_dimensions: usize,
    pub coverage: HashMap<String, HashMap<String, usize>>,
    pub accuracy: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ConstantClassifier {
        family: FamilyId,
    }

    impl Classifier for ConstantClassifier {
        fn predict(&self, _features: &FeatureVector) -> HashMap<FamilyId, f64> {
            let mut m = HashMap::new();
            m.insert(self.family.clone(), 1.0);
            m
        }

        fn name(&self) -> &str {
            "constant"
        }
    }

    #[test]
    fn constant_classifier_returns_single_family() {
        let clf = ConstantClassifier {
            family: FamilyId("claude".into()),
        };
        let fv = FeatureVector::empty("rust", 100);
        let result = clf.predict(&fv);
        assert_eq!(result.len(), 1);
        assert_eq!(result[&FamilyId("claude".into())], 1.0);
    }

    #[test]
    fn classifier_name_returns_expected_string() {
        let clf = ConstantClassifier {
            family: FamilyId("x".into()),
        };
        assert_eq!(clf.name(), "constant");
    }

    #[test]
    fn model_metadata_with_none_accuracy() {
        let meta = ModelMetadata {
            version: "0.2.0".into(),
            trained_at: "2026-02-27".into(),
            algorithms: vec![],
            training_samples: 0,
            feature_dimensions: 0,
            coverage: HashMap::new(),
            accuracy: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: ModelMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.accuracy, None);
        assert!(back.algorithms.is_empty());
    }

    #[test]
    fn model_metadata_roundtrip() {
        let meta = ModelMetadata {
            version: "0.1.0".into(),
            trained_at: "2026-02-27T00:00:00Z".into(),
            algorithms: vec!["logreg".into()],
            training_samples: 1000,
            feature_dimensions: 278,
            coverage: HashMap::new(),
            accuracy: Some(0.75),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: ModelMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, "0.1.0");
        assert_eq!(back.feature_dimensions, 278);
        assert_eq!(back.accuracy, Some(0.75));
    }
}
