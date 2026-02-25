use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::SymbolMetadata;

pub struct PythonCstAnalyzer;

impl CstAnalyzer for PythonCstAnalyzer {
    fn name(&self) -> &str {
        "python_cst"
    }

    fn target_language(&self) -> Language {
        Language::Python
    }

    fn ts_language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
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
            let documented = functions.iter().filter(|&&f| has_docstring(f)).count();
            let ratio = documented as f64 / functions.len() as f64;
            metrics.insert("doc_coverage_ratio".into(), ratio);

            let lengths: Vec<usize> = functions.iter().map(|&f| fn_line_count(f)).collect();
            let avg_len = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;
            metrics.insert("avg_fn_length".into(), avg_len);

            let complexities: Vec<usize> =
                functions.iter().map(|&f| complexity_of_fn(f)).collect();
            let avg_complexity =
                complexities.iter().sum::<usize>() as f64 / complexities.len() as f64;
            metrics.insert("avg_complexity".into(), avg_complexity);

            let depths: Vec<usize> =
                functions.iter().map(|&f| max_nesting_depth(f)).collect();
            let avg_depth =
                depths.iter().sum::<usize>() as f64 / depths.len() as f64;
            metrics.insert("avg_nesting_depth".into(), avg_depth);
        }

        let (typed, total_params) = count_type_annotations(&functions, src_bytes);
        if total_params >= 5 {
            let ratio = typed as f64 / total_params as f64;
            metrics.insert("type_annotation_ratio".into(), ratio);
        }

        let (fstring_count, old_style_count) = count_string_styles(root, src_bytes);
        let total_fmt = fstring_count + old_style_count;
        if total_fmt > 0 {
            metrics.insert("fstring_ratio".into(), fstring_count as f64 / total_fmt as f64);
        } else if fstring_count > 0 {
            metrics.insert("fstring_ratio".into(), 1.0);
        }

        let identifiers = collect_identifiers(root, src_bytes);
        if identifiers.len() >= 10 {
            metrics.insert("identifier_entropy".into(), shannon_entropy(&identifiers));
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
        let mut stack = vec![(root, false)];

        while let Some((node, inside_class)) = stack.pop() {
            match node.kind() {
                "function_definition" => {
                    let kind = if inside_class { "method" } else { "function" };
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
                "class_definition" => {
                    if let Some(name) = node
                        .children(&mut node.walk())
                        .find(|c| c.kind() == "identifier")
                        .and_then(|c| c.utf8_text(source).ok())
                    {
                        results.push((
                            SymbolMetadata {
                                name: name.to_string(),
                                kind: "class".to_string(),
                                start_line: node.start_position().row + 1,
                                end_line: node.end_position().row + 1,
                            },
                            node,
                        ));
                    }
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
                stack.push((child, inside_class));
            }
        }

        results
    }
}

fn collect_all_functions<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_definition" {
            result.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    result
}

