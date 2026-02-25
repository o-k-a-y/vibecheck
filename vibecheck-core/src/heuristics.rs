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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
// ALL_HEURISTICS — complete catalogue
// ---------------------------------------------------------------------------

macro_rules! h {
    ($id:expr, $lang:expr, $ana:expr, $desc:expr, $fam:expr, $w:expr) => {
        HeuristicSpec {
            id: $id,
            language: $lang,
            analyzer: $ana,
            description: $desc,
            family: $fam,
            default_weight: $w,
        }
    };
}

/// Complete table of every signal emitted by vibecheck's analyzers.
///
/// Use this to render documentation, seed TOML templates, or look up defaults.
pub static ALL_HEURISTICS: &[HeuristicSpec] = &[
    // ── rust · errors ────────────────────────────────────────────────────
    h!("rust.errors.zero_unwrap", HeuristicLanguage::Rust, "errors", "Zero .unwrap() calls in a large file",                         ModelFamily::Claude,  1.5),
    h!("rust.errors.many_unwraps", HeuristicLanguage::Rust, "errors", "5+ .unwrap() calls — pragmatic style",                         ModelFamily::Human,   1.5),
    h!("rust.errors.few_unwraps", HeuristicLanguage::Rust, "errors", "1–3 .unwrap() calls — moderate usage",                         ModelFamily::Copilot, 0.5),
    h!("rust.errors.expect_calls", HeuristicLanguage::Rust, "errors", "2+ .expect() calls — descriptive error handling",              ModelFamily::Claude,  1.0),
    h!("rust.errors.question_mark", HeuristicLanguage::Rust, "errors", "3+ uses of the ? operator",                                    ModelFamily::Claude,  1.0),
    h!("rust.errors.exhaustive_match", HeuristicLanguage::Rust, "errors", "Match expressions prefer exhaustive patterns over wildcards",  ModelFamily::Claude,  1.0),
    h!("rust.errors.panic_calls", HeuristicLanguage::Rust, "errors", "2+ panic!() calls",                                            ModelFamily::Human,   1.5),

    // ── python · errors ──────────────────────────────────────────────────
    h!("python.errors.broad_except", HeuristicLanguage::Python, "errors", "2+ broad except clauses — swallows all exceptions",          ModelFamily::Human,  1.5),
    h!("python.errors.specific_except", HeuristicLanguage::Python, "errors", "2+ specific exception types — precise error handling",       ModelFamily::Claude, 1.0),
    h!("python.errors.no_try_except", HeuristicLanguage::Python, "errors", "No try/except blocks in a substantial file",                 ModelFamily::Claude, 0.8),
    h!("python.errors.raise_from", HeuristicLanguage::Python, "errors", "2+ raise…from patterns — idiomatic exception chaining",     ModelFamily::Claude, 1.0),

    // ── js · errors ──────────────────────────────────────────────────────
    h!("js.errors.console_error", HeuristicLanguage::Js, "errors", "2+ console.error/warn calls — debug artifacts",          ModelFamily::Human,  1.0),
    h!("js.errors.typed_error_check", HeuristicLanguage::Js, "errors", "2+ instanceof Error checks — typed error handling",       ModelFamily::Claude, 1.0),
    h!("js.errors.promise_catch", HeuristicLanguage::Js, "errors", "2+ .catch() chains — promise-based error handling",       ModelFamily::Human,  0.8),
    h!("js.errors.try_catch_blocks", HeuristicLanguage::Js, "errors", "2+ try/catch blocks — structured async error handling",   ModelFamily::Claude, 0.8),
    h!("js.errors.typed_error_construction", HeuristicLanguage::Js, "errors", "2+ typed Error constructions — specific error classes",   ModelFamily::Claude, 0.8),

    // ── go · errors ──────────────────────────────────────────────────────
    h!("go.errors.simple_err_return", HeuristicLanguage::Go, "errors", "3+ simple 'if err != nil' returns",                     ModelFamily::Human,  0.8),
    h!("go.errors.errorf_wrap", HeuristicLanguage::Go, "errors", "2+ fmt.Errorf(%w) wrappings — idiomatic error context",  ModelFamily::Claude, 1.0),
    h!("go.errors.errors_sentinel", HeuristicLanguage::Go, "errors", "2+ errors.Is/As calls — structured error inspection",    ModelFamily::Claude, 1.2),
    h!("go.errors.panic_calls", HeuristicLanguage::Go, "errors", "2+ panic() calls — non-recoverable or human shortcut",   ModelFamily::Human,  1.5),

    // ── rust · ai_signals ────────────────────────────────────────────────
    h!("rust.ai_signals.no_todo", HeuristicLanguage::Rust, "ai_signals", "No TODO/FIXME markers in a substantial file",         ModelFamily::Claude, 0.8),
    h!("rust.ai_signals.no_dead_code", HeuristicLanguage::Rust, "ai_signals", "No dead code suppressions (#[allow(dead_code)])",     ModelFamily::Claude, 0.5),
    h!("rust.ai_signals.all_fns_documented", HeuristicLanguage::Rust, "ai_signals", "Every function has a doc comment — suspiciously thorough", ModelFamily::Claude, 2.0),
    h!("rust.ai_signals.no_trailing_ws", HeuristicLanguage::Rust, "ai_signals", "Zero trailing whitespace — machine-perfect formatting", ModelFamily::Gpt,   0.5),
    h!("rust.ai_signals.commented_out_code", HeuristicLanguage::Rust, "ai_signals", "2+ lines of commented-out code",                      ModelFamily::Human,  2.0),
    h!("rust.ai_signals.no_placeholder", HeuristicLanguage::Rust, "ai_signals", "No placeholder values — polished code",               ModelFamily::Gpt,    0.3),

    // ── python · ai_signals ──────────────────────────────────────────────
    h!("python.ai_signals.no_todo", HeuristicLanguage::Python, "ai_signals", "No TODO/FIXME markers in a substantial file",             ModelFamily::Claude, 0.8),
    h!("python.ai_signals.no_trailing_ws", HeuristicLanguage::Python, "ai_signals", "Zero trailing whitespace — machine-perfect formatting",   ModelFamily::Gpt,   0.5),
    h!("python.ai_signals.no_placeholder", HeuristicLanguage::Python, "ai_signals", "No placeholder values — polished code",                  ModelFamily::Gpt,    0.3),
    h!("python.ai_signals.no_linter_suppression", HeuristicLanguage::Python, "ai_signals", "No noqa/type: ignore suppressions",                      ModelFamily::Claude, 0.5),
    h!("python.ai_signals.commented_out_code", HeuristicLanguage::Python, "ai_signals", "2+ lines of commented-out code",                        ModelFamily::Human,  2.0),
    h!("python.ai_signals.all_fns_documented", HeuristicLanguage::Python, "ai_signals", "Every function has a docstring — suspiciously thorough", ModelFamily::Claude, 2.0),

    // ── js · ai_signals ──────────────────────────────────────────────────
    h!("js.ai_signals.no_todo", HeuristicLanguage::Js, "ai_signals", "No TODO/FIXME markers in a substantial file",              ModelFamily::Claude, 0.8),
    h!("js.ai_signals.no_trailing_ws", HeuristicLanguage::Js, "ai_signals", "Zero trailing whitespace — machine-perfect formatting",    ModelFamily::Gpt,   0.5),
    h!("js.ai_signals.no_placeholder", HeuristicLanguage::Js, "ai_signals", "No placeholder values — polished code",                   ModelFamily::Gpt,    0.3),
    h!("js.ai_signals.no_linter_suppression", HeuristicLanguage::Js, "ai_signals", "No eslint-disable / @ts-ignore suppressions",             ModelFamily::Claude, 0.5),
    h!("js.ai_signals.commented_out_code", HeuristicLanguage::Js, "ai_signals", "2+ lines of commented-out code",                         ModelFamily::Human,  2.0),
    h!("js.ai_signals.jsdoc_blocks", HeuristicLanguage::Js, "ai_signals", "3+ JSDoc comment blocks — thorough documentation",       ModelFamily::Claude, 1.5),
    h!("js.ai_signals.console_log", HeuristicLanguage::Js, "ai_signals", "3+ console.log calls — likely debugging artifacts",      ModelFamily::Human,  2.0),

    // ── go · ai_signals ──────────────────────────────────────────────────
    h!("go.ai_signals.no_todo", HeuristicLanguage::Go, "ai_signals", "No TODO/FIXME markers in a substantial file",              ModelFamily::Claude, 0.8),
    h!("go.ai_signals.no_trailing_ws", HeuristicLanguage::Go, "ai_signals", "Zero trailing whitespace — machine-perfect formatting",    ModelFamily::Gpt,   0.5),
    h!("go.ai_signals.no_placeholder", HeuristicLanguage::Go, "ai_signals", "No placeholder values — polished code",                   ModelFamily::Gpt,    0.3),
    h!("go.ai_signals.no_nolint", HeuristicLanguage::Go, "ai_signals", "No nolint suppressions — clean linter compliance",        ModelFamily::Claude, 0.5),
    h!("go.ai_signals.commented_out_code", HeuristicLanguage::Go, "ai_signals", "2+ lines of commented-out code",                         ModelFamily::Human,  2.0),
    h!("go.ai_signals.all_exported_documented", HeuristicLanguage::Go, "ai_signals","All exported identifiers have doc comments",               ModelFamily::Claude, 2.0),

    // ── rust · structure ─────────────────────────────────────────────────
    h!("rust.structure.high_type_annotation", HeuristicLanguage::Rust, "structure", "High type annotation ratio on let bindings",           ModelFamily::Gpt,    1.0),
    h!("rust.structure.low_type_annotation", HeuristicLanguage::Rust, "structure", "Relies on type inference — minimal annotations",       ModelFamily::Gemini, 0.8),
    h!("rust.structure.sorted_imports", HeuristicLanguage::Rust, "structure", "Import statements are alphabetically sorted",          ModelFamily::Gpt,    0.5),
    h!("rust.structure.consistent_blank_lines", HeuristicLanguage::Rust, "structure", "Perfectly consistent blank line spacing",               ModelFamily::Gemini, 0.5),
    h!("rust.structure.lines_under_100", HeuristicLanguage::Rust, "structure", "All lines ≤100 chars — disciplined formatting",        ModelFamily::Gemini, 0.4),
    h!("rust.structure.many_long_lines", HeuristicLanguage::Rust, "structure", "5+ lines over 100 chars",                              ModelFamily::Human,  1.0),
    h!("rust.structure.heavy_derive", HeuristicLanguage::Rust, "structure", "Heavy derive usage (avg ≥4 traits per derive)",        ModelFamily::Gpt, 1.0),

    // ── python · structure ───────────────────────────────────────────────
    h!("python.structure.sorted_imports", HeuristicLanguage::Python, "structure", "Import statements are alphabetically sorted",          ModelFamily::Gpt,    0.5),
    h!("python.structure.consistent_blank_lines", HeuristicLanguage::Python, "structure", "Perfectly consistent blank line spacing",               ModelFamily::Gemini, 0.5),
    h!("python.structure.lines_under_88", HeuristicLanguage::Python, "structure", "All lines ≤88 chars — PEP 8 / Black-style discipline", ModelFamily::Gemini, 0.4),

    // ── js · structure ───────────────────────────────────────────────────
    h!("js.structure.sorted_imports", HeuristicLanguage::Js, "structure", "Import statements are alphabetically sorted",     ModelFamily::Gpt,    0.5),
    h!("js.structure.consistent_blank_lines", HeuristicLanguage::Js, "structure", "Perfectly consistent blank line spacing",         ModelFamily::Gemini, 0.5),
    h!("js.structure.lines_under_100", HeuristicLanguage::Js, "structure", "All lines ≤100 chars — disciplined formatting",  ModelFamily::Gemini, 0.4),
    h!("js.structure.many_long_lines", HeuristicLanguage::Js, "structure", "5+ lines over 100 chars",                        ModelFamily::Human,  1.0),

    // ── go · structure ───────────────────────────────────────────────────
    h!("go.structure.sorted_imports", HeuristicLanguage::Go, "structure", "Import strings are sorted — goimports-style",          ModelFamily::Gpt,    0.5),
    h!("go.structure.consistent_blank_lines", HeuristicLanguage::Go, "structure", "Perfectly consistent blank line spacing",               ModelFamily::Gemini, 0.5),
    h!("go.structure.lines_under_120", HeuristicLanguage::Go, "structure", "All lines ≤120 chars — gofmt-style discipline",        ModelFamily::Gemini, 0.4),

    // ── rust · comments ──────────────────────────────────────────────────
    h!("rust.comments.high_density", HeuristicLanguage::Rust, "comments", "High comment density (>15%)",                              ModelFamily::Claude, 1.5),
    h!("rust.comments.low_density", HeuristicLanguage::Rust, "comments", "Very low comment density (<3%)",                           ModelFamily::Human,  1.0),
    h!("rust.comments.teaching_voice", HeuristicLanguage::Rust, "comments", "3+ comments with teaching/explanatory voice",              ModelFamily::Claude, 1.5),
    h!("rust.comments.some_explanatory", HeuristicLanguage::Rust, "comments", "Some explanatory comments present",                        ModelFamily::Gpt,   0.8),
    h!("rust.comments.doc_comments", HeuristicLanguage::Rust, "comments", "5+ doc comments (///) — thorough documentation",          ModelFamily::Claude, 1.5),
    h!("rust.comments.terse_markers", HeuristicLanguage::Rust, "comments", "2+ terse/frustrated comments (TODO, HACK, etc.)",         ModelFamily::Human,  2.0),

    // ── python · comments ────────────────────────────────────────────────
    h!("python.comments.high_density", HeuristicLanguage::Python, "comments", "High comment density (>15%)",                             ModelFamily::Claude, 1.5),
    h!("python.comments.low_density", HeuristicLanguage::Python, "comments", "Very low comment density (<3%)",                          ModelFamily::Human,  1.0),
    h!("python.comments.teaching_voice", HeuristicLanguage::Python, "comments", "3+ comments with teaching/explanatory voice",             ModelFamily::Claude, 1.5),
    h!("python.comments.some_explanatory", HeuristicLanguage::Python, "comments", "Some explanatory comments present",                       ModelFamily::Gpt,   0.8),
    h!("python.comments.docstring_blocks", HeuristicLanguage::Python, "comments", "5+ docstring blocks — thorough documentation",            ModelFamily::Claude, 1.5),
    h!("python.comments.terse_markers", HeuristicLanguage::Python, "comments", "2+ terse/frustrated comments",                           ModelFamily::Human,  2.0),

    // ── js · comments ────────────────────────────────────────────────────
    h!("js.comments.high_density", HeuristicLanguage::Js, "comments", "High comment density (>15%)",                                 ModelFamily::Claude, 1.5),
    h!("js.comments.low_density", HeuristicLanguage::Js, "comments", "Very low comment density (<3%)",                              ModelFamily::Human,  1.0),
    h!("js.comments.teaching_voice", HeuristicLanguage::Js, "comments", "3+ comments with teaching/explanatory voice",                 ModelFamily::Claude, 1.5),
    h!("js.comments.some_explanatory", HeuristicLanguage::Js, "comments", "Some explanatory comments present",                           ModelFamily::Gpt,   0.8),
    h!("js.comments.terse_markers", HeuristicLanguage::Js, "comments", "2+ terse/frustrated comments (TODO, HACK, etc.)",            ModelFamily::Human,  2.0),
    h!("js.comments.jsdoc_blocks", HeuristicLanguage::Js, "comments", "5+ JSDoc comment blocks — thorough API documentation",       ModelFamily::Claude, 1.5),

    // ── go · comments ────────────────────────────────────────────────────
    h!("go.comments.high_density", HeuristicLanguage::Go, "comments", "High comment density (>15%)",                                 ModelFamily::Claude, 1.5),
    h!("go.comments.low_density", HeuristicLanguage::Go, "comments", "Very low comment density (<3%)",                              ModelFamily::Human,  1.0),
    h!("go.comments.teaching_voice", HeuristicLanguage::Go, "comments", "3+ comments with teaching/explanatory voice",                 ModelFamily::Claude, 1.5),
    h!("go.comments.some_explanatory", HeuristicLanguage::Go, "comments", "Some explanatory comments present",                           ModelFamily::Gpt,   0.8),
    h!("go.comments.terse_markers", HeuristicLanguage::Go, "comments", "2+ terse/frustrated comments (TODO, HACK, etc.)",            ModelFamily::Human,  2.0),

    // ── rust · naming ────────────────────────────────────────────────────
    h!("rust.naming.very_descriptive_vars", HeuristicLanguage::Rust, "naming", "Very descriptive variable names (avg >12 chars)",            ModelFamily::Claude, 1.5),
    h!("rust.naming.descriptive_vars", HeuristicLanguage::Rust, "naming", "Descriptive variable names (avg 8–12 chars)",               ModelFamily::Gpt,   1.0),
    h!("rust.naming.short_vars", HeuristicLanguage::Rust, "naming", "Short variable names (avg <4 chars)",                       ModelFamily::Human,  1.5),
    h!("rust.naming.many_single_char_vars", HeuristicLanguage::Rust, "naming", "3+ single-character variable names",                        ModelFamily::Human,  2.0),
    h!("rust.naming.no_single_char_vars", HeuristicLanguage::Rust, "naming", "No single-character variable names",                        ModelFamily::Claude, 1.0),
    h!("rust.naming.underscore_bindings", HeuristicLanguage::Rust, "naming", "2+ underscore-prefixed bindings (acknowledging unused)",    ModelFamily::Human,  1.0),
    h!("rust.naming.descriptive_fn_names", HeuristicLanguage::Rust, "naming", "Very descriptive function names (avg >15 chars)",           ModelFamily::Claude, 1.0),

    // ── python · naming ──────────────────────────────────────────────────
    h!("python.naming.very_descriptive", HeuristicLanguage::Python, "naming", "Very descriptive names (avg >12 chars)",  ModelFamily::Claude, 1.5),
    h!("python.naming.descriptive", HeuristicLanguage::Python, "naming", "Descriptive names (avg 8–12 chars)",      ModelFamily::Gpt,   1.0),
    h!("python.naming.short_names", HeuristicLanguage::Python, "naming", "Short names (avg <4 chars)",              ModelFamily::Human,  1.5),
    h!("python.naming.many_single_char", HeuristicLanguage::Python, "naming", "3+ single-character names",               ModelFamily::Human,  2.0),
    h!("python.naming.no_single_char", HeuristicLanguage::Python, "naming", "No single-character names",               ModelFamily::Claude, 1.0),

    // ── js · naming ──────────────────────────────────────────────────────
    h!("js.naming.very_descriptive", HeuristicLanguage::Js, "naming", "Very descriptive names (avg >12 chars)",  ModelFamily::Claude, 1.5),
    h!("js.naming.descriptive", HeuristicLanguage::Js, "naming", "Descriptive names (avg 8–12 chars)",      ModelFamily::Gpt,   1.0),
    h!("js.naming.short_names", HeuristicLanguage::Js, "naming", "Short names (avg <4 chars)",              ModelFamily::Human,  1.5),
    h!("js.naming.many_single_char", HeuristicLanguage::Js, "naming", "3+ single-character names",               ModelFamily::Human,  2.0),
    h!("js.naming.no_single_char", HeuristicLanguage::Js, "naming", "No single-character names",               ModelFamily::Claude, 1.0),

    // ── go · naming ──────────────────────────────────────────────────────
    h!("go.naming.very_descriptive", HeuristicLanguage::Go, "naming", "Very descriptive names (avg >12 chars)",  ModelFamily::Claude, 1.5),
    h!("go.naming.descriptive", HeuristicLanguage::Go, "naming", "Descriptive names (avg 8–12 chars)",      ModelFamily::Gpt,   1.0),
    h!("go.naming.short_names", HeuristicLanguage::Go, "naming", "Short names (avg <4 chars)",              ModelFamily::Human,  1.5),
    h!("go.naming.many_single_char", HeuristicLanguage::Go, "naming", "3+ single-character names",               ModelFamily::Human,  2.0),
    h!("go.naming.no_single_char", HeuristicLanguage::Go, "naming", "No single-character names",               ModelFamily::Claude, 1.0),

    // ── rust · idioms ────────────────────────────────────────────────────
    h!("rust.idioms.iterator_chains", HeuristicLanguage::Rust, "idioms", "5+ iterator chain usages — textbook-idiomatic Rust",         ModelFamily::Claude, 1.5),
    h!("rust.idioms.builder_pattern", HeuristicLanguage::Rust, "idioms", "8+ method chain continuation lines — builder pattern",      ModelFamily::Gpt,   1.0),
    h!("rust.idioms.impl_display", HeuristicLanguage::Rust, "idioms", "Implements Display trait — thorough API design",             ModelFamily::Claude, 1.0),
    h!("rust.idioms.from_into_impls", HeuristicLanguage::Rust, "idioms", "2+ From/Into implementations — conversion-rich design",     ModelFamily::Claude, 1.0),
    h!("rust.idioms.self_usage", HeuristicLanguage::Rust, "idioms", "3+ uses of Self — consistent self-referencing",             ModelFamily::Claude, 0.8),
    h!("rust.idioms.pattern_matching", HeuristicLanguage::Rust, "idioms", "3+ if-let/while-let patterns",                              ModelFamily::Claude, 0.8),
    h!("rust.idioms.format_macro", HeuristicLanguage::Rust, "idioms", "Uses format!() exclusively, no string concatenation",       ModelFamily::Claude, 0.8),
    h!("rust.idioms.string_concat", HeuristicLanguage::Rust, "idioms", "3+ string concatenations — less idiomatic",                 ModelFamily::Human,  1.0),
    h!("rust.idioms.many_traits", HeuristicLanguage::Rust, "idioms", "3+ trait definitions — heavy abstraction",                  ModelFamily::Gpt,   1.5),

    // ── python · idioms ──────────────────────────────────────────────────
    h!("python.idioms.comprehensions", HeuristicLanguage::Python, "idioms", "3+ list/dict/set comprehensions — pythonic style",              ModelFamily::Claude, 1.5),
    h!("python.idioms.return_type_annotations", HeuristicLanguage::Python, "idioms", "All function definitions have return type annotations",         ModelFamily::Claude, 1.5),
    h!("python.idioms.context_managers", HeuristicLanguage::Python, "idioms", "2+ context manager usages (with statement)",                    ModelFamily::Claude, 0.8),
    h!("python.idioms.functional_builtins", HeuristicLanguage::Python, "idioms", "4+ functional builtin usages — idiomatic Python",              ModelFamily::Claude, 1.0),
    h!("python.idioms.fstrings", HeuristicLanguage::Python, "idioms", "Uses f-strings exclusively — modern string formatting",        ModelFamily::Claude, 0.8),
    h!("python.idioms.old_format", HeuristicLanguage::Python, "idioms", "3+ old-style format calls — legacy string formatting",        ModelFamily::Human,  1.0),

    // ── js · idioms ──────────────────────────────────────────────────────
    h!("js.idioms.arrow_fns_only", HeuristicLanguage::Js, "idioms", "5+ arrow functions, no regular functions — modern ES6+ style", ModelFamily::Claude, 1.5),
    h!("js.idioms.regular_fns_only", HeuristicLanguage::Js, "idioms", "3+ traditional function declarations — older style",           ModelFamily::Human,  1.0),
    h!("js.idioms.var_declarations", HeuristicLanguage::Js, "idioms", "3+ var declarations — legacy hoisting style",                  ModelFamily::Human,  1.5),
    h!("js.idioms.const_declarations", HeuristicLanguage::Js, "idioms", "5+ const declarations — immutability-first approach",          ModelFamily::Copilot, 1.0),
    h!("js.idioms.null_safe_ops", HeuristicLanguage::Js, "idioms", "3+ optional chaining/nullish ops — modern null safety",        ModelFamily::Claude, 1.0),
    h!("js.idioms.destructuring", HeuristicLanguage::Js, "idioms", "3+ destructuring assignments — idiomatic ES6+",                ModelFamily::Gemini, 0.8),
    h!("js.idioms.async_await", HeuristicLanguage::Js, "idioms", "3+ async/await usages — modern asynchronous style",            ModelFamily::Gemini, 0.8),

    // ── go · idioms ──────────────────────────────────────────────────────
    h!("go.idioms.interface_checks", HeuristicLanguage::Go, "idioms", "1+ compile-time interface checks — thorough Go design",      ModelFamily::Claude, 1.5),
    h!("go.idioms.goroutines", HeuristicLanguage::Go, "idioms", "2+ goroutine launches — concurrent design",                  ModelFamily::Gpt,   0.8),
    h!("go.idioms.defer_stmts", HeuristicLanguage::Go, "idioms", "2+ defer statements — idiomatic resource cleanup",           ModelFamily::Gemini, 0.8),
    h!("go.idioms.table_driven_tests", HeuristicLanguage::Go, "idioms", "Table-driven test pattern detected — idiomatic Go testing",  ModelFamily::Claude, 1.5),
    h!("go.idioms.iota_constants", HeuristicLanguage::Go, "idioms", "1+ iota constants — idiomatic Go enumeration",              ModelFamily::Copilot, 0.8),

    // ── rust_cst ─────────────────────────────────────────────────────────
    h!("rust_cst.complexity.low", HeuristicLanguage::RustCst, "rust_cst", "Low avg cyclomatic complexity (≤2.0) — simple, linear functions", ModelFamily::Claude, 1.5),
    h!("rust_cst.complexity.high", HeuristicLanguage::RustCst, "rust_cst", "High avg cyclomatic complexity (≥5.0) — complex branching",       ModelFamily::Human,  1.5),
    h!("rust_cst.doc_coverage.high", HeuristicLanguage::RustCst, "rust_cst", "≥90% doc comment coverage on pub functions",                     ModelFamily::Claude, 1.5),
    h!("rust_cst.entropy.high", HeuristicLanguage::RustCst, "rust_cst", "High identifier entropy (≥4.0) — diverse, descriptive names",    ModelFamily::Claude, 1.5),
    h!("rust_cst.entropy.low", HeuristicLanguage::RustCst, "rust_cst", "Low identifier entropy (<3.0) — repetitive or terse names",      ModelFamily::Human,  1.0),
    h!("rust_cst.nesting.low", HeuristicLanguage::RustCst, "rust_cst", "Low avg nesting depth (≤3.0) — flat, readable structure",        ModelFamily::Claude, 1.5),
    h!("rust_cst.imports.sorted", HeuristicLanguage::RustCst, "rust_cst", "use declarations are alphabetically sorted",                     ModelFamily::Claude, 0.5),

    // ── python_cst ───────────────────────────────────────────────────────
    h!("python_cst.doc_coverage.high", HeuristicLanguage::PythonCst, "python_cst", "≥85% docstring coverage — thorough documentation",              ModelFamily::Claude, 1.5),
    h!("python_cst.type_annotations.high", HeuristicLanguage::PythonCst, "python_cst", "≥80% type annotation coverage on parameters",                   ModelFamily::Claude, 1.5),
    h!("python_cst.fstrings.only", HeuristicLanguage::PythonCst, "python_cst", "f-strings present, no %-formatting — modern Python idiom",      ModelFamily::Claude, 1.0),

    // ── js_cst ───────────────────────────────────────────────────────────
    h!("js_cst.arrow_fns.high_ratio", HeuristicLanguage::JsCst, "js_cst", "≥70% arrow functions — modern JavaScript style",             ModelFamily::Claude, 1.5),
    h!("js_cst.async.await_only", HeuristicLanguage::JsCst, "js_cst", "2+ await expressions, no .then() — modern async style",     ModelFamily::Claude, 1.5),
    h!("js_cst.async.then_only", HeuristicLanguage::JsCst, "js_cst", "2+ .then() chains — promise chain style",                   ModelFamily::Human,  1.0),
    h!("js_cst.optional_chaining.high", HeuristicLanguage::JsCst, "js_cst", "3+ optional chaining usages (?.) — defensive modern style", ModelFamily::Claude, 1.0),

    // ── go_cst ───────────────────────────────────────────────────────────
    h!("go_cst.doc_coverage.high", HeuristicLanguage::GoCst, "go_cst", "≥80% Godoc coverage on exported functions",            ModelFamily::Claude, 1.5),
    h!("go_cst.goroutines.present", HeuristicLanguage::GoCst, "go_cst", "2+ goroutines — concurrent design",                    ModelFamily::Claude, 1.0),
    h!("go_cst.errors.nil_checks", HeuristicLanguage::GoCst, "go_cst", "3+ err != nil checks — thorough error handling",       ModelFamily::Claude, 1.5),

    // ── GPT signals (new) ──────────────────────────────────────────────
    h!("rust.comments.step_numbered",    HeuristicLanguage::Rust,   "comments",   "3+ step-numbered comments",                    ModelFamily::Gpt, 1.5),
    h!("python.comments.step_numbered",  HeuristicLanguage::Python, "comments",   "3+ step-numbered comments",                    ModelFamily::Gpt, 1.5),
    h!("js.comments.step_numbered",      HeuristicLanguage::Js,     "comments",   "3+ step-numbered comments",                    ModelFamily::Gpt, 1.5),
    h!("go.comments.step_numbered",      HeuristicLanguage::Go,     "comments",   "3+ step-numbered comments",                    ModelFamily::Gpt, 1.5),
    h!("rust.comments.heres_lets",       HeuristicLanguage::Rust,   "comments",   "3+ here's/let's phrases in comments",          ModelFamily::Gpt, 1.0),
    h!("python.comments.heres_lets",     HeuristicLanguage::Python, "comments",   "3+ here's/let's phrases in comments",          ModelFamily::Gpt, 1.0),
    h!("js.comments.heres_lets",         HeuristicLanguage::Js,     "comments",   "3+ here's/let's phrases in comments",          ModelFamily::Gpt, 1.0),
    h!("go.comments.heres_lets",         HeuristicLanguage::Go,     "comments",   "3+ here's/let's phrases in comments",          ModelFamily::Gpt, 1.0),
    h!("rust.ai_signals.triple_backtick",   HeuristicLanguage::Rust,   "ai_signals", "Markdown triple-backtick in code comments",  ModelFamily::Gpt, 1.5),
    h!("python.ai_signals.triple_backtick", HeuristicLanguage::Python, "ai_signals", "Markdown triple-backtick in code comments",  ModelFamily::Gpt, 1.5),
    h!("js.ai_signals.triple_backtick",     HeuristicLanguage::Js,     "ai_signals", "Markdown triple-backtick in code comments",  ModelFamily::Gpt, 1.5),
    h!("go.ai_signals.triple_backtick",     HeuristicLanguage::Go,     "ai_signals", "Markdown triple-backtick in code comments",  ModelFamily::Gpt, 1.5),
    h!("rust.comments.verbose_obvious",  HeuristicLanguage::Rust,   "comments",   "High comment-to-code ratio in simple code",    ModelFamily::Gpt, 1.2),
    h!("python.comments.verbose_obvious",HeuristicLanguage::Python, "comments",   "High comment-to-code ratio in simple code",    ModelFamily::Gpt, 1.2),
    h!("js.comments.verbose_obvious",    HeuristicLanguage::Js,     "comments",   "High comment-to-code ratio in simple code",    ModelFamily::Gpt, 1.2),
    h!("go.comments.verbose_obvious",    HeuristicLanguage::Go,     "comments",   "High comment-to-code ratio in simple code",    ModelFamily::Gpt, 1.2),

    // ── Gemini signals (new) ───────────────────────────────────────────
    h!("rust.comments.bullet_style",     HeuristicLanguage::Rust,   "comments",   "3+ bullet-point comments",                     ModelFamily::Gemini, 1.0),
    h!("python.comments.bullet_style",   HeuristicLanguage::Python, "comments",   "3+ bullet-point comments",                     ModelFamily::Gemini, 1.0),
    h!("js.comments.bullet_style",       HeuristicLanguage::Js,     "comments",   "3+ bullet-point comments",                     ModelFamily::Gemini, 1.0),
    h!("go.comments.bullet_style",       HeuristicLanguage::Go,     "comments",   "3+ bullet-point comments",                     ModelFamily::Gemini, 1.0),
    h!("rust.naming.medium_descriptive",    HeuristicLanguage::Rust,   "naming",  "Medium-length descriptive names (avg 5–8 chars)", ModelFamily::Gemini, 1.0),
    h!("python.naming.medium_descriptive",  HeuristicLanguage::Python, "naming",  "Medium-length descriptive names (avg 5–8 chars)", ModelFamily::Gemini, 1.0),
    h!("js.naming.medium_descriptive",      HeuristicLanguage::Js,     "naming",  "Medium-length descriptive names (avg 5–8 chars)", ModelFamily::Gemini, 1.0),
    h!("go.naming.medium_descriptive",      HeuristicLanguage::Go,     "naming",  "Medium-length descriptive names (avg 5–8 chars)", ModelFamily::Gemini, 1.0),
    h!("rust.structure.ternary_heavy",   HeuristicLanguage::Rust,   "structure",  "3+ conditional expressions",                   ModelFamily::Gemini, 1.2),
    h!("python.structure.ternary_heavy", HeuristicLanguage::Python, "structure",  "3+ conditional expressions",                   ModelFamily::Gemini, 1.2),
    h!("js.structure.ternary_heavy",     HeuristicLanguage::Js,     "structure",  "3+ ternary expressions",                       ModelFamily::Gemini, 1.2),
    h!("go.structure.ternary_heavy",     HeuristicLanguage::Go,     "structure",  "3+ conditional expressions",                   ModelFamily::Gemini, 1.2),
    h!("rust.structure.compact_fns",     HeuristicLanguage::Rust,   "structure",  "Average function length 10–20 lines",          ModelFamily::Gemini, 1.0),
    h!("python.structure.compact_fns",   HeuristicLanguage::Python, "structure",  "Average function length 10–20 lines",          ModelFamily::Gemini, 1.0),
    h!("js.structure.compact_fns",       HeuristicLanguage::Js,     "structure",  "Average function length 10–20 lines",          ModelFamily::Gemini, 1.0),
    h!("go.structure.compact_fns",       HeuristicLanguage::Go,     "structure",  "Average function length 10–20 lines",          ModelFamily::Gemini, 1.0),

    // ── Copilot signals (new) ──────────────────────────────────────────
    h!("rust.comments.minimal",          HeuristicLanguage::Rust,   "comments",   "<1% comment density in file >30 lines",        ModelFamily::Copilot, 1.5),
    h!("python.comments.minimal",        HeuristicLanguage::Python, "comments",   "<1% comment density in file >30 lines",        ModelFamily::Copilot, 1.5),
    h!("js.comments.minimal",            HeuristicLanguage::Js,     "comments",   "<1% comment density in file >30 lines",        ModelFamily::Copilot, 1.5),
    h!("go.comments.minimal",            HeuristicLanguage::Go,     "comments",   "<1% comment density in file >30 lines",        ModelFamily::Copilot, 1.5),
    h!("rust.naming.mixed_conventions",     HeuristicLanguage::Rust,   "naming",  "Mixed camelCase and snake_case identifiers",    ModelFamily::Copilot, 1.5),
    h!("python.naming.mixed_conventions",   HeuristicLanguage::Python, "naming",  "Mixed camelCase and snake_case identifiers",    ModelFamily::Copilot, 1.5),
    h!("js.naming.mixed_conventions",       HeuristicLanguage::Js,     "naming",  "Mixed camelCase and snake_case identifiers",    ModelFamily::Copilot, 1.5),
    h!("go.naming.mixed_conventions",       HeuristicLanguage::Go,     "naming",  "Mixed camelCase and snake_case identifiers",    ModelFamily::Copilot, 1.5),
    h!("rust.structure.very_short_fns",  HeuristicLanguage::Rust,   "structure",  "Average function length <10 lines",            ModelFamily::Copilot, 1.2),
    h!("python.structure.very_short_fns",HeuristicLanguage::Python, "structure",  "Average function length <10 lines",            ModelFamily::Copilot, 1.2),
    h!("js.structure.very_short_fns",    HeuristicLanguage::Js,     "structure",  "Average function length <10 lines",            ModelFamily::Copilot, 1.2),
    h!("go.structure.very_short_fns",    HeuristicLanguage::Go,     "structure",  "Average function length <10 lines",            ModelFamily::Copilot, 1.2),
    h!("rust.structure.format_inconsistent",   HeuristicLanguage::Rust,   "structure", "Mixed indentation (tabs+spaces)",          ModelFamily::Copilot, 1.2),
    h!("python.structure.format_inconsistent", HeuristicLanguage::Python, "structure", "Mixed indentation (tabs+spaces)",          ModelFamily::Copilot, 1.2),
    h!("js.structure.format_inconsistent",     HeuristicLanguage::Js,     "structure", "Mixed indentation (tabs+spaces)",          ModelFamily::Copilot, 1.2),
    h!("go.structure.format_inconsistent",     HeuristicLanguage::Go,     "structure", "Mixed indentation (tabs+spaces)",          ModelFamily::Copilot, 1.2),

    // ── Human signals (new) ────────────────────────────────────────────
    h!("rust.comments.external_refs",    HeuristicLanguage::Rust,   "comments",   "2+ ticket/issue references in comments",       ModelFamily::Human, 2.0),
    h!("python.comments.external_refs",  HeuristicLanguage::Python, "comments",   "2+ ticket/issue references in comments",       ModelFamily::Human, 2.0),
    h!("js.comments.external_refs",      HeuristicLanguage::Js,     "comments",   "2+ ticket/issue references in comments",       ModelFamily::Human, 2.0),
    h!("go.comments.external_refs",      HeuristicLanguage::Go,     "comments",   "2+ ticket/issue references in comments",       ModelFamily::Human, 2.0),
    h!("rust.ai_signals.pragma",         HeuristicLanguage::Rust,   "ai_signals", "Pragma/lint override directives",              ModelFamily::Human, 1.5),
    h!("python.ai_signals.pragma",       HeuristicLanguage::Python, "ai_signals", "Pragma/lint override directives",              ModelFamily::Human, 1.5),
    h!("js.ai_signals.pragma",           HeuristicLanguage::Js,     "ai_signals", "Pragma/lint override directives",              ModelFamily::Human, 1.5),
    h!("go.ai_signals.pragma",           HeuristicLanguage::Go,     "ai_signals", "Pragma/lint override directives",              ModelFamily::Human, 1.5),
    h!("rust.naming.domain_abbreviations",   HeuristicLanguage::Rust,   "naming", "3+ domain abbreviations (cfg, ctx, etc.)",     ModelFamily::Human, 1.0),
    h!("python.naming.domain_abbreviations", HeuristicLanguage::Python, "naming", "3+ domain abbreviations (cfg, ctx, etc.)",     ModelFamily::Human, 1.0),
    h!("js.naming.domain_abbreviations",     HeuristicLanguage::Js,     "naming", "3+ domain abbreviations (cfg, ctx, etc.)",     ModelFamily::Human, 1.0),
    h!("go.naming.domain_abbreviations",     HeuristicLanguage::Go,     "naming", "3+ domain abbreviations (cfg, ctx, etc.)",     ModelFamily::Human, 1.0),
];

