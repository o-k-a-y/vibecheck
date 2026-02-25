use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::SymbolMetadata;

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

    fn extract_metrics(
        &self,
        tree: &Tree,
        source: &str,
    ) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        let functions = collect_all_functions(root);

        if !functions.is_empty() {
            let complexities: Vec<usize> =
                functions.iter().map(|&f| complexity_of_fn(f)).collect();
            let avg =
                complexities.iter().sum::<usize>() as f64 / complexities.len() as f64;
            metrics.insert("avg_complexity".into(), avg);

            let depths: Vec<usize> =
                functions.iter().map(|&f| max_nesting_depth(f)).collect();
            let avg_depth =
                depths.iter().sum::<usize>() as f64 / depths.len() as f64;
            metrics.insert("avg_nesting_depth".into(), avg_depth);

            let lengths: Vec<usize> =
                functions.iter().map(|&f| fn_line_count(f)).collect();
            let avg_len =
                lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
            metrics.insert("avg_fn_length".into(), avg_len);
        }

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
            metrics.insert("doc_coverage_ratio".into(), ratio);
        }

        let identifiers = collect_identifiers(root, src_bytes);
        if identifiers.len() >= 10 {
            let entropy = shannon_entropy(&identifiers);
            metrics.insert("identifier_entropy".into(), entropy);
        }

        if imports_are_sorted(root, src_bytes) {
            metrics.insert("imports_sorted".into(), 1.0);
        } else {
            metrics.insert("imports_sorted".into(), 0.0);
        }

        let (comment_lines, code_lines) = inline_comment_ratio(&functions, src_bytes);
        if code_lines > 0 {
            metrics.insert(
                "inline_comment_ratio".into(),
                comment_lines as f64 / code_lines as f64,
            );
        }

        metrics
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
            if child.kind() != "function_item" {
                stack.push(child);
            }
        }
    }
    count
}

fn is_pub_fn(node: Node<'_>, src_bytes: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return child
                .utf8_text(src_bytes)
                .map(|t| t == "pub")
                .unwrap_or(false);
        }
    }
    false
}

fn has_preceding_doc_comment(node: Node<'_>, src_bytes: &[u8]) -> bool {
    let mut prev = node.prev_named_sibling();
    while let Some(n) = prev {
        match n.kind() {
            "line_comment" => {
                return n
                    .utf8_text(src_bytes)
                    .map(|t| t.starts_with("///"))
                    .unwrap_or(false);
            }
            "block_comment" => {
                return n
                    .utf8_text(src_bytes)
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

fn max_nesting_depth(root: Node<'_>) -> usize {
    let nesting_kinds = [
        "block",
        "if_expression",
        "match_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
    ];
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

fn fn_line_count(node: Node<'_>) -> usize {
    let start = node.start_position().row;
    let end = node.end_position().row;
    (end - start) + 1
}

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

fn inline_comment_ratio(functions: &[Node<'_>], src_bytes: &[u8]) -> (usize, usize) {
    let mut comment_lines = 0usize;
    let mut code_lines = 0usize;
    for &func in functions {
        let text = func.utf8_text(src_bytes).unwrap_or("");
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("//") {
                comment_lines += 1;
            } else {
                code_lines += 1;
            }
        }
    }
    (comment_lines, code_lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::CstAnalyzer;
    use crate::report::SymbolMetadata;

    fn parse_and_metrics(source: &str) -> HashMap<String, f64> {
        let analyzer = RustCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.extract_metrics(&tree, source)
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
        assert!(
            syms.iter().any(|s| s.name == "new" && s.kind == "method"),
            "expected 'new' as method; got: {:?}",
            syms
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "process" && s.kind == "method"),
            "expected 'process' as method; got: {:?}",
            syms
        );
        assert!(!syms.iter().any(|s| s.name == "Foo" && s.kind == "function"));
    }

    #[test]
    fn nested_functions_not_extracted() {
        let source = "fn outer() { fn inner() {} }\n";
        let syms = parse_and_extract(source);
        assert!(
            syms.iter().any(|s| s.name == "outer"),
            "expected 'outer'; got: {:?}",
            syms
        );
        assert!(
            !syms.iter().any(|s| s.name == "inner"),
            "inner should not appear; got: {:?}",
            syms
        );
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
    fn low_complexity_metrics() {
        let source = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
fn subtract(a: i32, b: i32) -> i32 { a - b }
fn multiply(a: i32, b: i32) -> i32 { a * b }
"#;
        let m = parse_and_metrics(source);
        assert!(m["avg_complexity"] <= 2.0, "expected low complexity; got {}", m["avg_complexity"]);
    }

    #[test]
    fn doc_coverage_metrics() {
        let source = r#"
/// Adds two numbers together.
pub fn add(a: i32, b: i32) -> i32 { a + b }

/// Subtracts b from a.
pub fn subtract(a: i32, b: i32) -> i32 { a - b }

/// Multiplies two numbers.
pub fn multiply(a: i32, b: i32) -> i32 { a * b }
"#;
        let m = parse_and_metrics(source);
        assert!(m["doc_coverage_ratio"] >= 0.9, "expected high doc coverage; got {}", m["doc_coverage_ratio"]);
    }

    #[test]
    fn sorted_imports_metrics() {
        let source = r#"
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

fn main() {}
"#;
        let m = parse_and_metrics(source);
        assert_eq!(m["imports_sorted"], 1.0);
    }

    #[test]
    fn avg_fn_length_computed() {
        let source = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let m = parse_and_metrics(source);
        assert!(m.contains_key("avg_fn_length"));
        assert!(m["avg_fn_length"] >= 1.0);
    }
}
