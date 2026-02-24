use tree_sitter::{Node, Tree};

use crate::analyzers::CstAnalyzer;
use crate::language::Language;
use crate::report::{ModelFamily, Signal, SymbolMetadata};

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

    fn analyze_tree(&self, tree: &Tree, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let src_bytes = source.as_bytes();
        let root = tree.root_node();

        // --- Signal 1: Arrow function ratio ---
        let arrow_count = count_nodes_of_kind(root, "arrow_function");
        let named_fn_count = count_nodes_of_kind(root, "function_declaration")
            + count_nodes_of_kind(root, "function");
        let total_fns = arrow_count + named_fn_count;
        if total_fns >= 3 {
            let ratio = arrow_count as f64 / total_fns as f64;
            if ratio >= 0.7 {
                signals.push(Signal {
                    source: self.name().into(),
                    description: format!(
                        "{:.0}% arrow functions — modern JavaScript style",
                        ratio * 100.0
                    ),
                    family: ModelFamily::Claude,
                    weight: 1.5,
                });
            }
        }

        // --- Signal 2: async/await vs .then() chaining ---
        let await_count = count_nodes_of_kind(root, "await_expression");
        let then_count = count_then_calls(root, src_bytes);
        if await_count >= 2 && then_count == 0 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!(
                    "{await_count} await expressions, no .then() — modern async style"
                ),
                family: ModelFamily::Claude,
                weight: 1.5,
            });
        } else if then_count >= 2 && await_count == 0 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{then_count} .then() chains — promise chain style"),
                family: ModelFamily::Human,
                weight: 1.0,
            });
        }

        // --- Signal 3: Optional chaining (?.) density ---
        // Uses source text since the tree-sitter node representation of `?.`
        // varies across grammar versions; text matching is unambiguous.
        let optional_chain_count = source.matches("?.").count();
        if optional_chain_count >= 3 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!(
                    "{optional_chain_count} optional chaining usages (?.) — defensive modern style"
                ),
                family: ModelFamily::Claude,
                weight: 1.0,
            });
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
                    // Capture `const foo = () => ...` / `const foo = function() ...`
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

/// Count `.then(` call expressions via the CST.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::CstAnalyzer;
    use crate::report::{ModelFamily, SymbolMetadata};

    fn parse_and_run(source: &str) -> Vec<Signal> {
        let analyzer = JavaScriptCstAnalyzer;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&analyzer.ts_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        analyzer.analyze_tree(&tree, source)
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
        assert!(syms.iter().any(|s| s.name == "greet" && s.kind == "function"),
            "expected 'greet' as function; got: {:?}", syms);
    }

    #[test]
    fn extract_arrow_function_const() {
        let source = "const add = (a, b) => a + b;\nconst double = x => x * 2;\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "add" && s.kind == "function"),
            "expected 'add' as function; got: {:?}", syms);
        assert!(syms.iter().any(|s| s.name == "double" && s.kind == "function"),
            "expected 'double' as function; got: {:?}", syms);
    }

    #[test]
    fn extract_class_methods() {
        let source = "class Greeter {\n  sayHello() { return 'hi'; }\n  sayBye() { return 'bye'; }\n}\n";
        let syms = parse_and_extract(source);
        assert!(syms.iter().any(|s| s.name == "sayHello" && s.kind == "method"),
            "expected 'sayHello' as method; got: {:?}", syms);
        assert!(syms.iter().any(|s| s.name == "sayBye" && s.kind == "method"),
            "expected 'sayBye' as method; got: {:?}", syms);
    }

    #[test]
    fn non_function_const_not_extracted() {
        let source = "const VALUE = 42;\nconst NAME = 'hello';\n";
        let syms = parse_and_extract(source);
        assert!(syms.is_empty(), "non-function consts should not be extracted; got: {:?}", syms);
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
    fn high_arrow_ratio_is_claude() {
        let source = r#"
const add = (a, b) => a + b;
const double = x => x * 2;
const greet = name => `Hello, ${name}`;
const square = n => n * n;
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.5
                && s.description.contains("arrow")),
            "expected arrow function Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn async_await_is_claude() {
        let source = r#"
async function fetchData() {
    const response = await fetch('/api/data');
    const json = await response.json();
    return json;
}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.5
                && s.description.contains("await")),
            "expected async/await Claude signal; got: {:?}", signals
        );
    }

    #[test]
    fn then_chains_is_human() {
        let source = r#"
function loadData() {
    fetch('/api/data')
        .then(response => response.json())
        .then(data => render(data))
        .catch(err => console.error(err));
}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human
                && s.weight == 1.0
                && s.description.contains(".then()")),
            "expected .then() Human signal; got: {:?}", signals
        );
    }

    #[test]
    fn optional_chaining_is_claude() {
        let source = r#"
function process(user) {
    const city = user?.address?.city;
    const zip = user?.address?.zip;
    const country = user?.address?.country;
    return city;
}
"#;
        let signals = parse_and_run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude
                && s.weight == 1.0
                && s.description.contains("?.")),
            "expected optional chaining Claude signal; got: {:?}", signals
        );
    }
}
