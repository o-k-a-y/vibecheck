use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use vibecheck_core::report::FamilyId;

// ---------------------------------------------------------------------------
// Vocabulary — bidirectional string↔u32 mapping for AST node kinds
// ---------------------------------------------------------------------------

const UNK_TOKEN: &str = "<UNK>";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vocabulary {
    token_to_id: HashMap<String, u32>,
    id_to_token: Vec<String>,
}

impl Vocabulary {
    pub fn new() -> Self {
        let mut v = Self {
            token_to_id: HashMap::new(),
            id_to_token: Vec::new(),
        };
        v.intern(UNK_TOKEN);
        v
    }

    pub fn intern(&mut self, token: &str) -> u32 {
        if let Some(&id) = self.token_to_id.get(token) {
            return id;
        }
        let id = self.id_to_token.len() as u32;
        self.id_to_token.push(token.to_string());
        self.token_to_id.insert(token.to_string(), id);
        id
    }

    pub fn get(&self, token: &str) -> u32 {
        self.token_to_id
            .get(token)
            .copied()
            .unwrap_or(self.unk_id())
    }

    pub fn resolve(&self, id: u32) -> &str {
        self.id_to_token
            .get(id as usize)
            .map(|s| s.as_str())
            .unwrap_or(UNK_TOKEN)
    }

    pub fn unk_id(&self) -> u32 {
        0
    }

