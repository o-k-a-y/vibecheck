use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::SymbolMetadata;

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

    fn extract_metrics(
        &self,
        tree: &Tree,
        source: &str,
    ) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        let exported_fns = collect_exported_functions(root, src_bytes);
        if !exported_fns.is_empty() {
            let documented = exported_fns
                .iter()
                .filter(|&&n| has_preceding_comment(n))
                .count();
            let ratio = documented as f64 / exported_fns.len() as f64;
            metrics.insert("doc_coverage_ratio".into(), ratio);
            metrics.insert("exported_fn_count".into(), exported_fns.len() as f64);
        }

        let goroutine_count = count_nodes_of_kind(root, "go_statement");
        metrics.insert("goroutine_count".into(), goroutine_count as f64);

        let fn_count = count_nodes_of_kind(root, "function_declaration")
            + count_nodes_of_kind(root, "method_declaration");
        let err_checks = count_err_nil_checks(root, src_bytes);
        metrics.insert("err_nil_check_count".into(), err_checks as f64);
        if fn_count >= 2 {
            metrics.insert(
                "err_nil_check_ratio".into(),
                err_checks as f64 / fn_count as f64,
            );
        }

        let all_fns = collect_all_functions(root);
        if !all_fns.is_empty() {
            let lengths: Vec<usize> = all_fns.iter().map(|&f| fn_line_count(f)).collect();
            let avg_len = lengths.iter().sum::<usize>() as f64 / all_fns.len() as f64;
            metrics.insert("avg_fn_length".into(), avg_len);

            let complexities: Vec<usize> =
                all_fns.iter().map(|&f| complexity_of_fn(f)).collect();
            let avg_complexity =
                complexities.iter().sum::<usize>() as f64 / all_fns.len() as f64;
            metrics.insert("avg_complexity".into(), avg_complexity);

            let depths: Vec<usize> =
                all_fns.iter().map(|&f| max_nesting_depth(f)).collect();
            let avg_depth =
                depths.iter().sum::<usize>() as f64 / all_fns.len() as f64;
            metrics.insert("avg_nesting_depth".into(), avg_depth);
        }

        let identifiers = collect_identifiers(root, src_bytes);
        if identifiers.len() >= 10 {
            metrics.insert("identifier_entropy".into(), shannon_entropy(&identifiers));
        }

        let (comment_lines, code_lines) = inline_comment_ratio(&all_fns, src_bytes);
        if code_lines > 0 {
            metrics.insert(
                "inline_comment_ratio".into(),
                comment_lines as f64 / code_lines as f64,
            );
        }

        let named_returns = count_named_returns(root, src_bytes);
        metrics.insert("named_return_count".into(), named_returns as f64);

        metrics
    }

    fn extract_symbols<'tree>(
        &self,
        tree: &'tree tree_sitter::Tree,
        source: &[u8],
    ) -> Vec<(SymbolMetadata, tree_sitter::Node<'tree>)> {
        let root = tree.root_node();
        let mut results = Vec::new();
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            match node.kind() {
                "function_declaration" | "method_declaration" => {
                    let kind = if node.kind() == "method_declaration" {
                        "method"
                    } else {
                        "function"
                    };
                    if let Some(name) = get_function_name(node, source) {
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
                }
                _ => {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        stack.push(child);
                    }
                }
            }
        }

        results
    }
}

fn collect_exported_functions<'t>(root: Node<'t>, src_bytes: &[u8]) -> Vec<Node<'t>> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
            if let Some(name) = get_function_name(node, src_bytes) {
                if name
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
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

fn collect_all_functions<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
            result.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    result
}

fn get_function_name<'s>(node: Node<'_>, src_bytes: &'s [u8]) -> Option<&'s str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "field_identifier" {
            return child.utf8_text(src_bytes).ok();
        }
    }
    None
}

fn has_preceding_comment(node: Node<'_>) -> bool {
    node.prev_named_sibling()
        .map(|n| n.kind() == "comment")
        .unwrap_or(false)
}

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

fn fn_line_count(node: Node<'_>) -> usize {
    let start = node.start_position().row;
    let end = node.end_position().row;
    (end - start) + 1
}

fn complexity_of_fn(root: Node<'_>) -> usize {
    let decision_kinds = [
        "if_statement",
        "for_statement",
        "expression_switch_statement",
        "type_switch_statement",
        "select_statement",
    ];
    let fn_kinds = ["function_declaration", "method_declaration"];
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if decision_kinds.contains(&node.kind()) {
            count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child != root && fn_kinds.contains(&child.kind()) {
                continue;
            }
            stack.push(child);
        }
    }
    count
}

fn max_nesting_depth(root: Node<'_>) -> usize {
    let nesting_kinds = [
        "block",
        "if_statement",
        "for_statement",
        "expression_switch_statement",
    ];
    let fn_kinds = ["function_declaration", "method_declaration"];
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
            if child != root && fn_kinds.contains(&child.kind()) {
                continue;
            }
            stack.push((child, new_depth));
        }
    }
    max_depth
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

fn count_named_returns(root: Node<'_>, src_bytes: &[u8]) -> usize {
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "parameter_list" {
                    // The second parameter_list in a function is the result list.
                    // Check if it appears after the first one (the params).
                    let text = child.utf8_text(src_bytes).unwrap_or("");
                    // Named returns look like `(name Type, name2 Type)`
                    // vs unnamed `(Type, Type)`.
                    // A named return has identifiers before the type.
                    let mut inner_cursor = child.walk();
                    for param in child.named_children(&mut inner_cursor) {
                        if param.kind() == "parameter_declaration" {
                            count += 1;
                            break;
                        }
                    }
                    let _ = text;
                }
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
    use crate::report::SymbolMetadata;

    fn parse_and_metrics(source: &str) -> HashMap<String, f64> {
        let analyzer = GoCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.extract_metrics(&tree, source)
    }

    fn parse_and_extract(source: &str) -> Vec<SymbolMetadata> {
        let analyzer = GoCstAnalyzer;
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
    fn extract_top_level_functions() {
        let source = "package main\nfunc Foo() {}\nfunc Bar() int { return 0 }\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "Foo" && s.kind == "function"));
        assert!(syms.iter().any(|s| s.name == "Bar" && s.kind == "function"));
    }

    #[test]
    fn extract_method_declaration() {
        let source = "package main\ntype MyType struct{}\nfunc (m *MyType) Run() {}\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "Run" && s.kind == "method"));
    }

    #[test]
    fn extract_symbol_line_numbers() {
        let source = "package main\nfunc First() {}\nfunc Second() {}\n";
        let syms = parse_and_extract(source);
        let first = syms.iter().find(|s| s.name == "First").unwrap();
        let second = syms.iter().find(|s| s.name == "Second").unwrap();
        assert_eq!(first.start_line, 2);
        assert_eq!(second.start_line, 3);
    }

    #[test]
    fn godoc_coverage_metrics() {
        let source = r#"package main

// Foo does something important.
func Foo() {}

// Bar processes the input data.
func Bar() {}

// Baz handles all incoming requests.
func Baz() {}
"#;
        let m = parse_and_metrics(source);
        assert!(m["doc_coverage_ratio"] >= 0.8);
    }

    #[test]
    fn goroutine_metrics() {
        let source = r#"package main

func main() {
    go worker()
    go worker()
}

func worker() {}
"#;
        let m = parse_and_metrics(source);
        assert!(m["goroutine_count"] >= 2.0);
    }

    #[test]
    fn err_nil_check_metrics() {
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
        let m = parse_and_metrics(source);
        assert!(m["err_nil_check_count"] >= 3.0);
    }
}
