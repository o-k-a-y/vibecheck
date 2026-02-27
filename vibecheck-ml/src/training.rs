use std::collections::HashMap;

use ndarray::{Array1, Array2};
use vibecheck_core::report::FamilyId;

use crate::features::FeatureVector;

/// Label encoder: bidirectional FamilyId ↔ usize mapping.
#[derive(Debug, Clone)]
pub struct LabelEncoder {
    families: Vec<FamilyId>,
    index: HashMap<FamilyId, usize>,
}

impl LabelEncoder {
    pub fn fit(labels: &[FamilyId]) -> Self {
        let mut seen = Vec::new();
        for l in labels {
            if !seen.contains(l) {
                seen.push(l.clone());
            }
        }
        seen.sort_by(|a, b| a.0.cmp(&b.0));
        let index: HashMap<FamilyId, usize> = seen
            .iter()
            .enumerate()
            .map(|(i, f)| (f.clone(), i))
            .collect();
        Self {
            families: seen,
            index,
        }
    }

    pub fn encode(&self, label: &FamilyId) -> Option<usize> {
        self.index.get(label).copied()
    }

    pub fn decode(&self, idx: usize) -> Option<&FamilyId> {
        self.families.get(idx)
    }

    pub fn n_classes(&self) -> usize {
        self.families.len()
    }

    pub fn families(&self) -> &[FamilyId] {
        &self.families
    }
}

/// Build a feature name schema from a collection of feature vectors.
///
/// Returns the sorted union of all feature names across all vectors.
/// This ensures every vector maps to the same column order.
pub fn build_schema(vectors: &[FeatureVector]) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    for fv in vectors {
        for k in fv.signal_features.keys() {
            names.insert(k.clone());
        }
        for k in fv.metric_features.keys() {
            names.insert(format!("metric:{k}"));
        }
    }
    names.into_iter().collect()
}

/// Convert feature vectors + labels into an ndarray feature matrix and label array.
///
/// Returns `(X, y, schema, encoder)` where:
/// - `X` is `(n_samples, n_features)` feature matrix
/// - `y` is `(n_samples,)` encoded label array
/// - `schema` is the ordered feature name list
/// - `encoder` maps between FamilyId and usize indices
pub fn build_dataset(
    vectors: &[FeatureVector],
    labels: &[FamilyId],
) -> (Array2<f64>, Array1<usize>, Vec<String>, LabelEncoder) {
    assert_eq!(vectors.len(), labels.len());

    let schema = build_schema(vectors);
    let encoder = LabelEncoder::fit(labels);

    let n_samples = vectors.len();
    let n_features = schema.len();

    let mut x = Array2::<f64>::zeros((n_samples, n_features));

    for (i, fv) in vectors.iter().enumerate() {
        for (j, name) in schema.iter().enumerate() {
            let value = if let Some(metric_name) = name.strip_prefix("metric:") {
                fv.metric_features.get(metric_name).copied().unwrap_or(0.0)
            } else {
                fv.signal_features.get(name).copied().unwrap_or(0.0)
            };
            x[[i, j]] = value;
        }
    }

    let y = Array1::from_vec(
        labels
            .iter()
            .map(|l| encoder.encode(l).unwrap_or(0))
            .collect(),
    );

    (x, y, schema, encoder)
}

