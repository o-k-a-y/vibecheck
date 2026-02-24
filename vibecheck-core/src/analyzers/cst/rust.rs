use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::heuristics::signal_ids;
use crate::language::Language;
use crate::report::{ModelFamily, Signal, SymbolMetadata};

pub struct RustCstAnalyzer;

impl CstAnalyzer for RustCstAnalyzer {
    fn name(&self) -> &str {
        "rust_cst"
    }

    fn target_language(&self) -> Language {
        Language::Rust
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn analyze_tree(&self, tree: &Tree, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        // --- Signal 1: Cyclomatic complexity ---
        let functions = collect_all_functions(root);
        if !functions.is_empty() {
            let complexities: Vec<usize> = functions.iter().map(|&f| complexity_of_fn(f)).collect();
            let avg = complexities.iter().sum::<usize>() as f64 / complexities.len() as f64;
            if avg <= 2.0 {
                signals.push(Signal::new(
                    signal_ids::RUST_CST_COMPLEXITY_LOW,
                    "rust_cst",
                    format!("Low average cyclomatic complexity ({avg:.1}) — simple, linear functions"),
                    ModelFamily::Claude,
                    2.5,
                ));
            } else if avg >= 5.0 {
                signals.push(Signal::new(
                    signal_ids::RUST_CST_COMPLEXITY_HIGH,
                    "rust_cst",
                    format!("High average cyclomatic complexity ({avg:.1}) — complex branching"),
                    ModelFamily::Human,
                    1.5,
                ));
            }
        }

        // --- Signal 2: Doc comment coverage on pub functions ---
        let pub_fns: Vec<Node> = functions
            .iter()
            .copied()
            .filter(|&f| is_pub_fn(f, src_bytes))
            .collect();
        if !pub_fns.is_empty() {
            let documented = pub_fns
                .iter()
                .filter(|&&f| has_preceding_doc_comment(f, src_bytes))
                .count();
            let ratio = documented as f64 / pub_fns.len() as f64;
            if ratio >= 0.9 {
                signals.push(Signal::new(
                    signal_ids::RUST_CST_DOC_COVERAGE_HIGH,
                    "rust_cst",
                    format!(
                        "Doc comment coverage {:.0}% on pub functions — thorough documentation",
                        ratio * 100.0
                    ),
                    ModelFamily::Claude,
                    2.0,
                ));
            }
        }

        // --- Signal 3: Identifier entropy ---
        let identifiers = collect_identifiers(root, src_bytes);
        if identifiers.len() >= 10 {
            let entropy = shannon_entropy(&identifiers);
            if entropy >= 4.0 {
                signals.push(Signal::new(
                    signal_ids::RUST_CST_ENTROPY_HIGH,
                    "rust_cst",
                    format!("High identifier entropy ({entropy:.2}) — diverse, descriptive names"),
                    ModelFamily::Claude,
                    1.5,
                ));
            } else if entropy < 3.0 {
                signals.push(Signal::new(
                    signal_ids::RUST_CST_ENTROPY_LOW,
                    "rust_cst",
                    format!("Low identifier entropy ({entropy:.2}) — repetitive or terse names"),
                    ModelFamily::Human,
                    1.0,
                ));
            }
        }

        // --- Signal 4: Max nesting depth ---
        if !functions.is_empty() {
            let depths: Vec<usize> = functions.iter().map(|&f| max_nesting_depth(f)).collect();
            let avg_depth = depths.iter().sum::<usize>() as f64 / depths.len() as f64;
            if avg_depth <= 3.0 {
                signals.push(Signal::new(
                    signal_ids::RUST_CST_NESTING_LOW,
                    "rust_cst",
                    format!("Low average nesting depth ({avg_depth:.1}) — flat, readable structure"),
                    ModelFamily::Claude,
                    1.5,
                ));
            }
        }

        // --- Signal 5: Import ordering ---
        if imports_are_sorted(root, src_bytes) {
            signals.push(Signal::new(
                signal_ids::RUST_CST_IMPORTS_SORTED,
                "rust_cst",
                "use declarations are alphabetically sorted",
                ModelFamily::Claude,
                1.0,
            ));
        }

        signals
    }

    fn extract_symbols<'tree>(
        &self,
        tree: &'tree tree_sitter::Tree,
        source: &[u8],
    ) -> Vec<(SymbolMetadata, tree_sitter::Node<'tree>)> {
        let root = tree.root_node();
        let mut results = Vec::new();
        let mut stack = vec![(root, false)]; // (node, inside_impl)

        while let Some((node, inside_impl)) = stack.pop() {
            match node.kind() {
                "function_item" => {
                    let kind = if inside_impl { "method" } else { "function" };
                    if let Some(name) = node
                        .children(&mut node.walk())
                        .find(|c| c.kind() == "identifier")
                        .and_then(|c| c.utf8_text(source).ok())
                    {
                        results.push((
                            SymbolMetadata {
                                name: name.to_string(),
                                kind: kind.to_string(),
                                start_line: node.start_position().row + 1,
                                end_line: node.end_position().row + 1,
                            },
                            node,
                        ));
                    }
                    // Don't recurse — skip nested function items.
                    continue;
                }
                "impl_item" => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push((child, true));
                    }
                    continue;
                }
                _ => {}
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push((child, inside_impl));
            }
        }

        results
    }
}

