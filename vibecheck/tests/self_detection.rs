use std::path::Path;
use vibecheck::report::ModelFamily;

#[test]
fn detects_own_source_as_ai_generated() {
    // Paths are relative to the vibecheck package root (cargo integration test working dir)
    let source_files = &[
        "src/lib.rs",
        "src/report.rs",
        "src/pipeline.rs",
        "src/analyzers/mod.rs",
        "src/analyzers/text/comment_style.rs",
        "src/analyzers/text/ai_signals.rs",
        "src/analyzers/text/error_handling.rs",
        "src/analyzers/text/naming.rs",
        "src/analyzers/text/code_structure.rs",
        "src/analyzers/text/idiom_usage.rs",
    ];

    let mut ai_detected = 0;
    let mut total = 0;

    for file in source_files {
        let path = Path::new(file);
        let report =
            vibecheck::analyze_file(path).expect("should be able to read own source");
        total += 1;

        let is_ai = report.attribution.primary != ModelFamily::Human;
        if is_ai {
            ai_detected += 1;
        } else {
            eprintln!(
                "WARN: {} was classified as Human (confidence: {:.1}%)",
                file,
                report.attribution.confidence * 100.0
            );
        }
    }

    assert!(total > 0, "should have found source files to analyze");
    let ratio = ai_detected as f64 / total as f64;
    assert!(
        ratio >= 0.5,
        "expected at least half of own source files to be detected as AI, got {ai_detected}/{total}"
    );
}

#[test]
fn analyze_string_produces_report() {
    let source = r#"
use std::collections::HashMap;

/// A simple data processor that transforms input records.
pub struct Processor {
    config: HashMap<String, String>,
}

impl Processor {
    /// Creates a new Processor with the given configuration.
    pub fn new(config: HashMap<String, String>) -> Self {
        Self { config }
    }

    /// Processes a single record and returns the transformed output.
    pub fn process(&self, input: &str) -> Result<String, ProcessError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ProcessError::EmptyInput);
        }
        let result = trimmed
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.to_uppercase())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(result)
    }
}

/// Errors that can occur during processing.
#[derive(Debug)]
pub enum ProcessError {
    EmptyInput,
    InvalidFormat(String),
}
"#;

    let report = vibecheck::analyze(source);
    assert!(!report.signals.is_empty(), "should produce signals");
    assert!(report.metadata.lines_of_code > 0);
    assert_ne!(
        report.attribution.primary,
        ModelFamily::Human,
        "textbook-clean code should be classified as AI-generated"
    );
}

#[test]
fn human_looking_code_detected() {
    let source = r#"
fn main() {
    let x = 42;
    let mut v = vec![];
    // TODO: fix this hack
    for i in 0..x {
        if i % 2 == 0 {
            v.push(i);
        }
    }
    // let old_val = compute_thing();
    // println!("{}", old_val);
    let n = v.len();
    println!("{}", n);
    let s = "asdf";
    let _unused = s.to_string();
}
"#;

    let report = vibecheck::analyze(source);
    assert!(!report.signals.is_empty());
    let human_score = report
        .attribution
        .scores
        .get(&ModelFamily::Human)
        .copied()
        .unwrap_or(0.0);
    assert!(
        human_score > 0.1,
        "human-looking code should have some Human score, got {human_score}"
    );
}

#[test]
fn single_file_analysis_under_100ms() {
    let path = Path::new("src/pipeline.rs");
    if !path.exists() {
        panic!("src/pipeline.rs not found — required for timing test");
    }
    let start = std::time::Instant::now();
    let _report = vibecheck::analyze_file(path).unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "analysis took {}ms, expected < 100ms",
        elapsed.as_millis()
    );
}

#[test]
fn analyze_file_no_cache_matches_analyze() {
    let path = Path::new("src/pipeline.rs");
    if !path.exists() {
        panic!("src/pipeline.rs not found — required for test");
    }
    let report = vibecheck::analyze_file_no_cache(path)
        .expect("should analyze file without cache");
    assert_ne!(
        report.attribution.primary,
        ModelFamily::Human,
        "pipeline.rs should be classified as AI-generated"
    );
    assert!(!report.signals.is_empty(), "should produce signals for pipeline.rs");
}

#[test]
fn cache_round_trip() {
    let tmp = std::env::temp_dir().join(format!("vibecheck_test_{}", std::process::id()));
    let cache = vibecheck::cache::Cache::open(&tmp).expect("open cache");

    let report = vibecheck::analyze("fn main() { println!(\"hello\"); }");
    let hash = vibecheck::cache::Cache::hash_content(b"cache_round_trip_test_content");

    cache.put(&hash, &report).expect("put report into cache");
    let retrieved = cache.get(&hash).expect("get report from cache");

    assert_eq!(retrieved.attribution.primary, report.attribution.primary);
    assert_eq!(retrieved.signals.len(), report.signals.len());
    assert_eq!(retrieved.metadata.lines_of_code, report.metadata.lines_of_code);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn report_scores_sum_to_one() {
    let source = r#"
use std::collections::HashMap;

/// A simple struct.
pub struct Processor {
    data: HashMap<String, String>,
}

impl Processor {
    /// Create a new Processor.
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// Process a key.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
}
"#;
    let report = vibecheck::analyze(source);
    let sum: f64 = report.attribution.scores.values().sum();
    assert!(
        (sum - 1.0).abs() < 0.001,
        "scores should sum to 1.0, got {sum}"
    );
}

#[test]
fn signals_have_valid_weights() {
    let source = r#"
use std::collections::HashMap;

/// A simple struct.
pub struct Processor {
    data: HashMap<String, String>,
}

impl Processor {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }
}
"#;
    let report = vibecheck::analyze(source);
    for signal in &report.signals {
        assert!(
            !signal.weight.is_nan(),
            "signal weight is NaN: {:?}", signal
        );
        assert!(
            !signal.weight.is_infinite(),
            "signal weight is infinite: {:?}", signal
        );
    }
}
