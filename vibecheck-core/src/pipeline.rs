use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::analyzers::{default_analyzers, default_cst_analyzers, Analyzer, CstAnalyzer};
use crate::heuristics::{all_heuristics, DefaultHeuristics, HeuristicLanguage, HeuristicsProvider};
use crate::language::{detect_language, get_ts_language, Language};
use crate::report::{Attribution, ModelFamily, Report, ReportMetadata, Signal, SymbolReport};

/// Match extracted CST metrics against TOML-defined threshold rules to produce signals.
pub(crate) fn match_metric_signals(
    metrics: &HashMap<String, f64>,
    language: HeuristicLanguage,
    heuristics: &dyn HeuristicsProvider,
) -> Vec<Signal> {
    let mut signals = Vec::new();
    for spec in all_heuristics() {
        if spec.language != language {
            continue;
        }
        let metric_name = match spec.metric {
            Some(m) => m,
            None => continue,
        };
        let op = match spec.op {
            Some(o) => o,
            None => continue,
        };
        let threshold = match spec.threshold {
            Some(t) => t,
            None => continue,
        };
        let value = match metrics.get(metric_name) {
            Some(&v) => v,
            None => continue,
        };

        let passes = match op {
            ">=" => value >= threshold,
            "<=" => value <= threshold,
            ">"  => value > threshold,
            "<"  => value < threshold,
            _    => false,
        };
        if !passes {
            continue;
        }
        if let Some(max) = spec.threshold_max {
            if value > max {
                continue;
            }
        }

        let weight = heuristics.weight(spec.id);
        if weight == 0.0 {
            continue;
        }

        let pct = value * 100.0;
        let description = spec.description
            .replace("{value}", &format!("{value}"))
            .replace("{value:.0}", &format!("{value:.0}"))
            .replace("{value:.1}", &format!("{value:.1}"))
            .replace("{value:.2}", &format!("{value:.2}"))
            .replace("{pct:.0}", &format!("{pct:.0}"))
            .replace("{pct:.1}", &format!("{pct:.1}"));

        signals.push(Signal::new(
            spec.id,
            spec.analyzer,
            description,
            spec.family,
            weight,
        ));
    }
    signals
}

/// Optional post-aggregation scorer that augments heuristic attribution
/// with ML model predictions. Defined in vibecheck-core (no ML deps);
/// implemented by vibecheck-ml's `EnsembleModel`.
pub trait PostScorer: Send + Sync {
    fn rescore(
        &self,
        signals: &[Signal],
        metrics: &HashMap<String, f64>,
        heuristic_attribution: &Attribution,
        language: Option<Language>,
        source: &str,
    ) -> Attribution;
}

/// Linearly interpolate two score distributions.
///
/// `blend = 0.0` → pure heuristic, `blend = 1.0` → pure ML.
fn blend_attributions(heuristic: &Attribution, ml: &Attribution, blend: f64) -> Attribution {
    let mut scores = HashMap::new();
    for family in ModelFamily::all() {
        let h = heuristic.scores.get(family).copied().unwrap_or(0.0);
        let m = ml.scores.get(family).copied().unwrap_or(0.0);
        scores.insert(*family, (1.0 - blend) * h + blend * m);
    }

    let (primary, confidence) = scores
        .iter()
        .max_by(|a, b| {
            a.1.partial_cmp(b.1)
                .unwrap()
                .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
        })
        .map(|(&k, &v)| (k, v))
        .unwrap();

    Attribution {
        primary,
        confidence,
        scores,
    }
}

/// Orchestrates analyzers and aggregates their signals into a report.
pub struct Pipeline {
    analyzers: Vec<Box<dyn Analyzer>>,
    cst_analyzers: Vec<Box<dyn CstAnalyzer>>,
    heuristics: Box<dyn HeuristicsProvider>,
    scorer: Option<Box<dyn PostScorer>>,
    ml_blend: f64,
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
            scorer: None,
            ml_blend: 0.0,
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