/// Split indices into train and test sets, stratified by label.
///
/// Returns `(train_indices, test_indices)`.  Each class contributes
/// `(1 - test_ratio)` of its samples to training.
pub fn stratified_split(
    labels: &[usize],
    test_ratio: f64,
    seed: u64,
) -> (Vec<usize>, Vec<usize>) {
    use std::collections::BTreeMap;

    let mut per_class: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &label) in labels.iter().enumerate() {
        per_class.entry(label).or_default().push(i);
    }

    // Simple deterministic shuffle using a linear congruential generator
    let mut rng_state = seed;
    let mut shuffle = |v: &mut Vec<usize>| {
        for i in (1..v.len()).rev() {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (rng_state >> 33) as usize % (i + 1);
            v.swap(i, j);
        }
    };

    let mut train = Vec::new();
    let mut test = Vec::new();

    for (_, mut indices) in per_class {
        shuffle(&mut indices);
        let n_test = ((indices.len() as f64 * test_ratio).round() as usize).max(1);
        let n_test = n_test.min(indices.len().saturating_sub(1));
        test.extend_from_slice(&indices[..n_test]);
        train.extend_from_slice(&indices[n_test..]);
    }

    train.sort();
    test.sort();
    (train, test)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fid(s: &str) -> FamilyId {
        FamilyId(s.into())
    }

    #[test]
    fn label_encoder_fit_and_encode() {
        let labels = vec![fid("claude"), fid("gpt"), fid("claude"), fid("human")];
        let enc = LabelEncoder::fit(&labels);
        assert_eq!(enc.n_classes(), 3);
        assert_eq!(enc.encode(&fid("claude")), Some(0));
        assert_eq!(enc.encode(&fid("gpt")), Some(1));
        assert_eq!(enc.encode(&fid("human")), Some(2));
        assert_eq!(enc.encode(&fid("unknown")), None);
    }

    #[test]
    fn label_encoder_decode_roundtrip() {
        let labels = vec![fid("a"), fid("b"), fid("c")];
        let enc = LabelEncoder::fit(&labels);
        for l in &labels {
            let idx = enc.encode(l).unwrap();
            assert_eq!(enc.decode(idx).unwrap(), l);
        }
    }

    #[test]
    fn build_schema_unions_all_features() {
        let mut fv1 = FeatureVector::empty("rust", 10);
        fv1.signal_features.insert("sig_a".into(), 1.0);
        fv1.metric_features.insert("met_x".into(), 2.0);

        let mut fv2 = FeatureVector::empty("rust", 20);
        fv2.signal_features.insert("sig_b".into(), 1.0);

        let schema = build_schema(&[fv1, fv2]);
        assert!(schema.contains(&"sig_a".to_string()));
        assert!(schema.contains(&"sig_b".to_string()));
        assert!(schema.contains(&"metric:met_x".to_string()));
        assert_eq!(schema.len(), 3);
    }

    #[test]
    fn build_dataset_shapes() {
        let mut fv = FeatureVector::empty("rust", 10);
        fv.signal_features.insert("a".into(), 1.0);
        fv.signal_features.insert("b".into(), 2.0);
        let vectors = vec![fv.clone(), fv.clone(), fv];
        let labels = vec![fid("claude"), fid("gpt"), fid("claude")];

        let (x, y, schema, encoder) = build_dataset(&vectors, &labels);
        assert_eq!(x.shape(), &[3, 2]);
        assert_eq!(y.len(), 3);
        assert_eq!(schema.len(), 2);
        assert_eq!(encoder.n_classes(), 2);
    }

    #[test]
    fn build_dataset_values_correct() {
        let mut fv1 = FeatureVector::empty("rust", 10);
        fv1.signal_features.insert("feat".into(), 3.5);
        let mut fv2 = FeatureVector::empty("rust", 10);
        fv2.signal_features.insert("feat".into(), 7.0);

        let vectors = vec![fv1, fv2];
        let labels = vec![fid("a"), fid("b")];

        let (x, y, _, _) = build_dataset(&vectors, &labels);
        assert_eq!(x[[0, 0]], 3.5);
        assert_eq!(x[[1, 0]], 7.0);
        assert_eq!(y[0], 0);
        assert_eq!(y[1], 1);
    }

    #[test]
    fn stratified_split_preserves_classes() {
        let labels = vec![0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2];
        let (train, test) = stratified_split(&labels, 0.25, 42);

        assert!(!train.is_empty());
        assert!(!test.is_empty());
        assert_eq!(train.len() + test.len(), labels.len());

        let train_classes: std::collections::HashSet<usize> =
            train.iter().map(|&i| labels[i]).collect();
        let test_classes: std::collections::HashSet<usize> =
            test.iter().map(|&i| labels[i]).collect();
        assert_eq!(train_classes.len(), 3, "all classes should be in train");
        assert_eq!(test_classes.len(), 3, "all classes should be in test");
    }

    #[test]
    fn stratified_split_deterministic() {
        let labels = vec![0, 0, 1, 1, 2, 2];
        let (t1, _) = stratified_split(&labels, 0.5, 99);
        let (t2, _) = stratified_split(&labels, 0.5, 99);
        assert_eq!(t1, t2, "same seed should give same split");
    }

    #[test]
    fn label_encoder_families_returns_sorted() {
        let labels = vec![fid("gpt"), fid("claude"), fid("human")];
        let enc = LabelEncoder::fit(&labels);
        let fams = enc.families();
        assert_eq!(fams.len(), 3);
        assert_eq!(fams[0], fid("claude"));
        assert_eq!(fams[1], fid("gpt"));
        assert_eq!(fams[2], fid("human"));
    }
}