// ---------------------------------------------------------------------------
// signal_ids — compile-time constants for every signal ID
// ---------------------------------------------------------------------------

/// Compile-time string constants for every signal ID in [`ALL_HEURISTICS`].
///
/// Import selectively: `use vibecheck_core::heuristics::signal_ids;`
pub mod signal_ids {
    // rust · errors
    pub const RUST_ERRORS_ZERO_UNWRAP:      &str = "rust.errors.zero_unwrap";
    pub const RUST_ERRORS_MANY_UNWRAPS:     &str = "rust.errors.many_unwraps";
    pub const RUST_ERRORS_FEW_UNWRAPS:      &str = "rust.errors.few_unwraps";
    pub const RUST_ERRORS_EXPECT_CALLS:     &str = "rust.errors.expect_calls";
    pub const RUST_ERRORS_QUESTION_MARK:    &str = "rust.errors.question_mark";
    pub const RUST_ERRORS_EXHAUSTIVE_MATCH: &str = "rust.errors.exhaustive_match";
    pub const RUST_ERRORS_PANIC_CALLS:      &str = "rust.errors.panic_calls";

    // python · errors
    pub const PYTHON_ERRORS_BROAD_EXCEPT:    &str = "python.errors.broad_except";
    pub const PYTHON_ERRORS_SPECIFIC_EXCEPT: &str = "python.errors.specific_except";
    pub const PYTHON_ERRORS_NO_TRY_EXCEPT:   &str = "python.errors.no_try_except";
    pub const PYTHON_ERRORS_RAISE_FROM:      &str = "python.errors.raise_from";

