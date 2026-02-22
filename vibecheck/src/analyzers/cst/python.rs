use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::{ModelFamily, Signal};

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
                signals.push(Signal {
                    source: "python_cst".into(),
                    description: format!(
                        "Docstring coverage {:.0}% — thorough documentation",
                        ratio * 100.0
                    ),
                    family: ModelFamily::Claude,
                    weight: 2.0,
                });
            }
        }

        // --- Signal 2: Type annotation coverage ---
        let (typed, total_params) = count_type_annotations(&functions, src_bytes);
        if total_params >= 5 {
            let ratio = typed as f64 / total_params as f64;
            if ratio >= 0.8 {
                signals.push(Signal {
                    source: "python_cst".into(),
                    description: format!(
                        "Type annotation coverage {:.0}% on parameters — modern Python style",
                        ratio * 100.0
                    ),
                    family: ModelFamily::Claude,
                    weight: 1.5,
                });
            }
        }

        // --- Signal 3: f-string usage ---
        let (fstring_count, old_style_count) = count_string_styles(root, src_bytes);
        if fstring_count > 0 && old_style_count == 0 {
            signals.push(Signal {
                source: "python_cst".into(),
                description: format!("{fstring_count} f-strings, no %-formatting — modern Python idiom"),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
        }

        signals
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