    /// Construct with an ML model scorer that blends with heuristic results.
    ///
    /// `blend` controls the mix: `0.0` = pure heuristic, `1.0` = pure ML,
    /// `0.5` = equal weight.  The scorer's [`PostScorer::rescore`] is called
    /// after heuristic aggregation in [`run`].
    pub fn with_model(
        analyzers: Vec<Box<dyn Analyzer>>,
        cst_analyzers: Vec<Box<dyn CstAnalyzer>>,
        heuristics: Box<dyn HeuristicsProvider>,
        scorer: Box<dyn PostScorer>,
        blend: f64,
    ) -> Self {
        Self {
            analyzers,
            cst_analyzers,
            heuristics,
            scorer: Some(scorer),
            ml_blend: blend.clamp(0.0, 1.0),
        }
    }

    pub fn run(&self, source: &str, file_path: Option<PathBuf>) -> Report {
        let lang = file_path.as_ref().and_then(|p| detect_language(p));

        let mut signals: Vec<Signal> = self
            .analyzers
            .iter()
            .flat_map(|a| a.analyze_with_language(source, lang))
            .collect();

        // CST analysis — extract metrics, match against TOML rules, and
        // accumulate raw metrics for the PostScorer (if configured).
        let mut collected_metrics = HashMap::new();

        if let Some(ref path) = file_path {
            if let Some(cst_lang) = detect_language(path) {
                let ts_lang = crate::language::get_ts_language(cst_lang);
                let mut parser = tree_sitter::Parser::new();
                if parser.set_language(&ts_lang).is_ok() {
                    if let Some(tree) = parser.parse(source.as_bytes(), None) {
                        let cst_heur_lang = HeuristicLanguage::cst_from(cst_lang);
                        for cst_analyzer in &self.cst_analyzers {
                            if cst_analyzer.target_language() == cst_lang {
                                let metrics = cst_analyzer.extract_metrics(&tree, source);
                                if metrics.is_empty() {
                                    signals.extend(cst_analyzer.analyze_tree(&tree, source));
                                } else {
                                    collected_metrics.extend(
                                        metrics.iter().map(|(k, &v)| (k.clone(), v)),
                                    );
                                    signals.extend(match_metric_signals(
                                        &metrics,
                                        cst_heur_lang,
                                        &*self.heuristics,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        for s in &mut signals {
            if !s.id.is_empty() {
                s.weight = self.heuristics.weight(&s.id);
            }
        }
        signals.retain(|s| s.id.is_empty() || self.heuristics.is_enabled(&s.id));

        let attribution = if let Some(ref scorer) = self.scorer {
            let heuristic_attr = self.aggregate(&signals);
            let ml_attr = scorer.rescore(
                &signals,
                &collected_metrics,
                &heuristic_attr,
                lang,
                source,
            );
            blend_attributions(&heuristic_attr, &ml_attr, self.ml_blend)
        } else {
            self.aggregate(&signals)
        };

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
            // No signal data — leave all scores at 0.0, confidence 0.0
            return Attribution {
                primary: ModelFamily::Human,
                confidence: 0.0,
                scores: shifted,
            };
        }

        let (primary, confidence) = shifted
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap().then_with(|| a.0.to_string().cmp(&b.0.to_string())))
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

    #[test]
    fn aggregate_empty_signals_returns_zero_confidence() {
        let pipeline = Pipeline::with_defaults();
        let attr = pipeline.aggregate(&[]);
        assert_eq!(attr.confidence, 0.0);
        assert!(!attr.has_sufficient_data());
        let total: f64 = attr.scores.values().sum();
        assert_eq!(total, 0.0, "scores should all be 0.0 when no signals");
    }

    // -- PostScorer / blend tests ------------------------------------------

    struct FixedScorer(Attribution);

    impl PostScorer for FixedScorer {
        fn rescore(
            &self,
            _signals: &[Signal],
            _metrics: &HashMap<String, f64>,
            _heuristic: &Attribution,
            _language: Option<Language>,
            _source: &str,
        ) -> Attribution {
            self.0.clone()
        }
    }

    fn make_attribution(primary: ModelFamily, confidence: f64) -> Attribution {
        let mut scores = HashMap::new();
        for f in ModelFamily::all() {
            scores.insert(*f, if *f == primary { confidence } else { (1.0 - confidence) / 4.0 });
        }
        Attribution { primary, confidence, scores }
    }

    #[test]
    fn blend_zero_returns_heuristic() {
        let h = make_attribution(ModelFamily::Claude, 0.8);
        let m = make_attribution(ModelFamily::Gpt, 0.9);
        let blended = blend_attributions(&h, &m, 0.0);
        assert_eq!(blended.primary, ModelFamily::Claude);
        assert!((blended.confidence - 0.8).abs() < 1e-9);
    }

    #[test]
    fn blend_one_returns_ml() {
        let h = make_attribution(ModelFamily::Claude, 0.8);
        let m = make_attribution(ModelFamily::Gpt, 0.9);
        let blended = blend_attributions(&h, &m, 1.0);
        assert_eq!(blended.primary, ModelFamily::Gpt);
        assert!((blended.confidence - 0.9).abs() < 1e-9);
    }

    #[test]
    fn blend_half_averages_scores() {
        let h = make_attribution(ModelFamily::Human, 1.0);
        let m = make_attribution(ModelFamily::Gemini, 1.0);
        let blended = blend_attributions(&h, &m, 0.5);
        let h_score = blended.scores[&ModelFamily::Human];
        let m_score = blended.scores[&ModelFamily::Gemini];
        assert!((h_score - m_score).abs() < 1e-9, "equal blend should produce equal scores");
    }

    #[test]
    fn blend_preserves_normalization() {
        let h = make_attribution(ModelFamily::Claude, 0.6);
        let m = make_attribution(ModelFamily::Copilot, 0.7);
        let blended = blend_attributions(&h, &m, 0.3);
        let total: f64 = blended.scores.values().sum();
        assert!((total - 1.0).abs() < 1e-9, "blended scores should sum to ~1.0; got {total}");
    }

    #[test]
    fn with_model_scorer_is_called() {
        let ml_attr = make_attribution(ModelFamily::Gemini, 0.95);
        let scorer = FixedScorer(ml_attr);
        let pipeline = Pipeline::with_model(
            default_analyzers(),
            default_cst_analyzers(),
            Box::new(DefaultHeuristics),
            Box::new(scorer),
            1.0,
        );
        let report = pipeline.run("let x = 42;", None);
        assert_eq!(report.attribution.primary, ModelFamily::Gemini);
    }

    #[test]
    fn without_scorer_behavior_unchanged() {
        let pipeline_default = Pipeline::with_defaults();
        let pipeline_explicit = Pipeline::with_heuristics(
            default_analyzers(),
            default_cst_analyzers(),
            Box::new(DefaultHeuristics),
        );
        let source = "fn main() { println!(\"hello\"); }\n";
        let r1 = pipeline_default.run(source, None);
        let r2 = pipeline_explicit.run(source, None);
        assert_eq!(r1.attribution.primary, r2.attribution.primary);
        assert!((r1.attribution.confidence - r2.attribution.confidence).abs() < 1e-9);
    }

    #[test]
    fn with_model_blend_clamped() {
        let ml_attr = make_attribution(ModelFamily::Gpt, 0.9);
        let pipeline = Pipeline::with_model(
            default_analyzers(),
            default_cst_analyzers(),
            Box::new(DefaultHeuristics),
            Box::new(FixedScorer(ml_attr)),
            5.0, // should clamp to 1.0
        );
        let report = pipeline.run("let x = 42;", None);
        assert_eq!(report.attribution.primary, ModelFamily::Gpt);
    }
}
