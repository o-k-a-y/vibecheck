//! Heuristics configuration system for vibecheck signal weights.
//!
//! The central abstraction is the [`HeuristicsProvider`] trait — the pipeline
//! depends only on the trait, enabling full dependency injection for testing
//! and TOML-configured overrides.
//!
//! # Production use
//! [`ConfiguredHeuristics`] is the production implementation.  It reads a
//! `[heuristics]` table from the `.vibecheck` config file and falls back to
//! [`DefaultHeuristics`] for any signal not explicitly overridden.
//!
//! # Testing / DI
//! [`InertHeuristics`] is a lightweight test double that always returns
//! defaults — analogous to [`super::ignore_rules::AllowAll`].

use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::language::Language;
use crate::report::ModelFamily;

// ---------------------------------------------------------------------------
// HeuristicLanguage — type-safe language scope for heuristic specs
// ---------------------------------------------------------------------------

/// Language scope for a [`HeuristicSpec`].
///
/// Covers both text-analyzer languages and CST-analyzer variants.
/// `Display` output matches the string values previously stored in the
/// `language` field — output is identical to before this enum was added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeuristicLanguage {
    /// Rust text analyzer signals.
    Rust,
    /// Python text analyzer signals.
    Python,
    /// JavaScript / TypeScript text analyzer signals.
    Js,
    /// Go text analyzer signals.
    Go,
    /// Rust CST analyzer signals.
    RustCst,
    /// Python CST analyzer signals.
    PythonCst,
    /// JavaScript / TypeScript CST analyzer signals.
    JsCst,
    /// Go CST analyzer signals.
    GoCst,
    /// Language-agnostic signals.
    All,
}

impl fmt::Display for HeuristicLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HeuristicLanguage::Rust      => "rust",
            HeuristicLanguage::Python    => "python",
            HeuristicLanguage::Js        => "js",
            HeuristicLanguage::Go        => "go",
            HeuristicLanguage::RustCst   => "rust_cst",
            HeuristicLanguage::PythonCst => "python_cst",
            HeuristicLanguage::JsCst     => "js_cst",
            HeuristicLanguage::GoCst     => "go_cst",
            HeuristicLanguage::All       => "all",
        })
    }
}

impl From<Language> for HeuristicLanguage {
    fn from(lang: Language) -> Self {
        match lang {
            Language::Rust       => HeuristicLanguage::Rust,
            Language::Python     => HeuristicLanguage::Python,
            Language::JavaScript => HeuristicLanguage::Js,
            Language::Go         => HeuristicLanguage::Go,
        }
    }
}

// ---------------------------------------------------------------------------
// HeuristicSpec — single signal entry
// ---------------------------------------------------------------------------

/// Metadata for a single named heuristic signal.
#[derive(Debug, Clone)]
pub struct HeuristicSpec {
    /// Stable dot-separated identifier, e.g. `"rust.errors.zero_unwrap"`.
    pub id: &'static str,
    /// Language scope of this signal.
    pub language: HeuristicLanguage,
    /// Analyzer that emits this signal.
    pub analyzer: &'static str,
    /// Short human-readable description of what this signal detects.
    pub description: &'static str,
    /// Primary attribution family this signal points toward.
    pub family: ModelFamily,
    /// Default weight (positive = toward `family`, 0.0 = disabled).
    pub default_weight: f64,
}

// ---------------------------------------------------------------------------
// TOML deserialization helper
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RawSignalDef {
    id: String,
    language: HeuristicLanguage,
    analyzer: String,
    description: String,
    family: ModelFamily,
    weight: f64,
}

#[derive(Deserialize)]
struct Manifest {
    signal: Vec<RawSignalDef>,
}

// ---------------------------------------------------------------------------
// all_heuristics() — lazily parsed from embedded TOML
// ---------------------------------------------------------------------------

static HEURISTICS: OnceLock<Vec<HeuristicSpec>> = OnceLock::new();