/// Collect all `function_item` nodes anywhere in the tree (includes methods).
fn collect_all_functions<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_item" {
            result.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    result
}

/// Count decision-point nodes within a function, not recursing into nested functions.
fn complexity_of_fn(root: Node<'_>) -> usize {
    let decision_kinds = [
        "if_expression",
        "match_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
    ];
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if decision_kinds.contains(&node.kind()) {
            count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Stop at nested function boundaries
            if child.kind() != "function_item" {
                stack.push(child);
            }
        }
    }
    count
}

/// Check whether a `function_item` has `pub` visibility.
fn is_pub_fn(node: Node<'_>, src_bytes: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return child.utf8_text(src_bytes).map(|t| t == "pub").unwrap_or(false);
        }
    }
    false
}

/// Check whether the previous named sibling of a node is a doc comment.
/// Also skips over attribute items that may appear between the comment and function.
/// In tree-sitter-rust, doc comments (`///`) are `line_comment` nodes whose text
/// starts with `///`, not a distinct `line_doc_comment` kind.
fn has_preceding_doc_comment(node: Node<'_>, src_bytes: &[u8]) -> bool {
    let mut prev = node.prev_named_sibling();
    while let Some(n) = prev {
        match n.kind() {
            "line_comment" => {
                return n.utf8_text(src_bytes)
                    .map(|t| t.starts_with("///"))
                    .unwrap_or(false);
            }
            "block_comment" => {
                return n.utf8_text(src_bytes)
                    .map(|t| t.starts_with("/**"))
                    .unwrap_or(false);
            }
            "attribute_item" => {
                prev = n.prev_named_sibling();
            }
            _ => break,
        }
    }
    false
}

/// Collect text of all `identifier` and `field_identifier` nodes.
fn collect_identifiers<'s>(root: Node<'_>, src_bytes: &'s [u8]) -> Vec<&'s str> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "identifier" || node.kind() == "field_identifier" {
            if let Ok(text) = node.utf8_text(src_bytes) {
                result.push(text);
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    result
}

/// Shannon entropy of the character distribution in the concatenated identifier text.
fn shannon_entropy(identifiers: &[&str]) -> f64 {
    let combined: String = identifiers.join("");
    if combined.is_empty() {
        return 0.0;
    }
    let mut freq: HashMap<char, usize> = HashMap::new();
    for c in combined.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }
    let total = combined.chars().count() as f64;
    -freq
        .values()
        .map(|&count| {
            let p = count as f64 / total;
            p * p.log2()
        })
        .sum::<f64>()
}