    // js · errors
    pub const JS_ERRORS_CONSOLE_ERROR:            &str = "js.errors.console_error";
    pub const JS_ERRORS_TYPED_ERROR_CHECK:        &str = "js.errors.typed_error_check";
    pub const JS_ERRORS_PROMISE_CATCH:            &str = "js.errors.promise_catch";
    pub const JS_ERRORS_TRY_CATCH_BLOCKS:         &str = "js.errors.try_catch_blocks";
    pub const JS_ERRORS_TYPED_ERROR_CONSTRUCTION: &str = "js.errors.typed_error_construction";

    // go · errors
    pub const GO_ERRORS_SIMPLE_ERR_RETURN: &str = "go.errors.simple_err_return";
    pub const GO_ERRORS_ERRORF_WRAP:       &str = "go.errors.errorf_wrap";
    pub const GO_ERRORS_ERRORS_SENTINEL:   &str = "go.errors.errors_sentinel";
    pub const GO_ERRORS_PANIC_CALLS:       &str = "go.errors.panic_calls";

    // rust · ai_signals
    pub const RUST_AI_SIGNALS_NO_TODO:           &str = "rust.ai_signals.no_todo";
    pub const RUST_AI_SIGNALS_NO_DEAD_CODE:      &str = "rust.ai_signals.no_dead_code";
    pub const RUST_AI_SIGNALS_ALL_FNS_DOCUMENTED:&str = "rust.ai_signals.all_fns_documented";
    pub const RUST_AI_SIGNALS_NO_TRAILING_WS:    &str = "rust.ai_signals.no_trailing_ws";
    pub const RUST_AI_SIGNALS_COMMENTED_OUT_CODE:&str = "rust.ai_signals.commented_out_code";
    pub const RUST_AI_SIGNALS_NO_PLACEHOLDER:    &str = "rust.ai_signals.no_placeholder";

