//! Unit tests for `scripts/bump-version.sh`.
//!
//! The script exposes two seams for testing:
//!   `BUMP_TOML=<path>`  — inject a temporary Cargo.toml (no real git repo needed)
//!   `BUMP_SKIP_BUILD=1` — skip `cargo build`             (no workspace build needed)
//!
//! No external test frameworks or new crate dependencies are required beyond
//! `tempfile`, which is already a dev-dependency of this crate.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bump_script() -> PathBuf {
    // CARGO_MANIFEST_DIR is vibecheck-cli/; the script lives at ../scripts/
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().unwrap().join("scripts/bump-version.sh")
}

/// Build a minimal workspace Cargo.toml with the given version.
/// The layout matches the real file: version appears in both
/// `[workspace.package]` and the `vibecheck-core` workspace dependency.
fn make_toml(version: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    write!(
        f,
        "[workspace]\n\
         members = [\"vibecheck-core\", \"vibecheck-cli\"]\n\
         \n\
         [workspace.package]\n\
         version = \"{version}\"\n\
         edition = \"2021\"\n\
         \n\
         [workspace.dependencies]\n\
         serde = {{ version = \"1\", features = [\"derive\"] }}\n\
         vibecheck-core = {{ path = \"vibecheck-core\", version = \"{version}\" }}\n",
    )
    .unwrap();
    f
}

/// Run the script against a fresh temp Cargo.toml.
/// Returns `(Output, NamedTempFile)` — keep the file alive to read its contents.
fn run(level: &str, version: &str) -> (Output, NamedTempFile) {
    let toml = make_toml(version);
    let out = Command::new(bump_script())
        .arg(level)
        .env("BUMP_TOML", toml.path())
        .env("BUMP_SKIP_BUILD", "1")
        .output()
        .expect("failed to spawn bump-version.sh — is it executable?");
    (out, toml)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn file_content(f: &NamedTempFile) -> String {
    fs::read_to_string(f.path()).unwrap()
}

// ---------------------------------------------------------------------------
// Version arithmetic
// ---------------------------------------------------------------------------

#[test]
fn patch_increments_patch() {
    let (out, toml) = run("patch", "1.2.3");
    assert!(out.status.success());
    assert!(file_content(&toml).contains(r#"version = "1.2.4""#));
}

#[test]
fn minor_increments_minor_and_resets_patch() {
    let (out, toml) = run("minor", "1.2.3");
    assert!(out.status.success());
    let body = file_content(&toml);
    assert!(body.contains(r#"version = "1.3.0""#));
    assert!(!body.contains(r#"version = "1.2.3""#));
}

#[test]
fn major_increments_major_and_resets_minor_and_patch() {
    let (out, toml) = run("major", "1.2.3");
    assert!(out.status.success());
    assert!(file_content(&toml).contains(r#"version = "2.0.0""#));
}

#[test]
fn minor_produces_double_digit_minor() {
    // Exercises the case where the minor component overflows into two digits.
    let (out, toml) = run("minor", "0.9.9");
    assert!(out.status.success());
    assert!(file_content(&toml).contains(r#"version = "0.10.0""#));
}

#[test]
fn major_promotes_from_zero_series() {
    let (out, toml) = run("major", "0.3.0");
    assert!(out.status.success());
    assert!(file_content(&toml).contains(r#"version = "1.0.0""#));
}

#[test]
fn patch_from_absolute_zero() {
    let (out, toml) = run("patch", "0.0.0");
    assert!(out.status.success());
    assert!(file_content(&toml).contains(r#"version = "0.0.1""#));
}

// ---------------------------------------------------------------------------
// File mutation correctness
// ---------------------------------------------------------------------------

#[test]
fn both_version_occurrences_updated() {
    // The real Cargo.toml has the version in [workspace.package] AND in the
    // vibecheck-core workspace dep — both must be bumped.
    let (out, toml) = run("minor", "1.2.3");
    assert!(out.status.success());
    let body = file_content(&toml);
    assert_eq!(
        body.matches(r#"version = "1.3.0""#).count(),
        2,
        "expected exactly 2 occurrences updated:\n{body}"
    );
}

#[test]
fn old_version_absent_after_bump() {
    let (out, toml) = run("patch", "2.4.6");
    assert!(out.status.success());
    assert!(
        !file_content(&toml).contains(r#"version = "2.4.6""#),
        "stale old version still present after bump"
    );
}

#[test]
fn unrelated_dep_version_strings_untouched() {
    // `serde = { version = "1", ... }` must survive the replacement.
    let (out, toml) = run("patch", "1.0.0");
    assert!(out.status.success());
    assert!(
        file_content(&toml).contains(r#"version = "1""#),
        "unrelated dep version was corrupted by replacement"
    );
}

// ---------------------------------------------------------------------------
// stdout content
// ---------------------------------------------------------------------------

#[test]
fn stdout_contains_old_to_new_arrow() {
    let (out, _toml) = run("minor", "3.1.4");
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("3.1.4 → 3.2.0"),
        "stdout missing transition arrow:\n{}",
        stdout(&out)
    );
}

#[test]
fn stdout_contains_bump_level_label() {
    let (out, _toml) = run("major", "1.0.0");
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("(major)"),
        "stdout missing level label:\n{}",
        stdout(&out)
    );
}

#[test]
fn stdout_contains_commit_hint_with_new_version() {
    let (out, _toml) = run("patch", "0.3.0");
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("chore: bump to v0.3.1"),
        "stdout missing commit hint:\n{}",
        stdout(&out)
    );
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn no_arguments_exits_nonzero() {
    let out = Command::new(bump_script())
        .env("BUMP_SKIP_BUILD", "1")
        .output()
        .unwrap();
    assert!(!out.status.success(), "expected non-zero exit with no arguments");
}

#[test]
fn invalid_level_exits_nonzero() {
    let out = Command::new(bump_script())
        .arg("bogus")
        .env("BUMP_SKIP_BUILD", "1")
        .output()
        .unwrap();
    assert!(!out.status.success(), "expected non-zero exit for unrecognised level 'bogus'");
}

#[test]
fn missing_cargo_toml_exits_nonzero() {
    let out = Command::new(bump_script())
        .arg("patch")
        .env("BUMP_TOML", "/nonexistent/path/Cargo.toml")
        .env("BUMP_SKIP_BUILD", "1")
        .output()
        .unwrap();
    assert!(!out.status.success(), "expected non-zero exit when BUMP_TOML file is missing");
}

#[test]
fn cargo_toml_with_no_version_line_exits_nonzero() {
    let mut f = NamedTempFile::new().unwrap();
    write!(f, "[workspace]\nmembers = []\n").unwrap();
    let out = Command::new(bump_script())
        .arg("patch")
        .env("BUMP_TOML", f.path())
        .env("BUMP_SKIP_BUILD", "1")
        .output()
        .unwrap();
    assert!(!out.status.success(), "expected non-zero exit when no version line present");
}