fn has_docstring(func: Node<'_>) -> bool {
    let mut cursor = func.walk();
    for child in func.children(&mut cursor) {
        if child.kind() == "block" {
            let mut block_cursor = child.walk();
            let first_stmt = child.named_children(&mut block_cursor).next();
            if let Some(stmt) = first_stmt {
                if stmt.kind() == "expression_statement" {
                    let mut stmt_cursor = stmt.walk();
                    for expr in stmt.named_children(&mut stmt_cursor) {
                        if expr.kind() == "string" {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn count_type_annotations(functions: &[Node<'_>], src_bytes: &[u8]) -> (usize, usize) {
    let mut typed = 0usize;
    let mut total = 0usize;

    for &func in functions {
        let mut cursor = func.walk();
        for child in func.children(&mut cursor) {
            if child.kind() == "parameters" {
                let mut param_cursor = child.walk();
                for param in child.named_children(&mut param_cursor) {
                    match param.kind() {
                        "typed_parameter" | "typed_default_parameter" => {
                            typed += 1;
                            total += 1;
                        }
                        "identifier" => {
                            if param
                                .utf8_text(src_bytes)
                                .map(|t| t != "self")
                                .unwrap_or(true)
                            {
                                total += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    (typed, total)
}

fn count_string_styles(root: Node<'_>, src_bytes: &[u8]) -> (usize, usize) {
    let mut fstrings = 0usize;
    let mut old_style = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "string" {
            if let Ok(text) = node.utf8_text(src_bytes) {
                let t = text.trim_start();
                if t.starts_with("f\"")
                    || t.starts_with("f'")
                    || t.starts_with("F\"")
                    || t.starts_with("F'")
                {
                    fstrings += 1;
                }
            }
        }
        if node.kind() == "binary_operator" {
            let left_is_string = node
                .child_by_field_name("left")
                .map(|n| n.kind() == "string")
                .unwrap_or(false);
            let op_is_percent = node
                .child_by_field_name("operator")
                .and_then(|n| n.utf8_text(src_bytes).ok())
                .map(|t| t == "%")
                .unwrap_or(false);
            if left_is_string && op_is_percent {
                old_style += 1;
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    (fstrings, old_style)
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
        "while_statement",
        "elif_clause",
        "except_clause",
    ];
    let mut count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if decision_kinds.contains(&node.kind()) {
            count += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "function_definition" {
                stack.push(child);
            }
        }
    }
    count
}

fn max_nesting_depth(root: Node<'_>) -> usize {
    let nesting_kinds = [
        "block",
        "if_statement",
        "for_statement",
        "while_statement",
        "try_statement",
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
            if child.kind() != "function_definition" {
                stack.push((child, new_depth));
            }
        }
    }
    max_depth
}

fn collect_identifiers<'s>(root: Node<'_>, src_bytes: &'s [u8]) -> Vec<&'s str> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "identifier" {
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
            if trimmed.starts_with('#') {
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
        let analyzer = PythonCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.extract_metrics(&tree, source)
    }

    fn parse_and_extract(source: &str) -> Vec<SymbolMetadata> {
        let analyzer = PythonCstAnalyzer;
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
    fn extract_module_level_functions() {
        let source = "def foo():\n    pass\n\ndef bar(x):\n    return x\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "foo" && s.kind == "function"));
        assert!(syms.iter().any(|s| s.name == "bar" && s.kind == "function"));
    }

    #[test]
    fn extract_class_and_methods() {
        let source =
            "class MyClass:\n    def __init__(self):\n        pass\n    def run(self):\n        pass\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "MyClass" && s.kind == "class"));
        assert!(syms.iter().any(|s| s.name == "__init__" && s.kind == "method"));
        assert!(syms.iter().any(|s| s.name == "run" && s.kind == "method"));
    }

    #[test]
    fn nested_functions_not_extracted() {
        let source = "def outer():\n    def inner():\n        pass\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "outer"));
        assert!(!syms.iter().any(|s| s.name == "inner"));
    }

    #[test]
    fn extract_symbol_line_numbers() {
        let source = "def first():\n    pass\ndef second():\n    pass\n";
        let syms = parse_and_extract(source);
        let first = syms.iter().find(|s| s.name == "first").unwrap();
        let second = syms.iter().find(|s| s.name == "second").unwrap();
        assert_eq!(first.start_line, 1);
        assert_eq!(second.start_line, 3);
    }

    #[test]
    fn docstring_coverage_metrics() {
        let source = r#"
def process(data):
    """Process the input data and return result."""
    return data

def validate(value):
    """Validate the given value against constraints."""
    return bool(value)

def transform(item):
    """Transform item into the required format."""
    return str(item)
"#;
        let m = parse_and_metrics(source);
        assert!(m["doc_coverage_ratio"] >= 0.85);
    }

    #[test]
    fn type_annotation_metrics() {
        let source = r#"
def add(a: int, b: int) -> int:
    return a + b

def greet(name: str, greeting: str) -> str:
    return f"{greeting}, {name}"

def compute(x: float, y: float, z: float) -> float:
    return x + y + z
"#;
        let m = parse_and_metrics(source);
        assert!(m["type_annotation_ratio"] >= 0.8);
    }

    #[test]
    fn fstring_metrics() {
        let source = r#"
def greet(name):
    return f"Hello, {name}"

def describe(item, count):
    return f"{count} items of type {item}"
"#;
        let m = parse_and_metrics(source);
        assert!(m["fstring_ratio"] >= 1.0);
    }
}
