pub mod cst;
pub mod text;

use crate::language::Language;
use crate::report::Signal;

/// Trait for text-pattern source code analyzers.
pub trait Analyzer: Send + Sync {
    /// A short name identifying this analyzer.
    fn name(&self) -> &str;

    /// Analyze the given source code and return signals.
    fn analyze(&self, source: &str) -> Vec<Signal>;
}

/// Trait for tree-sitter CST analyzers.
pub trait CstAnalyzer: Send + Sync {
    /// A short name identifying this analyzer.
    fn name(&self) -> &str;

    /// The language this analyzer targets (used for dispatch).
    fn target_language(&self) -> Language;

    /// The tree-sitter grammar language for parsing.
    fn ts_language(&self) -> tree_sitter::Language;

    /// Analyze the parsed CST and return signals.
    fn analyze_tree(&self, tree: &tree_sitter::Tree, source: &str) -> Vec<Signal>;
}

/// Returns the default set of text analyzers.
pub fn default_analyzers() -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(text::comment_style::CommentStyleAnalyzer),
        Box::new(text::ai_signals::AiSignalsAnalyzer),
        Box::new(text::error_handling::ErrorHandlingAnalyzer),
        Box::new(text::naming::NamingAnalyzer),
        Box::new(text::code_structure::CodeStructureAnalyzer),
        Box::new(text::idiom_usage::IdiomUsageAnalyzer),
    ]
}

/// Returns the default set of CST analyzers.
pub fn default_cst_analyzers() -> Vec<Box<dyn CstAnalyzer>> {
    vec![
        Box::new(cst::rust::RustCstAnalyzer),
        Box::new(cst::python::PythonCstAnalyzer),
        Box::new(cst::javascript::JavaScriptCstAnalyzer),
        Box::new(cst::go::GoCstAnalyzer),
    ]
}