    // python · ai_signals
    pub const PYTHON_AI_SIGNALS_NO_TODO:              &str = "python.ai_signals.no_todo";
    pub const PYTHON_AI_SIGNALS_NO_TRAILING_WS:       &str = "python.ai_signals.no_trailing_ws";
    pub const PYTHON_AI_SIGNALS_NO_PLACEHOLDER:       &str = "python.ai_signals.no_placeholder";
    pub const PYTHON_AI_SIGNALS_NO_LINTER_SUPPRESSION:&str = "python.ai_signals.no_linter_suppression";
    pub const PYTHON_AI_SIGNALS_COMMENTED_OUT_CODE:   &str = "python.ai_signals.commented_out_code";
    pub const PYTHON_AI_SIGNALS_ALL_FNS_DOCUMENTED:   &str = "python.ai_signals.all_fns_documented";

    // js · ai_signals
    pub const JS_AI_SIGNALS_NO_TODO:              &str = "js.ai_signals.no_todo";
    pub const JS_AI_SIGNALS_NO_TRAILING_WS:       &str = "js.ai_signals.no_trailing_ws";
    pub const JS_AI_SIGNALS_NO_PLACEHOLDER:       &str = "js.ai_signals.no_placeholder";
    pub const JS_AI_SIGNALS_NO_LINTER_SUPPRESSION:&str = "js.ai_signals.no_linter_suppression";
    pub const JS_AI_SIGNALS_COMMENTED_OUT_CODE:   &str = "js.ai_signals.commented_out_code";
    pub const JS_AI_SIGNALS_JSDOC_BLOCKS:         &str = "js.ai_signals.jsdoc_blocks";
    pub const JS_AI_SIGNALS_CONSOLE_LOG:          &str = "js.ai_signals.console_log";