/// Maximum nesting depth of block/if/match/loop nodes within a function,
/// not recursing into nested `function_item` nodes.
fn max_nesting_depth(root: Node<'_>) -> usize {
    let nesting_kinds = [
        "block",
        "if_expression",
        "match_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
    ];
    // Stack of (node, current_depth)
    let mut stack = vec![(root, 0usize)];
    let mut max_depth = 0usize;
    while let Some((node, depth)) = stack.pop() {
        let new_depth = if nesting_kinds.contains(&node.kind()) {
            depth + 1
        } else {
            depth
        };
        if new_depth > max_depth {
            max_depth = new_depth;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "function_item" {
                stack.push((child, new_depth));
            }
        }
    }
    max_depth
}

/// Check whether top-level `use_declaration` nodes are alphabetically sorted.
fn imports_are_sorted(root: Node<'_>, src_bytes: &[u8]) -> bool {
    let mut use_texts: Vec<String> = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "use_declaration" {
            if let Ok(text) = child.utf8_text(src_bytes) {
                use_texts.push(text.to_owned());
            }
        }
    }
    if use_texts.len() < 3 {
        return false;
    }
    use_texts.windows(2).all(|w| w[0] <= w[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::CstAnalyzer;
    use crate::report::{ModelFamily, SymbolMetadata};

    fn parse_and_run(source: &str) -> Vec<Signal> {
        let analyzer = RustCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.analyze_tree(&tree, source)
    }

    fn parse_and_extract(source: &str) -> Vec<SymbolMetadata> {
        let analyzer = RustCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer
            .extract_symbols(&tree, source.as_bytes())
            .into_iter()
            .map(|(meta, _)| meta)
            .collect()
    }

    #[test]
    fn extract_free_functions() {
        let source = "fn foo() {}\nfn bar(x: i32) -> i32 { x }\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "foo" && s.kind == "function"));
        assert!(syms.iter().any(|s| s.name == "bar" && s.kind == "function"));
    }

    #[test]
    fn extract_methods_from_impl() {
        let source = r#"
struct Foo;
impl Foo {
    fn new() -> Self { Foo }
    pub fn process(&self) -> i32 { 42 }
}
"#;
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "new" && s.kind == "method"),
            "expected 'new' as method; got: {:?}", syms);
        assert!(syms.iter().any(|s| s.name == "process" && s.kind == "method"),
            "expected 'process' as method; got: {:?}", syms);
        // The struct name itself is not a symbol we extract.
        assert!(!syms.iter().any(|s| s.name == "Foo" && s.kind == "function"));
    }

    #[test]
    fn nested_functions_not_extracted() {
        let source = "fn outer() { fn inner() {} }\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "outer"),
            "expected 'outer'; got: {:?}", syms);
        assert!(!syms.iter().any(|s| s.name == "inner"),
            "inner should not appear; got: {:?}", syms);
    }

    #[test]
    fn extract_symbol_line_numbers() {
        let source = "fn first() {}\nfn second() {}\n";
        let syms = parse_and_extract(source);
        let first = syms.iter().find(|s| s.name == "first").unwrap();
        let second = syms.iter().find(|s| s.name == "second").unwrap();
        assert_eq!(first.start_line, 1);
        assert_eq!(second.start_line, 2);
        assert!(first.end_line >= first.start_line);
        assert!(second.end_line >= second.start_line);
    }

    #[test]
    fn low_complexity_is_claude() {
        // Simple linear functions → avg cyclomatic complexity ≤ 2.0
        let source = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn subtract(a: i32, b: i32) -> i32 { a - b }
fn multiply(a: i32, b: i32) -> i32 { a * b }
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 2.5
                && s.description.contains("complexity")),
            "expected low complexity Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn doc_comment_coverage_is_claude() {
        // All pub functions preceded by doc comments
        let source = r#"
/// Adds two numbers together.
pub fn add(a: i32, b: i32) -> i32 { a + b }

/// Subtracts b from a.
pub fn subtract(a: i32, b: i32) -> i32 { a - b }

/// Multiplies two numbers.
pub fn multiply(a: i32, b: i32) -> i32 { a * b }
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 2.0
                && s.description.contains("Doc comment")),
            "expected doc comment coverage Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn sorted_imports_is_claude() {
        let source = r#"
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

fn main() {}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.0
                && s.description.contains("sorted")),
            "expected sorted imports Claude signal; got: {:?}", signals
        );
    }
}
