use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::analyzers::{default_analyzers, default_cst_analyzers, Analyzer, CstAnalyzer};
use crate::heuristics::{DefaultHeuristics, HeuristicsProvider};
use crate::language::{detect_language, get_ts_language};
use crate::report::{Attribution, ModelFamily, Report, ReportMetadata, Signal, SymbolReport};

/// Orchestrates analyzers and aggregates their signals into a report.
pub struct Pipeline {
    analyzers: Vec<Box<dyn Analyzer>>,
    cst_analyzers: Vec<Box<dyn CstAnalyzer>>,
    heuristics: Box<dyn HeuristicsProvider>,
}

impl Pipeline {
    /// Construct with explicit heuristics control.
    ///
    /// This is the preferred constructor for production code that loads a
    /// `.vibecheck` config (via [`crate::heuristics::ConfiguredHeuristics`])
    /// and for integration tests that need a specific weight table.
    pub fn with_heuristics(
        analyzers: Vec<Box<dyn Analyzer>>,
        cst_analyzers: Vec<Box<dyn CstAnalyzer>>,
        heuristics: Box<dyn HeuristicsProvider>,
    ) -> Self {
        Self {
            analyzers,
            cst_analyzers,
            heuristics,
        }
    }

    /// Construct with default heuristics and the standard analyzer set.
    pub fn with_defaults() -> Self {
        Self::with_heuristics(
            default_analyzers(),
            default_cst_analyzers(),
            Box::new(DefaultHeuristics),
        )
    }

    pub fn run(&self, source: &str, file_path: Option<PathBuf>) -> Report {
        // Detect language early so text analyzers can gate on it.
        let lang = file_path.as_ref().and_then(|p| detect_language(p));

        // Text-pattern analysis (language-aware)
        let mut signals: Vec<Signal> = self
            .analyzers
            .iter()
            .flat_map(|a| a.analyze_with_language(source, lang))
            .collect();

        // CST analysis — dispatch by Language enum
        if let Some(ref path) = file_path {
            if let Some(lang) = detect_language(path) {
                let ts_lang = crate::language::get_ts_language(lang);
                let mut parser = tree_sitter::Parser::new();
                if parser.set_language(&ts_lang).is_ok() {
                    if let Some(tree) = parser.parse(source.as_bytes(), None) {
                        for cst_analyzer in &self.cst_analyzers {
                            if cst_analyzer.target_language() == lang {
                                signals.extend(cst_analyzer.analyze_tree(&tree, source));
                            }
                        }
                    }
                }
            }
        }

        // Apply heuristic weight overrides and filter disabled signals.
        for s in &mut signals {
            if !s.id.is_empty() {
                s.weight = self.heuristics.weight(&s.id);
            }
        }
        signals.retain(|s| s.id.is_empty() || self.heuristics.is_enabled(&s.id));

        let attribution = self.aggregate(&signals);
        let lines_of_code = source.lines().count();
        let signal_count = signals.len();

        Report {
            attribution,
            signals,
            metadata: ReportMetadata {
                file_path,
                lines_of_code,
                signal_count,
            },
            symbol_reports: None,
        }
    }

    /// Analyze a file at the symbol level, returning one `SymbolReport` per
    /// extracted named symbol (function, method, class, …).
    ///
    /// Returns an empty `Vec` if the file language has no symbol analyzer or
    /// if the file cannot be parsed.
    pub fn run_symbols(&self, source: &[u8], file_path: &Path) -> anyhow::Result<Vec<SymbolReport>> {
        let lang = match detect_language(file_path) {
            Some(l) => l,
            None => return Ok(vec![]),
        };

        // Parse once and share the tree with both symbol extraction and
        // per-symbol signal collection.
        let ts_lang = get_ts_language(lang);
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&ts_lang)
            .map_err(|e| anyhow::anyhow!("tree-sitter language error: {e}"))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse file"))?;

        // Use the matching CstAnalyzer — it already knows the node kinds for
        // its language; no separate SymbolAnalyzer needed.
        let symbols: Vec<_> = self
            .cst_analyzers
            .iter()
            .find(|a| a.target_language() == lang)
            .map(|a| a.extract_symbols(&tree, source))
            .unwrap_or_default();