    // go · ai_signals
    pub const GO_AI_SIGNALS_NO_TODO:               &str = "go.ai_signals.no_todo";
    pub const GO_AI_SIGNALS_NO_TRAILING_WS:        &str = "go.ai_signals.no_trailing_ws";
    pub const GO_AI_SIGNALS_NO_PLACEHOLDER:        &str = "go.ai_signals.no_placeholder";
    pub const GO_AI_SIGNALS_NO_NOLINT:             &str = "go.ai_signals.no_nolint";
    pub const GO_AI_SIGNALS_COMMENTED_OUT_CODE:    &str = "go.ai_signals.commented_out_code";
    pub const GO_AI_SIGNALS_ALL_EXPORTED_DOCUMENTED:&str = "go.ai_signals.all_exported_documented";

    // rust · structure
    pub const RUST_STRUCTURE_HIGH_TYPE_ANNOTATION:  &str = "rust.structure.high_type_annotation";
    pub const RUST_STRUCTURE_LOW_TYPE_ANNOTATION:   &str = "rust.structure.low_type_annotation";
    pub const RUST_STRUCTURE_SORTED_IMPORTS:        &str = "rust.structure.sorted_imports";
    pub const RUST_STRUCTURE_CONSISTENT_BLANK_LINES:&str = "rust.structure.consistent_blank_lines";
    pub const RUST_STRUCTURE_LINES_UNDER_100:       &str = "rust.structure.lines_under_100";
    pub const RUST_STRUCTURE_MANY_LONG_LINES:       &str = "rust.structure.many_long_lines";
    pub const RUST_STRUCTURE_HEAVY_DERIVE:          &str = "rust.structure.heavy_derive";

