use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::SymbolMetadata;

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

    fn extract_metrics(
        &self,
        tree: &Tree,
        source: &str,
    ) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        let arrow_count = count_nodes_of_kind(root, "arrow_function");
        let named_fn_count = count_nodes_of_kind(root, "function_declaration")
            + count_nodes_of_kind(root, "function");
        let total_fns = arrow_count + named_fn_count;
        if total_fns >= 3 {
            metrics.insert(
                "arrow_fn_ratio".into(),
                arrow_count as f64 / total_fns as f64,
            );
        }

        let await_count = count_nodes_of_kind(root, "await_expression");
        let then_count = count_then_calls(root, src_bytes);
        if await_count >= 2 && then_count == 0 {
            metrics.insert("await_only".into(), 1.0);
        } else if then_count >= 2 && await_count == 0 {
            metrics.insert("then_only".into(), 1.0);
        }
        if await_count >= 1 && then_count >= 1 {
            metrics.insert("has_mixed_async".into(), 1.0);
        }

        let optional_chain_count = source.matches("?.").count();
        metrics.insert("optional_chain_count".into(), optional_chain_count as f64);

        let all_fns = collect_all_functions(root);
        if !all_fns.is_empty() {
            let lengths: Vec<usize> = all_fns.iter().map(|&f| fn_line_count(f)).collect();
            let avg_len = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
            metrics.insert("avg_fn_length".into(), avg_len);

            let complexities: Vec<usize> =
                all_fns.iter().map(|&f| complexity_of_fn(f)).collect();
            let avg_complexity =
                complexities.iter().sum::<usize>() as f64 / complexities.len() as f64;
            metrics.insert("avg_complexity".into(), avg_complexity);

            let depths: Vec<usize> =
                all_fns.iter().map(|&f| max_nesting_depth(f)).collect();
            let avg_depth =
                depths.iter().sum::<usize>() as f64 / depths.len() as f64;
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

        let (template_count, total_strings) = count_template_literals(root, src_bytes);
        if total_strings > 0 {
            metrics.insert(
                "template_literal_ratio".into(),
                template_count as f64 / total_strings as f64,
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
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            match node.kind() {
                "function_declaration" | "generator_function_declaration" => {
                    if let Some(name) = node
                        .children(&mut node.walk())
                        .find(|c| c.kind() == "identifier")
                        .and_then(|c| c.utf8_text(source).ok())
                    {
                        results.push((
                            SymbolMetadata {
                                name: name.to_string(),
                                kind: "function".to_string(),
                                start_line: node.start_position().row + 1,
                                end_line: node.end_position().row + 1,
                            },
                            node,
                        ));
                    }
                }
                "method_definition" => {
                    if let Some(name) = node
                        .children(&mut node.walk())
                        .find(|c| {
                            c.kind() == "property_identifier" || c.kind() == "identifier"
                        })
                        .and_then(|c| c.utf8_text(source).ok())
                    {
                        results.push((
                            SymbolMetadata {
                                name: name.to_string(),
                                kind: "method".to_string(),
                                start_line: node.start_position().row + 1,
                                end_line: node.end_position().row + 1,
                            },
                            node,
                        ));
                    }
                }
                "variable_declarator" => {
                    let has_fn = node.children(&mut node.walk()).any(|c| {
                        matches!(
                            c.kind(),
                            "arrow_function"
                                | "function_expression"
                                | "generator_function_expression"
                        )
                    });
                    if has_fn {
                        if let Some(name) = node
                            .children(&mut node.walk())
                            .find(|c| c.kind() == "identifier")
                            .and_then(|c| c.utf8_text(source).ok())
                        {
                            results.push((
                                SymbolMetadata {
                                    name: name.to_string(),
                                    kind: "function".to_string(),
                                    start_line: node.start_position().row + 1,
                                    end_line: node.end_position().row + 1,
                                },
                                node,
                            ));
                        }
                    }
                }
                _ => {}
            }
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }

        results
    }
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

fn count_then_calls(root: Node<'_>, src_bytes: &[u8]) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "call_expression" {
            if let Some(func) = node.child_by_field_name("function") {
                if func.kind() == "member_expression" {
                    if let Some(prop) = func.child_by_field_name("property") {
                        if prop.utf8_text(src_bytes).ok() == Some("then") {
                            count += 1;
                        }
                    }
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

fn collect_all_functions<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    let fn_kinds = [
        "function_declaration",
        "function",
        "arrow_function",
        "method_definition",
    ];
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if fn_kinds.contains(&node.kind()) {
            result.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    result
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
        "for_in_statement",
        "while_statement",
        "switch_statement",
        "ternary_expression",
    ];
    let fn_kinds = [
        "function_declaration",
        "function",
        "arrow_function",
        "method_definition",
    ];
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
        "statement_block",
        "if_statement",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "switch_statement",
    ];
    let fn_kinds = [
        "function_declaration",
        "function",
        "arrow_function",
        "method_definition",
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
        if node.kind() == "identifier" || node.kind() == "property_identifier" {
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

fn count_template_literals(root: Node<'_>, _src_bytes: &[u8]) -> (usize, usize) {
    let mut template_count = 0usize;
    let mut string_count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "template_string" => {
                template_count += 1;
                string_count += 1;
            }
            "string" => {
                string_count += 1;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    (template_count, string_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::CstAnalyzer;
    use crate::report::SymbolMetadata;

    fn parse_and_metrics(source: &str) -> HashMap<String, f64> {
        let analyzer = JavaScriptCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.extract_metrics(&tree, source)
    }

    fn parse_and_extract(source: &str) -> Vec<SymbolMetadata> {
        let analyzer = JavaScriptCstAnalyzer;
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
    fn extract_function_declaration() {
        let source = "function greet(name) { return name; }\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "greet" && s.kind == "function"));
    }

    #[test]
    fn extract_arrow_function_const() {
        let source = "const add = (a, b) => a + b;\nconst double = x => x * 2;\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "add" && s.kind == "function"));
        assert!(syms.iter().any(|s| s.name == "double" && s.kind == "function"));
    }

    #[test]
    fn extract_class_methods() {
        let source =
            "class Greeter {\n  sayHello() { return 'hi'; }\n  sayBye() { return 'bye'; }\n}\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "sayHello" && s.kind == "method"));
        assert!(syms.iter().any(|s| s.name == "sayBye" && s.kind == "method"));
    }

    #[test]
    fn non_function_const_not_extracted() {
        let source = "const VALUE = 42;\nconst NAME = 'hello';\n";
        let syms = parse_and_extract(source);
        assert!(syms.is_empty());
    }

    #[test]
    fn extract_symbol_line_numbers() {
        let source = "function first() {}\nfunction second() {}\n";
        let syms = parse_and_extract(source);
        let first = syms.iter().find(|s| s.name == "first").unwrap();
        let second = syms.iter().find(|s| s.name == "second").unwrap();
        assert_eq!(first.start_line, 1);
        assert_eq!(second.start_line, 2);
    }

    #[test]
    fn high_arrow_ratio_metrics() {
        let source = r#"
const add = (a, b) => a + b;
const double = x => x * 2;
const greet = name => `Hello, ${name}`;
const square = n => n * n;
"#;
        let m = parse_and_metrics(source);
        assert!(m["arrow_fn_ratio"] >= 0.7);
    }

    #[test]
    fn async_await_metrics() {
        let source = r#"
async function fetchData() {
    const response = await fetch('/api/data');
    const json = await response.json();
    return json;
}
"#;
        let m = parse_and_metrics(source);
        assert_eq!(m["await_only"], 1.0);
    }

    #[test]
    fn then_chains_metrics() {
        let source = r#"
function loadData() {
    fetch('/api/data')
        .then(response => response.json())
        .then(data => render(data))
        .catch(err => console.error(err));
}
"#;
        let m = parse_and_metrics(source);
        assert_eq!(m["then_only"], 1.0);
    }

    #[test]
    fn optional_chaining_metrics() {
        let source = r#"
function process(user) {
    const city = user?.address?.city;
    const zip = user?.address?.zip;
    const country = user?.address?.country;
    return city;
}
"#;
        let m = parse_and_metrics(source);
        assert!(m["optional_chain_count"] >= 3.0);
    }
}
