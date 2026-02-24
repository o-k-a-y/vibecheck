use std::path::Path;

/// Source languages supported by CST analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    Go,
}

/// Detect the language of a file from its extension.
pub fn detect_language(path: &Path) -> Option<Language> {
    match path.extension()?.to_str()? {
        "rs" => Some(Language::Rust),
        "py" => Some(Language::Python),
        "js" | "ts" | "jsx" | "tsx" => Some(Language::JavaScript),
        "go" => Some(Language::Go),
        _ => None,
    }
}

/// Get the tree-sitter grammar for a given language.
pub fn get_ts_language(lang: Language) -> tree_sitter::Language {
    match lang {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
    }
}