    // python · structure
    pub const PYTHON_STRUCTURE_SORTED_IMPORTS:        &str = "python.structure.sorted_imports";
    pub const PYTHON_STRUCTURE_CONSISTENT_BLANK_LINES:&str = "python.structure.consistent_blank_lines";
    pub const PYTHON_STRUCTURE_LINES_UNDER_88:        &str = "python.structure.lines_under_88";

    // js · structure
    pub const JS_STRUCTURE_SORTED_IMPORTS:        &str = "js.structure.sorted_imports";
    pub const JS_STRUCTURE_CONSISTENT_BLANK_LINES:&str = "js.structure.consistent_blank_lines";
    pub const JS_STRUCTURE_LINES_UNDER_100:       &str = "js.structure.lines_under_100";
    pub const JS_STRUCTURE_MANY_LONG_LINES:       &str = "js.structure.many_long_lines";

    // go · structure
    pub const GO_STRUCTURE_SORTED_IMPORTS:        &str = "go.structure.sorted_imports";
    pub const GO_STRUCTURE_CONSISTENT_BLANK_LINES:&str = "go.structure.consistent_blank_lines";
    pub const GO_STRUCTURE_LINES_UNDER_120:       &str = "go.structure.lines_under_120";

    // rust · comments
    pub const RUST_COMMENTS_HIGH_DENSITY:    &str = "rust.comments.high_density";
    pub const RUST_COMMENTS_LOW_DENSITY:     &str = "rust.comments.low_density";
    pub const RUST_COMMENTS_TEACHING_VOICE:  &str = "rust.comments.teaching_voice";
    pub const RUST_COMMENTS_SOME_EXPLANATORY:&str = "rust.comments.some_explanatory";
    pub const RUST_COMMENTS_DOC_COMMENTS:    &str = "rust.comments.doc_comments";
    pub const RUST_COMMENTS_TERSE_MARKERS:   &str = "rust.comments.terse_markers";

    // python · comments
    pub const PYTHON_COMMENTS_HIGH_DENSITY:    &str = "python.comments.high_density";
    pub const PYTHON_COMMENTS_LOW_DENSITY:     &str = "python.comments.low_density";
    pub const PYTHON_COMMENTS_TEACHING_VOICE:  &str = "python.comments.teaching_voice";
    pub const PYTHON_COMMENTS_SOME_EXPLANATORY:&str = "python.comments.some_explanatory";
    pub const PYTHON_COMMENTS_DOCSTRING_BLOCKS:&str = "python.comments.docstring_blocks";
    pub const PYTHON_COMMENTS_TERSE_MARKERS:   &str = "python.comments.terse_markers";

    // js · comments
    pub const JS_COMMENTS_HIGH_DENSITY:    &str = "js.comments.high_density";
    pub const JS_COMMENTS_LOW_DENSITY:     &str = "js.comments.low_density";
    pub const JS_COMMENTS_TEACHING_VOICE:  &str = "js.comments.teaching_voice";
    pub const JS_COMMENTS_SOME_EXPLANATORY:&str = "js.comments.some_explanatory";
    pub const JS_COMMENTS_TERSE_MARKERS:   &str = "js.comments.terse_markers";
    pub const JS_COMMENTS_JSDOC_BLOCKS:    &str = "js.comments.jsdoc_blocks";

