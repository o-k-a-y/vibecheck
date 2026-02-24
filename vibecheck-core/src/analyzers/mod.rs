pub mod cst;
pub mod text;

use crate::language::Language;
use crate::report::{Signal, SymbolMetadata};

/// Trait for text-pattern source code analyzers.
pub trait Analyzer: Send + Sync {
    /// A short name identifying this analyzer.
    fn name(&self) -> &str;

    /// Analyze Rust source code (the default / fallback language).
    fn analyze(&self, source: &str) -> Vec<Signal>;

    /// Analyze Rust source (alias used by the dispatch table).
    /// Defaults to [`analyze`].
    fn analyze_rust(&self, source: &str) -> Vec<Signal> {
        self.analyze(source)
    }

    /// Analyze Python source.  Defaults to [`analyze`] when not overridden.
    fn analyze_python(&self, source: &str) -> Vec<Signal> {
        self.analyze(source)
    }

    /// Analyze JavaScript / TypeScript source.  Defaults to [`analyze`].
    fn analyze_javascript(&self, source: &str) -> Vec<Signal> {
        self.analyze(source)
    }

    /// Analyze Go source.  Defaults to [`analyze`].
    fn analyze_go(&self, source: &str) -> Vec<Signal> {
        self.analyze(source)
    }

    /// Fully-provided language dispatch — **never override**.
    ///
    /// Routes the call to the appropriate `analyze_<lang>` method based on
    /// the detected language.
    fn analyze_with_language(&self, source: &str, lang: Option<Language>) -> Vec<Signal> {
        match lang {
            None | Some(Language::Rust)       => self.analyze_rust(source),
            Some(Language::Python)            => self.analyze_python(source),
            Some(Language::JavaScript)        => self.analyze_javascript(source),
            Some(Language::Go)                => self.analyze_go(source),
        }
    }
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

    /// Extract named top-level symbols (functions, methods, classes, …) from
    /// the tree.  Returns `(metadata, node)` pairs where `node` covers the
    /// full symbol definition.
    ///
    /// The default implementation returns an empty vec; each language-specific
    /// analyzer overrides this.
    fn extract_symbols<'tree>(
        &self,
        _tree: &'tree tree_sitter::Tree,
        _source: &[u8],
    ) -> Vec<(SymbolMetadata, tree_sitter::Node<'tree>)> {
        vec![]
    }
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
