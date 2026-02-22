use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::{ModelFamily, Signal};

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

    fn analyze_tree(&self, tree: &Tree, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        // --- Signal 1: Godoc coverage on exported functions ---
        let exported_fns = collect_exported_functions(root, src_bytes);
        if !exported_fns.is_empty() {
            let documented = exported_fns
                .iter()
                .filter(|&&n| has_preceding_comment(n))
                .count();
            let ratio = documented as f64 / exported_fns.len() as f64;
            if ratio >= 0.8 {
                signals.push(Signal {
                    source: self.name().into(),
                    description: format!(
                        "Godoc coverage {:.0}% on exported functions — thorough documentation",
                        ratio * 100.0
                    ),
                    family: ModelFamily::Claude,
                    weight: 2.0,
                });
            }
        }

        // --- Signal 2: Goroutine usage ---
        let goroutine_count = count_nodes_of_kind(root, "go_statement");
        if goroutine_count >= 2 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{goroutine_count} goroutines — concurrent design"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        // --- Signal 3: err != nil check density ---
        // AI-written Go almost always checks every error; human code sometimes skips checks.
        let fn_count = count_nodes_of_kind(root, "function_declaration")
            + count_nodes_of_kind(root, "method_declaration");
        let err_checks = count_err_nil_checks(root, src_bytes);
        if fn_count >= 2 && err_checks >= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!(
                    "{err_checks} err != nil checks — thorough error handling"
                ),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
        }

        signals
    }
}

/// Collect exported `function_declaration` and `method_declaration` nodes
/// (those whose name starts with an uppercase letter).
fn collect_exported_functions<'t>(root: Node<'t>, src_bytes: &[u8]) -> Vec<Node<'t>> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
            if let Some(name) = get_function_name(node, src_bytes) {
                if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    result.push(node);
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    result
}

/// Get the name identifier of a function or method declaration.
fn get_function_name<'s>(node: Node<'_>, src_bytes: &'s [u8]) -> Option<&'s str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "field_identifier" {
            return child.utf8_text(src_bytes).ok();
        }
    }
    None
}

/// Check if the immediately preceding named sibling is a comment.
fn has_preceding_comment(node: Node<'_>) -> bool {
    node.prev_named_sibling()
        .map(|n| n.kind() == "comment")
        .unwrap_or(false)
}

/// Count nodes of a specific kind throughout the tree.
fn count_nodes_of_kind(root: Node<'_>, kind: &str) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == kind {
            count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    count
}

/// Count `if_statement` nodes whose text contains both "err" and "nil",
/// indicating an `err != nil` or `err == nil` guard.
fn count_err_nil_checks(root: Node<'_>, src_bytes: &[u8]) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "if_statement" {
            let text = node.utf8_text(src_bytes).unwrap_or("");
            let first_line = text.lines().next().unwrap_or("");
            if first_line.contains("err") && first_line.contains("nil") {
                count += 1;
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::CstAnalyzer;
    use crate::report::ModelFamily;

    fn parse_and_run(source: &str) -> Vec<Signal> {
        let analyzer = GoCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.analyze_tree(&tree, source)
    }

    #[test]
    fn godoc_coverage_is_claude() {
        let source = r#"package main

// Foo does something important.
func Foo() {}

// Bar processes the input data.
func Bar() {}

// Baz handles all incoming requests.
func Baz() {}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 2.0
                && s.description.contains("Godoc")),
            "expected godoc coverage Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn goroutines_is_claude() {
        let source = r#"package main

func main() {
    go worker()
    go worker()
}

func worker() {}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.0
                && s.description.contains("goroutine")),
            "expected goroutine Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn err_nil_checks_is_claude() {
        let source = r#"package main

import "errors"

func Foo() error { return nil }
func Bar() error { return nil }
func Baz() error { return nil }

func Run() {
    if err := Foo(); err != nil {
        return
    }
    if err := Bar(); err != nil {
        return
    }
    if err := Baz(); err != nil {
        return
    }
}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.5
                && s.description.contains("err")),
            "expected err != nil Claude signal; got: {:?}", signals
        );
    }
}