/// Complete table of every signal emitted by vibecheck's analyzers.
///
/// Parsed once from the embedded `heuristics.toml` on first access.
/// Use this to render documentation, seed TOML templates, or look up defaults.
pub fn all_heuristics() -> &'static [HeuristicSpec] {
    HEURISTICS.get_or_init(|| {
        let toml_src = include_str!("../heuristics.toml");
        let manifest: Manifest =
            toml::from_str(toml_src).expect("embedded heuristics.toml is invalid");
        manifest
            .signal
            .into_iter()
            .map(|raw| {
                let id: &'static str = Box::leak(raw.id.into_boxed_str());
                let analyzer: &'static str = Box::leak(raw.analyzer.into_boxed_str());
                let description: &'static str = Box::leak(raw.description.into_boxed_str());
                HeuristicSpec {
                    id,
                    language: raw.language,
                    analyzer,
                    description,
                    family: raw.family,
                    default_weight: raw.weight,
                }
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------
// signal_ids — compile-time constants generated by build.rs
// ---------------------------------------------------------------------------

/// Compile-time string constants for every signal ID in [`all_heuristics`].
///
/// Import selectively: `use vibecheck_core::heuristics::signal_ids;`
pub mod signal_ids {
    include!(concat!(env!("OUT_DIR"), "/signal_ids.rs"));
}

// ---------------------------------------------------------------------------
// HeuristicsProvider trait
// ---------------------------------------------------------------------------

/// Seam for dependency injection of signal weights.
///
/// Implement this trait to supply custom weights — TOML configuration,
/// in-memory overrides for tests, etc. — without touching the pipeline.
pub trait HeuristicsProvider: Send + Sync {
    /// Return the effective weight for the given signal ID.
    ///
    /// Return `0.0` to disable the signal entirely.
    fn weight(&self, id: &str) -> f64;

    /// Return `false` if this signal should be suppressed from output.
    ///
    /// Defaults to `weight(id) != 0.0`.
    fn is_enabled(&self, id: &str) -> bool {
        self.weight(id) != 0.0
    }
}

// ---------------------------------------------------------------------------
// DefaultHeuristics — looks up all_heuristics() table
// ---------------------------------------------------------------------------

/// Uses the hardcoded defaults from [`all_heuristics`].
///
/// This is the implementation used by [`Pipeline::with_defaults`].
pub struct DefaultHeuristics;

impl HeuristicsProvider for DefaultHeuristics {
    fn weight(&self, id: &str) -> f64 {
        all_heuristics()
            .iter()
            .find(|h| h.id == id)
            .map(|h| h.default_weight)
            .unwrap_or(1.0) // unknown signals pass through at weight 1.0
    }
}

// ---------------------------------------------------------------------------
// InertHeuristics — test double (always uses defaults)
// ---------------------------------------------------------------------------

/// Test double: delegates everything to [`DefaultHeuristics`].
///
/// Use this in integration tests to verify that the pipeline produces the
/// expected signals without any weight overrides.  Analogous to
/// [`super::ignore_rules::AllowAll`].
pub struct InertHeuristics;

impl HeuristicsProvider for InertHeuristics {
    fn weight(&self, id: &str) -> f64 {
        DefaultHeuristics.weight(id)
    }
}

// ---------------------------------------------------------------------------
// ConfiguredHeuristics — TOML-loaded overrides
// ---------------------------------------------------------------------------

/// Heuristics loaded from the `[heuristics]` table in `.vibecheck`.
///
/// Any signal not present in the overrides map falls back to
/// [`DefaultHeuristics`].
pub struct ConfiguredHeuristics {
    overrides: HashMap<String, f64>,
}

impl ConfiguredHeuristics {
    /// Build from a map of signal-ID → weight overrides (e.g. parsed from
    /// the `[heuristics]` TOML section).
    pub fn from_config(overrides: HashMap<String, f64>) -> Self {
        Self { overrides }
    }

    /// Returns `true` if no overrides are configured (fast path: use defaults).
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

impl HeuristicsProvider for ConfiguredHeuristics {
    fn weight(&self, id: &str) -> f64 {
        self.overrides
            .get(id)
            .copied()
            .unwrap_or_else(|| DefaultHeuristics.weight(id))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_toml_parses_successfully() {
        assert!(!all_heuristics().is_empty());
    }

    #[test]
    fn all_heuristics_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for h in all_heuristics() {
            assert!(seen.insert(h.id), "duplicate heuristic id: {}", h.id);
        }
    }

    #[test]
    fn signal_id_constants_match_all_heuristics() {
        let ids: std::collections::HashSet<&str> =
            all_heuristics().iter().map(|h| h.id).collect();
        assert!(ids.contains(signal_ids::RUST_ERRORS_ZERO_UNWRAP));
        assert!(ids.contains(signal_ids::PYTHON_AI_SIGNALS_NO_TODO));
        assert!(ids.contains(signal_ids::GO_CST_ERRORS_NIL_CHECKS));
        assert!(ids.contains(signal_ids::JS_CST_ARROW_FNS_HIGH_RATIO));
    }

    #[test]
    fn default_heuristics_returns_correct_weight() {
        let h = DefaultHeuristics;
        assert_eq!(h.weight(signal_ids::RUST_ERRORS_ZERO_UNWRAP), 1.5);
        assert_eq!(h.weight(signal_ids::RUST_AI_SIGNALS_COMMENTED_OUT_CODE), 2.0);
        assert_eq!(h.weight(signal_ids::RUST_CST_COMPLEXITY_LOW), 1.5);
    }

    #[test]
    fn default_heuristics_unknown_id_returns_one() {
        let h = DefaultHeuristics;
        assert_eq!(h.weight("this.does.not.exist"), 1.0);
    }

    #[test]
    fn configured_heuristics_override_applies() {
        let mut overrides = HashMap::new();
        overrides.insert("rust.errors.zero_unwrap".to_string(), 3.0);
        let h = ConfiguredHeuristics::from_config(overrides);
        assert_eq!(h.weight(signal_ids::RUST_ERRORS_ZERO_UNWRAP), 3.0);
        assert_eq!(h.weight(signal_ids::RUST_ERRORS_MANY_UNWRAPS), 1.5);
    }

    #[test]
    fn configured_heuristics_zero_disables() {
        let mut overrides = HashMap::new();
        overrides.insert("rust.errors.zero_unwrap".to_string(), 0.0);
        let h = ConfiguredHeuristics::from_config(overrides);
        assert!(!h.is_enabled(signal_ids::RUST_ERRORS_ZERO_UNWRAP));
        assert!(h.is_enabled(signal_ids::RUST_ERRORS_MANY_UNWRAPS));
    }

    #[test]
    fn no_family_exceeds_35_percent() {
        let mut counts: std::collections::HashMap<ModelFamily, usize> = std::collections::HashMap::new();
        for h in all_heuristics() {
            *counts.entry(h.family).or_default() += 1;
        }
        let total = all_heuristics().len();
        for (fam, count) in &counts {
            assert!(
                (*count as f64 / total as f64) <= 0.35,
                "{fam:?} has {count}/{total} signals ({:.1}%), exceeds 35%",
                *count as f64 / total as f64 * 100.0
            );
        }
    }

    #[test]
    fn inert_heuristics_matches_defaults() {
        let inert = InertHeuristics;
        let def = DefaultHeuristics;
        for spec in all_heuristics() {
            assert_eq!(
                inert.weight(spec.id),
                def.weight(spec.id),
                "mismatch for {}",
                spec.id
            );
        }
    }
}
