use std::path::Path;
use vibecheck::report::ModelFamily;

#[test]
fn detects_own_source_as_ai_generated() {
    let source_files = &[
        "src/lib.rs",
        "src/report.rs",
        "src/pipeline.rs",
        "src/analyzers/mod.rs",
        "src/analyzers/comment_style.rs",
        "src/analyzers/ai_signals.rs",
        "src/analyzers/error_handling.rs",
        "src/analyzers/naming.rs",
        "src/analyzers/code_structure.rs",
        "src/analyzers/idiom_usage.rs",
    ];

    let mut ai_detected = 0;
    let mut total = 0;

    for file in source_files {
        let path = Path::new(file);
        if !path.exists() {
            continue;
        }
        let report = vibecheck::analyze_file(path).expect("should be able to read own source");
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
    // This textbook-clean code should be detected as AI
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
    // This messy code should lean toward Human
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
        return;
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
