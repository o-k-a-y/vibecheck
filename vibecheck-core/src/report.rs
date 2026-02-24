use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// The model families we can attribute code to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFamily {
    Claude,
    Gpt,
    Gemini,
    Copilot,
    Human,
}

impl ModelFamily {
    pub fn all() -> &'static [ModelFamily] {
        &[
            ModelFamily::Claude,
            ModelFamily::Gpt,
            ModelFamily::Gemini,
            ModelFamily::Copilot,
            ModelFamily::Human,
        ]
    }

    /// Short display abbreviation for compact UI contexts (e.g. TUI badges, table cells).
    pub fn abbrev(self) -> &'static str {
        match self {
            ModelFamily::Claude  => "Cl",
            ModelFamily::Gpt     => "GPT",
            ModelFamily::Gemini  => "Ge",
            ModelFamily::Copilot => "Co",
            ModelFamily::Human   => "Hu",
        }
    }
}

impl std::fmt::Display for ModelFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFamily::Claude => write!(f, "Claude"),
            ModelFamily::Gpt => write!(f, "GPT"),
            ModelFamily::Gemini => write!(f, "Gemini"),
            ModelFamily::Copilot => write!(f, "Copilot"),
            ModelFamily::Human => write!(f, "Human"),
        }
    }
}

/// A single signal emitted by an analyzer.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// Stable dot-separated identifier, e.g. `"rust.errors.zero_unwrap"`.
    /// Empty string for signals loaded from old cache entries that predate IDs.
    #[serde(default)]
    pub id: String,
    /// Which analyzer produced this signal.
    pub source: String,
    /// Human-readable description of what was detected.
    pub description: String,
    /// Which model family this signal points toward.
    pub family: ModelFamily,
    /// Weight of this signal (negative = evidence against).
    pub weight: f64,
}

impl Signal {
    /// Construct a signal with a stable ID.
    ///
    /// Use the constants in [`vibecheck_core::heuristics::signal_ids`] for
    /// the `id` argument so that the ID never drifts from the catalogue.
    pub fn new(
        id: &str,
        source: &str,
        desc: impl Into<String>,
        family: ModelFamily,
        weight: f64,
    ) -> Self {
        Signal {
            id: id.to_string(),
            source: source.to_string(),
            description: desc.into(),
            family,
            weight,
        }
    }
}

/// The final attribution for a piece of code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribution {
    /// Most likely model family.
    pub primary: ModelFamily,
    /// Confidence in the primary attribution (0.0–1.0).
    pub confidence: f64,
    /// Score distribution across all families (sums to ~1.0).
    pub scores: HashMap<ModelFamily, f64>,
}

/// Metadata about the analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub file_path: Option<PathBuf>,
    pub lines_of_code: usize,
    pub signal_count: usize,
}

/// Metadata about a named symbol (function, method, class, etc.) within a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    pub name: String,
    pub kind: String,       // "function", "method", "class", etc.
    pub start_line: usize,
    pub end_line: usize,
}

impl SymbolMetadata {
    /// Short label for the symbol kind used in display contexts.
    ///
    /// Returns `"method"`, `"class"`, or `"fn"` (the catch-all).
    /// Falls back to `"fn"` rather than silently mis-labelling future kinds.
    pub fn kind_label(&self) -> &'static str {
        match self.kind.as_str() {
            "method"   => "method",
            "class"    => "class",
            _          => "fn",
        }
    }

    /// Display name truncated to `max_chars`, with `()` appended for
    /// functions and methods and left bare for classes.
    ///
    /// If the result would exceed `max_chars` it is truncated and `…` is
    /// appended.
    pub fn display_name(&self, max_chars: usize) -> String {
        let raw = match self.kind_label() {
            "class" => self.name.clone(),
            _       => format!("{}()", self.name),
        };
        if raw.len() > max_chars {
            format!("{}…", &raw[..max_chars.saturating_sub(1)])
        } else {
            raw
        }
    }
}

/// Analysis report for a single symbol within a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReport {
    pub metadata: SymbolMetadata,
    pub attribution: Attribution,
    pub signals: Vec<Signal>,
}

/// The full analysis report for a single source input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub attribution: Attribution,
    pub signals: Vec<Signal>,
    pub metadata: ReportMetadata,
    pub symbol_reports: Option<Vec<SymbolReport>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sym(kind: &str) -> SymbolMetadata {
        SymbolMetadata { name: "do_thing".into(), kind: kind.into(), start_line: 1, end_line: 5 }
    }

    #[test]
    fn kind_label_method() {
        assert_eq!(make_sym("method").kind_label(), "method");
    }

    #[test]
    fn kind_label_class() {
        assert_eq!(make_sym("class").kind_label(), "class");
    }

    #[test]
    fn kind_label_function_falls_back_to_fn() {
        assert_eq!(make_sym("function").kind_label(), "fn");
        assert_eq!(make_sym("unknown").kind_label(), "fn");
    }

    #[test]
    fn display_name_function_appends_parens() {
        assert_eq!(make_sym("function").display_name(50), "do_thing()");
    }

    #[test]
    fn display_name_class_no_parens() {
        let sym = SymbolMetadata { name: "MyClass".into(), kind: "class".into(), start_line: 1, end_line: 5 };
        assert_eq!(sym.display_name(50), "MyClass");
    }

    #[test]
    fn display_name_truncates() {
        let sym = SymbolMetadata {
            name: "very_long_function_name_here".into(),
            kind: "function".into(),
            start_line: 1,
            end_line: 5,
        };
        let result = sym.display_name(10);
        // "…" is 3 bytes; total byte len = 9 ASCII chars + 3 = 12
        assert!(result.chars().count() <= 10, "truncated result should fit within max_chars");
        assert!(result.ends_with('…'));
    }

    #[test]
    fn signal_new_roundtrip() {
        let s = Signal::new("rust.errors.zero_unwrap", "errors", "desc", ModelFamily::Claude, 1.5);
        assert_eq!(s.id, "rust.errors.zero_unwrap");
        assert_eq!(s.source, "errors");
        assert_eq!(s.description, "desc");
        assert_eq!(s.family, ModelFamily::Claude);
        assert_eq!(s.weight, 1.5);
    }

    #[test]
    fn model_family_display() {
        assert_eq!(ModelFamily::Claude.to_string(),  "Claude");
        assert_eq!(ModelFamily::Gpt.to_string(),     "GPT");
        assert_eq!(ModelFamily::Gemini.to_string(),  "Gemini");
        assert_eq!(ModelFamily::Copilot.to_string(), "Copilot");
        assert_eq!(ModelFamily::Human.to_string(),   "Human");
    }
}
