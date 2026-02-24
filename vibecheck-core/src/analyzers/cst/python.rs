use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::heuristics::signal_ids;
use crate::language::Language;
use crate::report::{ModelFamily, Signal, SymbolMetadata};

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

    fn analyze_tree(&self, tree: &Tree, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        let functions = collect_all_functions(root);

        // --- Signal 1: Docstring coverage ---
        if !functions.is_empty() {
            let documented = functions
                .iter()
                .filter(|&&f| has_docstring(f))
                .count();
            let ratio = documented as f64 / functions.len() as f64;
            if ratio >= 0.85 {
                signals.push(Signal::new(
                    signal_ids::PYTHON_CST_DOC_COVERAGE_HIGH,
                    "python_cst",
                    format!(
                        "Docstring coverage {:.0}% — thorough documentation",
                        ratio * 100.0
                    ),
                    ModelFamily::Claude,
                    2.0,
                ));
            }
        }

        // --- Signal 2: Type annotation coverage ---
        let (typed, total_params) = count_type_annotations(&functions, src_bytes);
        if total_params >= 5 {
            let ratio = typed as f64 / total_params as f64;
            if ratio >= 0.8 {
                signals.push(Signal::new(
                    signal_ids::PYTHON_CST_TYPE_ANNOTATIONS_HIGH,
                    "python_cst",
                    format!(
                        "Type annotation coverage {:.0}% on parameters — modern Python style",
                        ratio * 100.0
                    ),
                    ModelFamily::Claude,
                    1.5,
                ));
            }
        }

        // --- Signal 3: f-string usage ---
        let (fstring_count, old_style_count) = count_string_styles(root, src_bytes);
        if fstring_count > 0 && old_style_count == 0 {
            signals.push(Signal::new(
                signal_ids::PYTHON_CST_FSTRINGS_ONLY,
                "python_cst",
                format!("{fstring_count} f-strings, no %-formatting — modern Python idiom"),
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
        let mut stack = vec![(root, false)]; // (node, inside_class)

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
                    // Don't recurse into nested functions.
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
                    // Recurse into the class body to pick up methods.
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

/// Collect all `function_definition` nodes in the tree.
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

/// Check whether a `function_definition`'s body starts with a docstring.
fn has_docstring(func: Node<'_>) -> bool {
    // Find the `block` child of the function
    let mut cursor = func.walk();
    for child in func.children(&mut cursor) {
        if child.kind() == "block" {
            // Check if the first named child is an expression_statement containing a string
            let mut block_cursor = child.walk();
            for stmt in child.named_children(&mut block_cursor) {
                if stmt.kind() == "expression_statement" {
                    let mut stmt_cursor = stmt.walk();
                    for expr in stmt.named_children(&mut stmt_cursor) {
                        if expr.kind() == "string" {
                            return true;
                        }
                    }
                }
                // Only check the first statement
                break;
            }
        }
    }
    false
}

/// Count typed vs untyped parameters across all functions.
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
                            // Check it's not `self`
                            if param.utf8_text(src_bytes).map(|t| t != "self").unwrap_or(true) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::CstAnalyzer;
    use crate::report::{ModelFamily, SymbolMetadata};

    fn parse_and_run(source: &str) -> Vec<Signal> {
        let analyzer = PythonCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.analyze_tree(&tree, source)
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
        assert!(syms.iter().any(|s| s.name == "foo" && s.kind == "function"),
            "expected 'foo' as function; got: {:?}", syms);
        assert!(syms.iter().any(|s| s.name == "bar" && s.kind == "function"),
            "expected 'bar' as function; got: {:?}", syms);
    }

    #[test]
    fn extract_class_and_methods() {
        let source = "class MyClass:\n    def __init__(self):\n        pass\n    def run(self):\n        pass\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "MyClass" && s.kind == "class"),
            "expected 'MyClass' as class; got: {:?}", syms);
        assert!(syms.iter().any(|s| s.name == "__init__" && s.kind == "method"),
            "expected '__init__' as method; got: {:?}", syms);
        assert!(syms.iter().any(|s| s.name == "run" && s.kind == "method"),
            "expected 'run' as method; got: {:?}", syms);
    }

    #[test]
    fn nested_functions_not_extracted() {
        let source = "def outer():\n    def inner():\n        pass\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "outer"),
            "expected 'outer'; got: {:?}", syms);
        assert!(!syms.iter().any(|s| s.name == "inner"),
            "inner should not appear; got: {:?}", syms);
    }

    #[test]
    fn extract_symbol_line_numbers() {
        let source = "def first():\n    pass\ndef second():\n    pass\n";
        let syms = parse_and_extract(source);
        let first = syms.iter().find(|s| s.name == "first").unwrap();
        let second = syms.iter().find(|s| s.name == "second").unwrap();
        assert_eq!(first.start_line, 1);
        assert_eq!(second.start_line, 3);
        assert!(first.end_line >= first.start_line);
    }

    #[test]
    fn docstring_coverage_is_claude() {
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
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 2.0
                && s.description.contains("Docstring")),
            "expected docstring coverage Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn type_annotation_coverage_is_claude() {
        let source = r#"
def add(a: int, b: int) -> int:
    return a + b

def greet(name: str, greeting: str) -> str:
    return f"{greeting}, {name}"

def compute(x: float, y: float, z: float) -> float:
    return x + y + z
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.5
                && s.description.contains("annotation")),
            "expected type annotation Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn fstring_only_is_claude() {
        let source = r#"
def greet(name):
    return f"Hello, {name}"

def describe(item, count):
    return f"{count} items of type {item}"
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.0
                && s.description.contains("f-string")),
            "expected f-string Claude signal; got: {:?}", signals
        );
    }
}

/// Count f-strings and old-style %-formatted strings in the tree.
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
        // Detect old-style % formatting: binary_operator whose operator is "%" and left is string.
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
