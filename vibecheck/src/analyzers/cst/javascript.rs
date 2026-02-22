use tree_sitter::Tree;

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::Signal;

pub struct JavaScriptCstAnalyzer;

impl CstAnalyzer for JavaScriptCstAnalyzer {
    fn name(&self) -> &str {
        "js_cst"
    }

    fn target_language(&self) -> Language {
        Language::JavaScript
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn analyze_tree(&self, _tree: &Tree, _source: &str) -> Vec<Signal> {
        // TODO: planned signals:
        // 1. Arrow function ratio vs function declarations (high ratio → Claude)
        // 2. Optional chaining (?.) usage density (high → Claude)
        // 3. async/await vs .then() chaining (async/await → Claude)
        // 4. Destructuring assignment coverage (high → Claude)
        // 5. JSDoc comment coverage on exported functions (high → Claude)
        Vec::new()
    }
}
