use tree_sitter::Tree;

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::Signal;

pub struct GoCstAnalyzer;

impl CstAnalyzer for GoCstAnalyzer {
    fn name(&self) -> &str {
        "go_cst"
    }

    fn target_language(&self) -> Language {
        Language::Go
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn analyze_tree(&self, _tree: &Tree, _source: &str) -> Vec<Signal> {
        // TODO: planned signals:
        // 1. Error return coverage — functions returning error that check err != nil (high → Claude)
        // 2. godoc comment coverage on exported functions (high → Claude)
        // 3. Named return values usage (named → Claude)
        // 4. goroutine + channel usage vs mutex usage (channel-first → Claude)
        // 5. table-driven test pattern in _test.go files (present → Claude)
        Vec::new()
    }
}