    // go · comments
    pub const GO_COMMENTS_HIGH_DENSITY:    &str = "go.comments.high_density";
    pub const GO_COMMENTS_LOW_DENSITY:     &str = "go.comments.low_density";
    pub const GO_COMMENTS_TEACHING_VOICE:  &str = "go.comments.teaching_voice";
    pub const GO_COMMENTS_SOME_EXPLANATORY:&str = "go.comments.some_explanatory";
    pub const GO_COMMENTS_TERSE_MARKERS:   &str = "go.comments.terse_markers";

    // rust · naming
    pub const RUST_NAMING_VERY_DESCRIPTIVE_VARS: &str = "rust.naming.very_descriptive_vars";
    pub const RUST_NAMING_DESCRIPTIVE_VARS:      &str = "rust.naming.descriptive_vars";
    pub const RUST_NAMING_SHORT_VARS:            &str = "rust.naming.short_vars";
    pub const RUST_NAMING_MANY_SINGLE_CHAR_VARS: &str = "rust.naming.many_single_char_vars";
    pub const RUST_NAMING_NO_SINGLE_CHAR_VARS:   &str = "rust.naming.no_single_char_vars";
    pub const RUST_NAMING_UNDERSCORE_BINDINGS:   &str = "rust.naming.underscore_bindings";
    pub const RUST_NAMING_DESCRIPTIVE_FN_NAMES:  &str = "rust.naming.descriptive_fn_names";

    // python · naming
    pub const PYTHON_NAMING_VERY_DESCRIPTIVE: &str = "python.naming.very_descriptive";
    pub const PYTHON_NAMING_DESCRIPTIVE:      &str = "python.naming.descriptive";
    pub const PYTHON_NAMING_SHORT_NAMES:      &str = "python.naming.short_names";
    pub const PYTHON_NAMING_MANY_SINGLE_CHAR: &str = "python.naming.many_single_char";
    pub const PYTHON_NAMING_NO_SINGLE_CHAR:   &str = "python.naming.no_single_char";

    // js · naming
    pub const JS_NAMING_VERY_DESCRIPTIVE: &str = "js.naming.very_descriptive";
    pub const JS_NAMING_DESCRIPTIVE:      &str = "js.naming.descriptive";
    pub const JS_NAMING_SHORT_NAMES:      &str = "js.naming.short_names";
    pub const JS_NAMING_MANY_SINGLE_CHAR: &str = "js.naming.many_single_char";
    pub const JS_NAMING_NO_SINGLE_CHAR:   &str = "js.naming.no_single_char";

    // go · naming
    pub const GO_NAMING_VERY_DESCRIPTIVE: &str = "go.naming.very_descriptive";
    pub const GO_NAMING_DESCRIPTIVE:      &str = "go.naming.descriptive";
    pub const GO_NAMING_SHORT_NAMES:      &str = "go.naming.short_names";
    pub const GO_NAMING_MANY_SINGLE_CHAR: &str = "go.naming.many_single_char";
    pub const GO_NAMING_NO_SINGLE_CHAR:   &str = "go.naming.no_single_char";

    // rust · idioms
    pub const RUST_IDIOMS_ITERATOR_CHAINS:  &str = "rust.idioms.iterator_chains";
    pub const RUST_IDIOMS_BUILDER_PATTERN:  &str = "rust.idioms.builder_pattern";
    pub const RUST_IDIOMS_IMPL_DISPLAY:     &str = "rust.idioms.impl_display";
    pub const RUST_IDIOMS_FROM_INTO_IMPLS:  &str = "rust.idioms.from_into_impls";
    pub const RUST_IDIOMS_SELF_USAGE:       &str = "rust.idioms.self_usage";
    pub const RUST_IDIOMS_PATTERN_MATCHING: &str = "rust.idioms.pattern_matching";
    pub const RUST_IDIOMS_FORMAT_MACRO:     &str = "rust.idioms.format_macro";
    pub const RUST_IDIOMS_STRING_CONCAT:    &str = "rust.idioms.string_concat";
    pub const RUST_IDIOMS_MANY_TRAITS:      &str = "rust.idioms.many_traits";

    // python · idioms
    pub const PYTHON_IDIOMS_COMPREHENSIONS:          &str = "python.idioms.comprehensions";
    pub const PYTHON_IDIOMS_RETURN_TYPE_ANNOTATIONS: &str = "python.idioms.return_type_annotations";
    pub const PYTHON_IDIOMS_CONTEXT_MANAGERS:        &str = "python.idioms.context_managers";
    pub const PYTHON_IDIOMS_FUNCTIONAL_BUILTINS:     &str = "python.idioms.functional_builtins";
    pub const PYTHON_IDIOMS_FSTRINGS:                &str = "python.idioms.fstrings";
    pub const PYTHON_IDIOMS_OLD_FORMAT:              &str = "python.idioms.old_format";

    // js · idioms
    pub const JS_IDIOMS_ARROW_FNS_ONLY:    &str = "js.idioms.arrow_fns_only";
    pub const JS_IDIOMS_REGULAR_FNS_ONLY:  &str = "js.idioms.regular_fns_only";
    pub const JS_IDIOMS_VAR_DECLARATIONS:  &str = "js.idioms.var_declarations";
    pub const JS_IDIOMS_CONST_DECLARATIONS:&str = "js.idioms.const_declarations";
    pub const JS_IDIOMS_NULL_SAFE_OPS:     &str = "js.idioms.null_safe_ops";
    pub const JS_IDIOMS_DESTRUCTURING:     &str = "js.idioms.destructuring";
    pub const JS_IDIOMS_ASYNC_AWAIT:       &str = "js.idioms.async_await";

    // go · idioms
    pub const GO_IDIOMS_INTERFACE_CHECKS:   &str = "go.idioms.interface_checks";
    pub const GO_IDIOMS_GOROUTINES:         &str = "go.idioms.goroutines";
    pub const GO_IDIOMS_DEFER_STMTS:        &str = "go.idioms.defer_stmts";
    pub const GO_IDIOMS_TABLE_DRIVEN_TESTS: &str = "go.idioms.table_driven_tests";
    pub const GO_IDIOMS_IOTA_CONSTANTS:     &str = "go.idioms.iota_constants";

    // rust_cst
    pub const RUST_CST_COMPLEXITY_LOW:    &str = "rust_cst.complexity.low";
    pub const RUST_CST_COMPLEXITY_HIGH:   &str = "rust_cst.complexity.high";
    pub const RUST_CST_DOC_COVERAGE_HIGH: &str = "rust_cst.doc_coverage.high";
    pub const RUST_CST_ENTROPY_HIGH:      &str = "rust_cst.entropy.high";
    pub const RUST_CST_ENTROPY_LOW:       &str = "rust_cst.entropy.low";
    pub const RUST_CST_NESTING_LOW:       &str = "rust_cst.nesting.low";
    pub const RUST_CST_IMPORTS_SORTED:    &str = "rust_cst.imports.sorted";

    // python_cst
    pub const PYTHON_CST_DOC_COVERAGE_HIGH:     &str = "python_cst.doc_coverage.high";
    pub const PYTHON_CST_TYPE_ANNOTATIONS_HIGH: &str = "python_cst.type_annotations.high";
    pub const PYTHON_CST_FSTRINGS_ONLY:         &str = "python_cst.fstrings.only";

    // js_cst
    pub const JS_CST_ARROW_FNS_HIGH_RATIO:   &str = "js_cst.arrow_fns.high_ratio";
    pub const JS_CST_ASYNC_AWAIT_ONLY:        &str = "js_cst.async.await_only";
    pub const JS_CST_ASYNC_THEN_ONLY:         &str = "js_cst.async.then_only";
    pub const JS_CST_OPTIONAL_CHAINING_HIGH:  &str = "js_cst.optional_chaining.high";

    // go_cst
    pub const GO_CST_DOC_COVERAGE_HIGH:  &str = "go_cst.doc_coverage.high";
    pub const GO_CST_GOROUTINES_PRESENT: &str = "go_cst.goroutines.present";
    pub const GO_CST_ERRORS_NIL_CHECKS:  &str = "go_cst.errors.nil_checks";

