use std::path::Path;
use vibecheck_core::report::ModelFamily;

fn assert_fixture(fixture_path: &str, expected: ModelFamily) {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_path);
    let report = vibecheck_core::analyze_file_no_cache(&full)
        .unwrap_or_else(|e| panic!("Failed to analyze {fixture_path}: {e}"));
    assert_eq!(
        report.attribution.primary, expected,
        "{fixture_path}: expected {expected:?}, got {:?} (confidence {:.2}, scores: {:?})",
        report.attribution.primary, report.attribution.confidence, report.attribution.scores
    );
}

// ── Claude fixtures ────────────────────────────────────────────────────

#[test]
fn claude_rust() {
    assert_fixture("lru_cache/claude.rs", ModelFamily::Claude);
}

#[test]
fn claude_python() {
    assert_fixture("lru_cache/claude.py", ModelFamily::Claude);
}

#[test]
fn claude_javascript() {
    assert_fixture("lru_cache/claude.js", ModelFamily::Claude);
}

#[test]
fn claude_go() {
    assert_fixture("lru_cache/claude.go", ModelFamily::Claude);
}

// ── Human fixtures ─────────────────────────────────────────────────────

#[test]
fn human_rust() {
    assert_fixture("lru_cache/human.rs", ModelFamily::Human);
}

#[test]
fn human_python() {
    assert_fixture("lru_cache/human.py", ModelFamily::Human);
}

#[test]
fn human_javascript() {
    assert_fixture("lru_cache/human.js", ModelFamily::Human);
}

#[test]
fn human_go() {
    assert_fixture("lru_cache/human.go", ModelFamily::Human);
}

// ── GPT fixtures ──────────────────────────────────────────────────────

#[test]

fn gpt_rust() {
    assert_fixture("lru_cache/gpt.rs", ModelFamily::Gpt);
}

#[test]

fn gpt_python() {
    assert_fixture("lru_cache/gpt.py", ModelFamily::Gpt);
}

#[test]
fn gpt_javascript() {
    assert_fixture("lru_cache/gpt.js", ModelFamily::Gpt);
}

#[test]
fn gpt_go() {
    assert_fixture("lru_cache/gpt.go", ModelFamily::Gpt);
}

// ── Gemini fixtures ───────────────────────────────────────────────────

#[test]

fn gemini_rust() {
    assert_fixture("lru_cache/gemini.rs", ModelFamily::Gemini);
}

#[test]
fn gemini_python() {
    assert_fixture("lru_cache/gemini.py", ModelFamily::Gemini);
}

#[test]
fn gemini_javascript() {
    assert_fixture("lru_cache/gemini.js", ModelFamily::Gemini);
}

#[test]
fn gemini_go() {
    assert_fixture("lru_cache/gemini.go", ModelFamily::Gemini);
}

// ── Copilot fixtures ──────────────────────────────────────────────────

#[test]

fn copilot_rust() {
    assert_fixture("lru_cache/copilot.rs", ModelFamily::Copilot);
}

#[test]
fn copilot_python() {
    assert_fixture("lru_cache/copilot.py", ModelFamily::Copilot);
}

#[test]
fn copilot_javascript() {
    assert_fixture("lru_cache/copilot.js", ModelFamily::Copilot);
}

#[test]
fn copilot_go() {
    assert_fixture("lru_cache/copilot.go", ModelFamily::Copilot);
}