    pub fn len(&self) -> usize {
        self.id_to_token.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id_to_token.len() <= 1
    }
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AST sequence extraction — depth-first traversal of tree-sitter tree
// ---------------------------------------------------------------------------

/// Extract a sequence of AST node kind strings via depth-first traversal.
///
/// Visits every named node in the tree, collecting `node.kind()` in DFS
/// pre-order.  Anonymous/unnamed nodes (punctuation, operators) are skipped
/// to keep sequences focused on structural patterns.
pub fn extract_ast_sequence(tree: &tree_sitter::Tree) -> Vec<String> {
    let mut sequence = Vec::new();
    let mut cursor = tree.walk();
    dfs_collect(&mut cursor, &mut sequence);
    sequence
}

fn dfs_collect(cursor: &mut tree_sitter::TreeCursor, out: &mut Vec<String>) {
    loop {
        let node = cursor.node();
        if node.is_named() {
            out.push(node.kind().to_string());
        }
        if cursor.goto_first_child() {
            dfs_collect(cursor, out);
            cursor.goto_parent();
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

/// Intern an AST sequence into vocabulary IDs (mutates the vocabulary).
pub fn intern_sequence(vocab: &mut Vocabulary, sequence: &[String]) -> Vec<u32> {
    sequence.iter().map(|s| vocab.intern(s)).collect()
}

/// Map an AST sequence to vocabulary IDs (read-only, unknown → UNK).
pub fn encode_sequence(vocab: &Vocabulary, sequence: &[String]) -> Vec<u32> {
    sequence.iter().map(|s| vocab.get(s)).collect()
}

// ---------------------------------------------------------------------------
// TransitionMatrix — sparse n-gram transition counts / probabilities
// ---------------------------------------------------------------------------

/// Sparse transition matrix for an n-gram Markov model.
///
/// For order `k`, the key is a `(k+1)`-gram stored as a `Vec<u32>` prefix of
/// length `k` mapped to a `HashMap<u32, f64>` of next-token probabilities.
/// This representation is compact for the typically sparse AST vocabularies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionMatrix {
    pub order: u8,
    pub vocab_size: u32,
    #[serde(with = "transition_serde")]
    transitions: HashMap<Vec<u32>, HashMap<u32, f64>>,
}

mod transition_serde {
    use super::*;
    use serde::ser::SerializeMap;

    type Inner = HashMap<Vec<u32>, HashMap<u32, f64>>;

    pub fn serialize<S: serde::Serializer>(map: &Inner, ser: S) -> Result<S::Ok, S::Error> {
        let mut m = ser.serialize_map(Some(map.len()))?;
        for (ctx, nexts) in map {
            let ctx_key: String = ctx.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
            let str_nexts: HashMap<String, f64> =
                nexts.iter().map(|(k, &v)| (k.to_string(), v)).collect();
            m.serialize_entry(&ctx_key, &str_nexts)?;
        }
        m.end()
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Inner, D::Error> {
        let raw: HashMap<String, HashMap<String, f64>> = HashMap::deserialize(de)?;
        let mut out = Inner::new();
        for (ctx_str, nexts_str) in raw {
            let ctx: Vec<u32> = ctx_str
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().map_err(serde::de::Error::custom))
                .collect::<Result<_, _>>()?;
            let nexts: HashMap<u32, f64> = nexts_str
                .into_iter()
                .map(|(k, v)| Ok((k.parse::<u32>().map_err(serde::de::Error::custom)?, v)))
                .collect::<Result<_, D::Error>>()?;
            out.insert(ctx, nexts);
        }
        Ok(out)
    }
}

impl TransitionMatrix {
    /// Train from interned ID sequences with Laplace smoothing.
    ///
    /// `alpha` is the Laplace smoothing constant (typically 1.0).
    pub fn train(sequences: &[Vec<u32>], order: u8, vocab_size: u32, alpha: f64) -> Self {
        let k = order as usize;
        let mut counts: HashMap<Vec<u32>, HashMap<u32, f64>> = HashMap::new();

        for seq in sequences {
            if seq.len() <= k {
                continue;
            }
            for window in seq.windows(k + 1) {
                let context = window[..k].to_vec();
                let next = window[k];
                *counts
                    .entry(context)
                    .or_default()
                    .entry(next)
                    .or_insert(0.0) += 1.0;
            }
        }

        let mut transitions: HashMap<Vec<u32>, HashMap<u32, f64>> = HashMap::new();
        let vs = vocab_size as f64;

        for (context, next_counts) in &counts {
            let total: f64 = next_counts.values().sum::<f64>() + alpha * vs;
            let mut probs = HashMap::new();
            for (&next_id, &count) in next_counts {
                probs.insert(next_id, (count + alpha) / total);
            }
            transitions.insert(context.clone(), probs);
        }

        Self {
            order,
            vocab_size,
            transitions,
        }
    }

    /// Log-probability of a token given its context.
    ///
    /// Returns the log-probability (base e).  Unseen contexts get a uniform
    /// distribution; unseen transitions within a known context get the
    /// Laplace-smoothed floor (computed from the remaining probability mass).
    pub fn log_prob(&self, context: &[u32], next: u32) -> f64 {
        let vs = self.vocab_size as f64;
        match self.transitions.get(context) {
            Some(probs) => {
                let p = probs.get(&next).copied().unwrap_or_else(|| {
                    let observed_mass: f64 = probs.values().sum::<f64>();
                    let unobserved = (1.0 - observed_mass)
                        / (vs - probs.len() as f64).max(1.0);
                    unobserved.max(f64::MIN_POSITIVE)
                });
                p.max(f64::MIN_POSITIVE).ln()
            }
            None => (1.0 / vs).ln(),
        }
    }

    /// Total log-likelihood of a sequence under this model.
    pub fn sequence_log_likelihood(&self, sequence: &[u32]) -> f64 {
        let k = self.order as usize;
        if sequence.len() <= k {
            return 0.0;
        }
        let mut ll = 0.0;
        for window in sequence.windows(k + 1) {
            ll += self.log_prob(&window[..k], window[k]);
        }
        ll
    }

    pub fn context_count(&self) -> usize {
        self.transitions.len()
    }
}

// ---------------------------------------------------------------------------
// MarkovModel — one (language, family, order) combination
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkovModel {
    pub language: String,
    pub family: String,
    pub order: u8,
    pub matrix: TransitionMatrix,
}

// ---------------------------------------------------------------------------
// MarkovClassifier — multi-family scoring with adaptive backoff
// ---------------------------------------------------------------------------

/// Classifies AST sequences by comparing log-likelihoods under per-family
/// Markov models.  Supports adaptive order backoff: tries order 3, falls
/// back to 2, then 1 if higher-order data is insufficient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkovClassifier {
    models: Vec<MarkovModel>,
    pub vocabulary: Vocabulary,
    alpha: f64,
}

impl MarkovClassifier {
    pub fn new(vocabulary: Vocabulary, alpha: f64) -> Self {
        Self {
            models: Vec::new(),
            vocabulary,
            alpha,
        }
    }

    /// Train a model for the given language and family from raw AST
    /// sequences (already interned as u32 IDs).
    pub fn add_model(
        &mut self,
        language: &str,
        family: &str,
        order: u8,
        sequences: &[Vec<u32>],
    ) {
        let matrix =
            TransitionMatrix::train(sequences, order, self.vocabulary.len() as u32, self.alpha);
        self.models.push(MarkovModel {
            language: language.into(),
            family: family.into(),
            order,
            matrix,
        });
    }

    /// Score an interned sequence against all models for the given language.
    ///
    /// Returns a map of `FamilyId → normalized probability`.  Uses adaptive
    /// backoff: for each family, tries the highest available order first,
    /// falling back to lower orders if no model exists at that order.
    pub fn classify(
        &self,
        sequence: &[u32],
        language: &str,
    ) -> HashMap<FamilyId, f64> {
        let mut log_likes: HashMap<String, f64> = HashMap::new();

        let families: Vec<String> = self
            .models
            .iter()
            .filter(|m| m.language == language)
            .map(|m| m.family.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        for family in &families {
            let ll = self.best_log_likelihood(sequence, language, family);
            log_likes.insert(family.clone(), ll);
        }

        normalize_log_likelihoods(&log_likes)
    }

    /// Adaptive backoff: try order 3 → 2 → 1 for this (language, family).
    fn best_log_likelihood(
        &self,
        sequence: &[u32],
        language: &str,
        family: &str,
    ) -> f64 {
        for order in (1..=3).rev() {
            if let Some(model) = self.models.iter().find(|m| {
                m.language == language && m.family == family && m.order == order
            }) {
                let ll = model.matrix.sequence_log_likelihood(sequence);
                if ll.is_finite() && ll != 0.0 {
                    return ll;
                }
            }
        }
        f64::NEG_INFINITY
    }

    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    pub fn families_for_language(&self, language: &str) -> Vec<String> {
        let mut fams: Vec<String> = self
            .models
            .iter()
            .filter(|m| m.language == language)
            .map(|m| m.family.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        fams.sort();
        fams
    }
}

/// Convert log-likelihoods to normalized probabilities via log-sum-exp.
fn normalize_log_likelihoods(log_likes: &HashMap<String, f64>) -> HashMap<FamilyId, f64> {
    if log_likes.is_empty() {
        return HashMap::new();
    }

    let max_ll = log_likes
        .values()
        .copied()
        .filter(|v| v.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);

    if !max_ll.is_finite() {
        let uniform = 1.0 / log_likes.len() as f64;
        return log_likes
            .keys()
            .map(|k| (FamilyId(k.clone()), uniform))
            .collect();
    }

    let exp_sum: f64 = log_likes
        .values()
        .map(|&ll| {
            if ll.is_finite() {
                (ll - max_ll).exp()
            } else {
                0.0
            }
        })
        .sum();

    log_likes
        .iter()
        .map(|(k, &ll)| {
            let prob = if ll.is_finite() {
                (ll - max_ll).exp() / exp_sum
            } else {
                0.0
            };
            (FamilyId(k.clone()), prob)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulary_intern_and_get() {
        let mut vocab = Vocabulary::new();
        let id_fn = vocab.intern("function_definition");
        let id_class = vocab.intern("class_definition");
        assert_ne!(id_fn, id_class);
        assert_eq!(vocab.get("function_definition"), id_fn);
        assert_eq!(vocab.get("class_definition"), id_class);
        assert_eq!(vocab.get("unknown_node"), vocab.unk_id());
    }

    #[test]
    fn vocabulary_resolve() {
        let mut vocab = Vocabulary::new();
        let id = vocab.intern("identifier");
        assert_eq!(vocab.resolve(id), "identifier");
        assert_eq!(vocab.resolve(9999), UNK_TOKEN);
    }

    #[test]
    fn vocabulary_unk_is_zero() {
        let vocab = Vocabulary::new();
        assert_eq!(vocab.unk_id(), 0);
        assert_eq!(vocab.resolve(0), UNK_TOKEN);
    }

    #[test]
    fn vocabulary_len() {
        let mut vocab = Vocabulary::new();
        assert_eq!(vocab.len(), 1); // UNK
        vocab.intern("a");
        vocab.intern("b");
        assert_eq!(vocab.len(), 3);
    }

    #[test]
    fn vocabulary_json_roundtrip() {
        let mut vocab = Vocabulary::new();
        vocab.intern("function_definition");
        vocab.intern("class_definition");
        let json = serde_json::to_string(&vocab).unwrap();
        let back: Vocabulary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), vocab.len());
        assert_eq!(back.get("function_definition"), vocab.get("function_definition"));
    }

    #[test]
    fn intern_sequence_grows_vocab() {
        let mut vocab = Vocabulary::new();
        let seq = vec![
            "function_definition".to_string(),
            "identifier".to_string(),
            "parameters".to_string(),
        ];
        let ids = intern_sequence(&mut vocab, &seq);
        assert_eq!(ids.len(), 3);
        assert_eq!(vocab.len(), 4); // UNK + 3
    }

    #[test]
    fn encode_sequence_uses_unk_for_unknown() {
        let mut vocab = Vocabulary::new();
        vocab.intern("known");
        let seq = vec!["known".to_string(), "unknown".to_string()];
        let ids = encode_sequence(&vocab, &seq);
        assert_eq!(ids[0], vocab.get("known"));
        assert_eq!(ids[1], vocab.unk_id());
    }

    #[test]
    fn transition_matrix_train_order1() {
        let seqs = vec![vec![1u32, 2, 3, 1, 2, 3]];
        let tm = TransitionMatrix::train(&seqs, 1, 4, 1.0);
        assert_eq!(tm.order, 1);
        assert!(tm.context_count() > 0);
    }

    #[test]
    fn transition_matrix_log_prob_known_context() {
        let seqs = vec![vec![1u32, 2, 1, 2, 1, 2]];
        let tm = TransitionMatrix::train(&seqs, 1, 3, 0.1);
        let lp = tm.log_prob(&[1], 2);
        assert!(lp < 0.0, "log-prob should be negative");
        assert!(lp.is_finite());
    }

    #[test]
    fn transition_matrix_log_prob_unknown_context() {
        let seqs = vec![vec![1u32, 2, 3]];
        let tm = TransitionMatrix::train(&seqs, 1, 4, 1.0);
        let lp = tm.log_prob(&[99], 1);
        assert!(lp.is_finite());
    }

    #[test]
    fn transition_matrix_sequence_log_likelihood() {
        let train = vec![vec![1u32, 2, 3, 1, 2, 3, 1, 2, 3]];
        let tm = TransitionMatrix::train(&train, 1, 4, 0.1);
        let ll_matching = tm.sequence_log_likelihood(&[1, 2, 3, 1, 2]);
        let ll_random = tm.sequence_log_likelihood(&[3, 3, 3, 3, 3]);
        assert!(
            ll_matching > ll_random,
            "matching sequence should have higher likelihood: {ll_matching} vs {ll_random}"
        );
    }

    #[test]
    fn transition_matrix_json_roundtrip() {
        let seqs = vec![vec![1u32, 2, 3]];
        let tm = TransitionMatrix::train(&seqs, 1, 4, 1.0);
        let json = serde_json::to_string(&tm).unwrap();
        let back: TransitionMatrix = serde_json::from_str(&json).unwrap();
        assert_eq!(back.order, tm.order);
        assert_eq!(back.vocab_size, tm.vocab_size);
        assert_eq!(back.context_count(), tm.context_count());
    }

    #[test]
    fn markov_classifier_classify_prefers_matching_family() {
        let mut vocab = Vocabulary::new();
        let a = vocab.intern("a");
        let b = vocab.intern("b");
        let c = vocab.intern("c");

        let pattern_a = vec![vec![a, b, a, b, a, b, a, b, a, b]];
        let pattern_b = vec![vec![c, c, c, c, c, c, c, c, c, c]];

        let mut clf = MarkovClassifier::new(vocab, 0.1);
        clf.add_model("rust", "claude", 1, &pattern_a);
        clf.add_model("rust", "gpt", 1, &pattern_b);

        let test_seq = vec![a, b, a, b, a, b];
        let result = clf.classify(&test_seq, "rust");

        let claude_score = result.get(&FamilyId("claude".into())).copied().unwrap_or(0.0);
        let gpt_score = result.get(&FamilyId("gpt".into())).copied().unwrap_or(0.0);
        assert!(
            claude_score > gpt_score,
            "claude ({claude_score:.4}) should score higher than gpt ({gpt_score:.4}) for matching pattern"
        );
    }

    #[test]
    fn markov_classifier_classify_normalizes_to_one() {
        let mut vocab = Vocabulary::new();
        let a = vocab.intern("a");
        let b = vocab.intern("b");

        let seqs = vec![vec![a, b, a, b, a]];
        let mut clf = MarkovClassifier::new(vocab, 1.0);
        clf.add_model("rust", "claude", 1, &seqs);
        clf.add_model("rust", "gpt", 1, &seqs);

        let result = clf.classify(&[a, b, a], "rust");
        let total: f64 = result.values().sum();
        assert!(
            (total - 1.0).abs() < 1e-6,
            "probabilities should sum to ~1.0; got {total}"
        );
    }

    #[test]
    fn markov_classifier_empty_for_unknown_language() {
        let vocab = Vocabulary::new();
        let clf = MarkovClassifier::new(vocab, 1.0);
        let result = clf.classify(&[1, 2, 3], "haskell");
        assert!(result.is_empty());
    }

    #[test]
    fn markov_classifier_adaptive_backoff() {
        let mut vocab = Vocabulary::new();
        let a = vocab.intern("a");
        let b = vocab.intern("b");

        let seqs = vec![vec![a, b, a, b, a, b, a, b]];

        let mut clf = MarkovClassifier::new(vocab, 0.1);
        clf.add_model("rust", "claude", 1, &seqs);
        clf.add_model("rust", "claude", 2, &seqs);

        let result = clf.classify(&[a, b, a, b], "rust");
        assert!(!result.is_empty());
    }

    #[test]
    fn markov_classifier_json_roundtrip() {
        let mut vocab = Vocabulary::new();
        let a = vocab.intern("a");
        let b = vocab.intern("b");

        let seqs = vec![vec![a, b, a, b]];
        let mut clf = MarkovClassifier::new(vocab, 1.0);
        clf.add_model("rust", "claude", 1, &seqs);

        let json = serde_json::to_string(&clf).unwrap();
        let back: MarkovClassifier = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model_count(), 1);
        assert_eq!(back.vocabulary.len(), clf.vocabulary.len());
    }

    #[test]
    fn normalize_log_likelihoods_uniform_for_equal_ll() {
        let mut lls = HashMap::new();
        lls.insert("a".into(), -5.0);
        lls.insert("b".into(), -5.0);
        let probs = normalize_log_likelihoods(&lls);
        let a = probs[&FamilyId("a".into())];
        let b = probs[&FamilyId("b".into())];
        assert!((a - b).abs() < 1e-9, "equal LL should give equal probs");
        assert!((a - 0.5).abs() < 1e-9);
    }

    #[test]
    fn normalize_log_likelihoods_handles_neg_infinity() {
        let mut lls = HashMap::new();
        lls.insert("a".into(), -2.0);
        lls.insert("b".into(), f64::NEG_INFINITY);
        let probs = normalize_log_likelihoods(&lls);
        assert!(probs[&FamilyId("a".into())] > 0.99);
        assert!(probs[&FamilyId("b".into())] < 0.01);
    }

    #[test]
    fn vocabulary_is_empty() {
        let vocab = Vocabulary::new();
        assert!(vocab.is_empty(), "new vocab has only UNK, should be empty");

        let mut vocab2 = Vocabulary::new();
        vocab2.intern("x");
        assert!(!vocab2.is_empty());
    }

    #[test]
    fn vocabulary_default() {
        let vocab = Vocabulary::default();
        assert_eq!(vocab.len(), 1);
        assert!(vocab.is_empty());
    }

    #[test]
    fn normalize_all_neg_infinity_returns_uniform() {
        let mut lls = HashMap::new();
        lls.insert("a".into(), f64::NEG_INFINITY);
        lls.insert("b".into(), f64::NEG_INFINITY);
        let probs = normalize_log_likelihoods(&lls);
        let a = probs[&FamilyId("a".into())];
        let b = probs[&FamilyId("b".into())];
        assert!((a - 0.5).abs() < 1e-9, "all NEG_INF should be uniform; a={a}");
        assert!((b - 0.5).abs() < 1e-9, "all NEG_INF should be uniform; b={b}");
    }

    #[test]
    fn transition_matrix_empty_sequence() {
        let seqs = vec![vec![1u32, 2, 3]];
        let tm = TransitionMatrix::train(&seqs, 1, 4, 1.0);
        let ll = tm.sequence_log_likelihood(&[]);
        assert_eq!(ll, 0.0, "empty sequence should have 0.0 log-likelihood");
    }

    #[test]
    fn families_for_language_lists_trained() {
        let mut vocab = Vocabulary::new();
        vocab.intern("x");
        let seqs = vec![vec![1u32, 1, 1]];
        let mut clf = MarkovClassifier::new(vocab, 1.0);
        clf.add_model("rust", "claude", 1, &seqs);
        clf.add_model("rust", "gpt", 1, &seqs);
        clf.add_model("python", "human", 1, &seqs);

        let rust_fams = clf.families_for_language("rust");
        assert!(rust_fams.contains(&"claude".to_string()));
        assert!(rust_fams.contains(&"gpt".to_string()));
        assert!(!rust_fams.contains(&"human".to_string()));
    }
}