    // GPT signals (new)
    pub const RUST_COMMENTS_STEP_NUMBERED:       &str = "rust.comments.step_numbered";
    pub const PYTHON_COMMENTS_STEP_NUMBERED:     &str = "python.comments.step_numbered";
    pub const JS_COMMENTS_STEP_NUMBERED:         &str = "js.comments.step_numbered";
    pub const GO_COMMENTS_STEP_NUMBERED:         &str = "go.comments.step_numbered";
    pub const RUST_COMMENTS_HERES_LETS:          &str = "rust.comments.heres_lets";
    pub const PYTHON_COMMENTS_HERES_LETS:        &str = "python.comments.heres_lets";
    pub const JS_COMMENTS_HERES_LETS:            &str = "js.comments.heres_lets";
    pub const GO_COMMENTS_HERES_LETS:            &str = "go.comments.heres_lets";
    pub const RUST_AI_SIGNALS_TRIPLE_BACKTICK:   &str = "rust.ai_signals.triple_backtick";
    pub const PYTHON_AI_SIGNALS_TRIPLE_BACKTICK: &str = "python.ai_signals.triple_backtick";
    pub const JS_AI_SIGNALS_TRIPLE_BACKTICK:     &str = "js.ai_signals.triple_backtick";
    pub const GO_AI_SIGNALS_TRIPLE_BACKTICK:     &str = "go.ai_signals.triple_backtick";
    pub const RUST_COMMENTS_VERBOSE_OBVIOUS:     &str = "rust.comments.verbose_obvious";
    pub const PYTHON_COMMENTS_VERBOSE_OBVIOUS:   &str = "python.comments.verbose_obvious";
    pub const JS_COMMENTS_VERBOSE_OBVIOUS:       &str = "js.comments.verbose_obvious";
    pub const GO_COMMENTS_VERBOSE_OBVIOUS:       &str = "go.comments.verbose_obvious";

    // Gemini signals (new)
    pub const RUST_COMMENTS_BULLET_STYLE:        &str = "rust.comments.bullet_style";
    pub const PYTHON_COMMENTS_BULLET_STYLE:      &str = "python.comments.bullet_style";
    pub const JS_COMMENTS_BULLET_STYLE:          &str = "js.comments.bullet_style";
    pub const GO_COMMENTS_BULLET_STYLE:          &str = "go.comments.bullet_style";
    pub const RUST_NAMING_MEDIUM_DESCRIPTIVE:    &str = "rust.naming.medium_descriptive";
    pub const PYTHON_NAMING_MEDIUM_DESCRIPTIVE:  &str = "python.naming.medium_descriptive";
    pub const JS_NAMING_MEDIUM_DESCRIPTIVE:      &str = "js.naming.medium_descriptive";
    pub const GO_NAMING_MEDIUM_DESCRIPTIVE:      &str = "go.naming.medium_descriptive";
    pub const RUST_STRUCTURE_TERNARY_HEAVY:      &str = "rust.structure.ternary_heavy";
    pub const PYTHON_STRUCTURE_TERNARY_HEAVY:    &str = "python.structure.ternary_heavy";
    pub const JS_STRUCTURE_TERNARY_HEAVY:        &str = "js.structure.ternary_heavy";
    pub const GO_STRUCTURE_TERNARY_HEAVY:        &str = "go.structure.ternary_heavy";
    pub const RUST_STRUCTURE_COMPACT_FNS:        &str = "rust.structure.compact_fns";
    pub const PYTHON_STRUCTURE_COMPACT_FNS:      &str = "python.structure.compact_fns";
    pub const JS_STRUCTURE_COMPACT_FNS:          &str = "js.structure.compact_fns";
    pub const GO_STRUCTURE_COMPACT_FNS:          &str = "go.structure.compact_fns";

    // Copilot signals (new)
    pub const RUST_COMMENTS_MINIMAL:             &str = "rust.comments.minimal";
    pub const PYTHON_COMMENTS_MINIMAL:           &str = "python.comments.minimal";
    pub const JS_COMMENTS_MINIMAL:               &str = "js.comments.minimal";
    pub const GO_COMMENTS_MINIMAL:               &str = "go.comments.minimal";
    pub const RUST_NAMING_MIXED_CONVENTIONS:     &str = "rust.naming.mixed_conventions";
    pub const PYTHON_NAMING_MIXED_CONVENTIONS:   &str = "python.naming.mixed_conventions";
    pub const JS_NAMING_MIXED_CONVENTIONS:       &str = "js.naming.mixed_conventions";
    pub const GO_NAMING_MIXED_CONVENTIONS:       &str = "go.naming.mixed_conventions";
    pub const RUST_STRUCTURE_VERY_SHORT_FNS:     &str = "rust.structure.very_short_fns";
    pub const PYTHON_STRUCTURE_VERY_SHORT_FNS:   &str = "python.structure.very_short_fns";
    pub const JS_STRUCTURE_VERY_SHORT_FNS:       &str = "js.structure.very_short_fns";
    pub const GO_STRUCTURE_VERY_SHORT_FNS:       &str = "go.structure.very_short_fns";
    pub const RUST_STRUCTURE_FORMAT_INCONSISTENT:   &str = "rust.structure.format_inconsistent";
    pub const PYTHON_STRUCTURE_FORMAT_INCONSISTENT: &str = "python.structure.format_inconsistent";
    pub const JS_STRUCTURE_FORMAT_INCONSISTENT:     &str = "js.structure.format_inconsistent";
    pub const GO_STRUCTURE_FORMAT_INCONSISTENT:     &str = "go.structure.format_inconsistent";

    // Human signals (new)
    pub const RUST_COMMENTS_EXTERNAL_REFS:       &str = "rust.comments.external_refs";
    pub const PYTHON_COMMENTS_EXTERNAL_REFS:     &str = "python.comments.external_refs";
    pub const JS_COMMENTS_EXTERNAL_REFS:         &str = "js.comments.external_refs";
    pub const GO_COMMENTS_EXTERNAL_REFS:         &str = "go.comments.external_refs";
    pub const RUST_AI_SIGNALS_PRAGMA:            &str = "rust.ai_signals.pragma";
    pub const PYTHON_AI_SIGNALS_PRAGMA:          &str = "python.ai_signals.pragma";
    pub const JS_AI_SIGNALS_PRAGMA:              &str = "js.ai_signals.pragma";
    pub const GO_AI_SIGNALS_PRAGMA:              &str = "go.ai_signals.pragma";
    pub const RUST_NAMING_DOMAIN_ABBREVIATIONS:  &str = "rust.naming.domain_abbreviations";
    pub const PYTHON_NAMING_DOMAIN_ABBREVIATIONS:&str = "python.naming.domain_abbreviations";
    pub const JS_NAMING_DOMAIN_ABBREVIATIONS:    &str = "js.naming.domain_abbreviations";
    pub const GO_NAMING_DOMAIN_ABBREVIATIONS:    &str = "go.naming.domain_abbreviations";
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
// DefaultHeuristics — looks up ALL_HEURISTICS table
// ---------------------------------------------------------------------------

/// Uses the hardcoded defaults from [`ALL_HEURISTICS`].
///
/// This is the implementation used by [`Pipeline::with_defaults`].
pub struct DefaultHeuristics;

impl HeuristicsProvider for DefaultHeuristics {
    fn weight(&self, id: &str) -> f64 {
        ALL_HEURISTICS
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
    fn all_heuristics_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for h in ALL_HEURISTICS {
            assert!(seen.insert(h.id), "duplicate heuristic id: {}", h.id);
        }
    }

    #[test]
    fn signal_id_constants_match_all_heuristics() {
        // Every constant in signal_ids should appear in ALL_HEURISTICS.
        let ids: std::collections::HashSet<&str> =
            ALL_HEURISTICS.iter().map(|h| h.id).collect();
        // Spot-check a few well-known constants.
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
        // Unoverridden signal falls back to default.
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
        for h in ALL_HEURISTICS {
            *counts.entry(h.family).or_default() += 1;
        }
        let total = ALL_HEURISTICS.len();
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
        for spec in ALL_HEURISTICS {
            assert_eq!(
                inert.weight(spec.id),
                def.weight(spec.id),
                "mismatch for {}",
                spec.id
            );
        }
    }
}
