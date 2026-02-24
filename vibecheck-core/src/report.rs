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
