use std::collections::HashMap;
use std::path::Path;

use vibecheck_ml::markov::{
    extract_ast_sequence, intern_sequence, MarkovClassifier, Vocabulary,
};

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vibecheck-core/tests/fixtures/lru_cache")
        .join(name)
}

fn parse_fixture(path: &std::path::PathBuf, lang: &tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let source = std::fs::read_to_string(path).ok()?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(lang).ok()?;
    parser.parse(source.as_bytes(), None)
}

fn ts_lang_for_ext(ext: &str) -> tree_sitter::Language {
    match ext {
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        "py" => tree_sitter_python::LANGUAGE.into(),
        "js" => tree_sitter_javascript::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        _ => panic!("unsupported extension: {ext}"),
    }
}

const FAMILIES: &[&str] = &["claude", "gpt", "gemini", "copilot", "human"];
const EXTENSIONS: &[&str] = &["rs", "py", "js", "go"];

fn lang_name(ext: &str) -> &str {
    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "go" => "go",
        _ => ext,
    }
}

fn build_classifier() -> (MarkovClassifier, HashMap<String, Vec<(String, Vec<u32>)>>) {
    let mut vocab = Vocabulary::new();
    let mut per_lang_family: HashMap<String, Vec<(String, Vec<u32>)>> = HashMap::new();

    for &ext in EXTENSIONS {
        let ts_lang = ts_lang_for_ext(ext);
        let language = lang_name(ext);

        for &family in FAMILIES {
            let filename = format!("{family}.{ext}");
            let path = fixture_path(&filename);
            if let Some(tree) = parse_fixture(&path, &ts_lang) {
                let raw = extract_ast_sequence(&tree);
                let interned = intern_sequence(&mut vocab, &raw);
                per_lang_family
                    .entry(language.to_string())
                    .or_default()
                    .push((family.to_string(), interned));
            }
        }
    }

    let mut clf = MarkovClassifier::new(vocab, 0.5);

    for (language, entries) in &per_lang_family {
        for (family, seq) in entries {
            clf.add_model(language, family, 1, &[seq.clone()]);
            if seq.len() > 3 {
                clf.add_model(language, family, 2, &[seq.clone()]);
            }
        }
    }

    (clf, per_lang_family)
}

#[test]
fn ast_extraction_produces_nonempty_sequences() {
    for &ext in EXTENSIONS {
        let ts_lang = ts_lang_for_ext(ext);
        for &family in FAMILIES {
            let path = fixture_path(&format!("{family}.{ext}"));
            let tree = parse_fixture(&path, &ts_lang).expect("should parse");
            let seq = extract_ast_sequence(&tree);
            assert!(
                seq.len() >= 10,
                "{family}.{ext}: expected >=10 AST nodes, got {}",
                seq.len()
            );
        }
    }
}

#[test]
fn classifier_produces_scores_for_all_families() {
    let (clf, per_lang_family) = build_classifier();

    for (language, entries) in &per_lang_family {
        let (_, ref seq) = entries[0];
        let scores = clf.classify(seq, language);
        assert_eq!(
            scores.len(),
            FAMILIES.len(),
            "{language}: expected scores for all {0} families, got {1}",
            FAMILIES.len(),
            scores.len()
        );
        let total: f64 = scores.values().sum();
        assert!(
            (total - 1.0).abs() < 1e-6,
            "{language}: scores should sum to ~1.0, got {total}"
        );
    }
}

#[test]
fn classifier_scores_normalize_to_one() {
    let (clf, per_lang_family) = build_classifier();

    for (language, entries) in &per_lang_family {
        for (family, seq) in entries {
            let scores = clf.classify(seq, language);
            let total: f64 = scores.values().sum();
            assert!(
                (total - 1.0).abs() < 1e-6,
                "{language}/{family}: scores sum to {total}, expected ~1.0"
            );
        }
    }
}

#[test]
fn classifier_differentiates_families_above_chance() {
    let (clf, per_lang_family) = build_classifier();

    let mut correct = 0;
    let mut total = 0;

    for (language, entries) in &per_lang_family {
        for (expected_family, seq) in entries {
            let scores = clf.classify(seq, language);
            let predicted = scores
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(fid, _)| fid.0.clone())
                .unwrap();
            if predicted == *expected_family {
                correct += 1;
            }
            total += 1;
        }
    }

    let accuracy = correct as f64 / total as f64;
    let chance = 1.0 / FAMILIES.len() as f64;
    assert!(
        accuracy > chance,
        "overall accuracy {accuracy:.1}% ({correct}/{total}) should be above chance ({chance:.1}%)"
    );
}

#[test]
fn json_roundtrip_preserves_classification() {
    let (clf, per_lang_family) = build_classifier();

    let json = serde_json::to_string(&clf).unwrap();
    let restored: MarkovClassifier = serde_json::from_str(&json).unwrap();

    for (language, entries) in &per_lang_family {
        let (_, ref seq) = entries[0];
        let original = clf.classify(seq, language);
        let roundtripped = restored.classify(seq, language);

        for (fid, &orig_score) in &original {
            let rt_score = roundtripped.get(fid).copied().unwrap_or(0.0);
            assert!(
                (orig_score - rt_score).abs() < 1e-9,
                "{language}: scores diverged after JSON roundtrip for {fid}"
            );
        }
    }
}

#[test]
fn different_languages_produce_different_sequences() {
    let mut sequences: HashMap<String, Vec<String>> = HashMap::new();

    for &ext in EXTENSIONS {
        let ts_lang = ts_lang_for_ext(ext);
        let path = fixture_path(&format!("claude.{ext}"));
        let tree = parse_fixture(&path, &ts_lang).unwrap();
        let seq = extract_ast_sequence(&tree);
        sequences.insert(ext.to_string(), seq);
    }

    let rs_seq = &sequences["rs"];
    let py_seq = &sequences["py"];
    assert_ne!(rs_seq, py_seq, "Rust and Python AST sequences should differ");

    assert!(
        rs_seq.iter().any(|n| n == "function_item"),
        "Rust should have function_item nodes"
    );
    assert!(
        py_seq.iter().any(|n| n == "function_definition"),
        "Python should have function_definition nodes"
    );
}

#[test]
fn model_count_matches_trained() {
    let (clf, _) = build_classifier();
    // 5 families × 4 languages × 2 orders = 40
    assert_eq!(clf.model_count(), 40);
}
