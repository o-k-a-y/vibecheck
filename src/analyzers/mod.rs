pub mod ai_signals;
pub mod code_structure;
pub mod comment_style;
pub mod error_handling;
pub mod idiom_usage;
pub mod naming;

use crate::report::Signal;

/// Trait for all source code analyzers.
pub trait Analyzer: Send + Sync {
    /// A short name identifying this analyzer.
    fn name(&self) -> &str;

    /// Analyze the given source code and return signals.
    fn analyze(&self, source: &str) -> Vec<Signal>;
}

/// Returns the default set of analyzers.
pub fn default_analyzers() -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(comment_style::CommentStyleAnalyzer),
        Box::new(ai_signals::AiSignalsAnalyzer),
        Box::new(error_handling::ErrorHandlingAnalyzer),
        Box::new(naming::NamingAnalyzer),
        Box::new(code_structure::CodeStructureAnalyzer),
        Box::new(idiom_usage::IdiomUsageAnalyzer),
    ]
}