        let mut reports = Vec::new();
        for (metadata, node) in symbols {
            let range = node.byte_range();
            let symbol_bytes = source.get(range).unwrap_or(b"");
            let symbol_str = std::str::from_utf8(symbol_bytes).unwrap_or("");
            let sub_report = self.run(symbol_str, Some(file_path.to_path_buf()));
            reports.push(SymbolReport {
                metadata,
                attribution: sub_report.attribution,
                signals: sub_report.signals,
            });
        }

        Ok(reports)
    }

    fn aggregate(&self, signals: &[Signal]) -> Attribution {
        let mut raw_scores: HashMap<ModelFamily, f64> = HashMap::new();
        for family in ModelFamily::all() {
            raw_scores.insert(*family, 0.0);
        }

        for signal in signals {
            *raw_scores.entry(signal.family).or_insert(0.0) += signal.weight;
        }

        // Shift all scores so the minimum is 0
        let min_score = raw_scores.values().cloned().fold(f64::INFINITY, f64::min);
        let mut shifted: HashMap<ModelFamily, f64> = raw_scores
            .iter()
            .map(|(&k, &v)| (k, (v - min_score).max(0.0)))
            .collect();

        // Normalize to a distribution summing to 1.0
        let total: f64 = shifted.values().sum();
        if total > 0.0 {
            for v in shifted.values_mut() {
                *v /= total;
            }
        } else {
            let uniform = 1.0 / ModelFamily::all().len() as f64;
            for v in shifted.values_mut() {
                *v = uniform;
            }
        }

        let (primary, confidence) = shifted
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(&k, &v)| (k, v))
            .unwrap();

        Attribution {
            primary,
            confidence,
            scores: shifted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_symbols_returns_one_report_per_function() {
        let source = b"fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\n";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        std::fs::write(&path, source).unwrap();

        let pipeline = Pipeline::with_defaults();
        let reports = pipeline.run_symbols(source, &path).unwrap();

        assert_eq!(reports.len(), 2, "expected one report per function; got: {:?}",
            reports.iter().map(|r| &r.metadata.name).collect::<Vec<_>>());
        assert!(reports.iter().any(|r| r.metadata.name == "add"));
        assert!(reports.iter().any(|r| r.metadata.name == "sub"));
    }

    #[test]
    fn run_symbols_symbol_reports_have_attribution() {
        let source = b"fn documented() -> i32 { 42 }\n";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        std::fs::write(&path, source).unwrap();

        let pipeline = Pipeline::with_defaults();
        let reports = pipeline.run_symbols(source, &path).unwrap();

        assert_eq!(reports.len(), 1);
        // Confidence must be in [0, 1] and scores must sum to ~1.
        let attr = &reports[0].attribution;
        assert!(attr.confidence >= 0.0 && attr.confidence <= 1.0);
        let total: f64 = attr.scores.values().sum();
        assert!((total - 1.0).abs() < 0.01, "scores should sum to ~1.0; got {total}");
    }

    #[test]
    fn run_symbols_empty_for_unsupported_extension() {
        let source = b"hello world";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, source).unwrap();

        let pipeline = Pipeline::with_defaults();
        let reports = pipeline.run_symbols(source, &path).unwrap();
        assert!(reports.is_empty());
    }

    #[test]
    fn run_symbols_python_extracts_functions_and_methods() {
        let source = b"class Foo:\n    def bar(self):\n        pass\n\ndef baz():\n    pass\n";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.py");
        std::fs::write(&path, source).unwrap();

        let pipeline = Pipeline::with_defaults();
        let reports = pipeline.run_symbols(source, &path).unwrap();

        let names: Vec<&str> = reports.iter().map(|r| r.metadata.name.as_str()).collect();
        assert!(names.contains(&"Foo"), "expected 'Foo' class; got: {:?}", names);
        assert!(names.contains(&"bar"), "expected 'bar' method; got: {:?}", names);
        assert!(names.contains(&"baz"), "expected 'baz' function; got: {:?}", names);
    }
}
