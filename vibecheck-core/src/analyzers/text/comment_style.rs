use crate::analyzers::Analyzer;
use crate::heuristics::signal_ids;
use crate::report::{ModelFamily, Signal};

pub struct CommentStyleAnalyzer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzers::Analyzer;
    use crate::report::ModelFamily;

    fn run(source: &str) -> Vec<Signal> {
        CommentStyleAnalyzer.analyze(source)
    }

    #[test]
    fn empty_source_no_signals() {
        assert!(run("").is_empty());
    }

    #[test]
    fn high_comment_density_is_claude() {
        // 5 comment lines out of 25 total = 20% > 15%
        let lines: Vec<&str> = std::iter::repeat("// a comment")
            .take(5)
            .chain(std::iter::repeat("let x = 1;").take(20))
            .collect();
        let source = lines.join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected high density Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn low_comment_density_is_human() {
        // 25 lines, 0 comments → 0% < 3% and > 20 lines
        let source = std::iter::repeat("let x = 1;").take(25).collect::<Vec<_>>().join("\n");
        let signals = run(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 1.0),
            "expected low density Human signal (weight 1.0)"
        );
    }

    #[test]
    fn teaching_voice_3_plus_is_claude() {
        let source = "// note that this is correct\n// this ensures safety\n// this allows reuse\nlet x = 1;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected teaching voice Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn teaching_voice_1_is_gpt() {
        let source = "// note that this works\nlet x = 1;\nlet y = 2;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Gpt && s.weight == 0.8),
            "expected single teaching phrase Gpt signal (weight 0.8)"
        );
    }

    #[test]
    fn five_doc_comments_is_claude() {
        let source = "/// doc one\n/// doc two\n/// doc three\n/// doc four\n/// doc five\nlet x = 1;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 1.5),
            "expected doc comments Claude signal (weight 1.5)"
        );
    }

    #[test]
    fn terse_markers_is_human() {
        let source = "// TODO: fix this\n// HACK: workaround needed\nlet x = 1;";
        let signals = run(source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Human && s.weight == 2.0),
            "expected terse markers Human signal (weight 2.0)"
        );
    }

    #[test]
    fn python_high_comment_density_is_claude() {
        // Many # comments relative to code lines
        let mut lines: Vec<String> = (0..10).map(|_| "# This explains the approach".into()).collect();
        lines.extend((0..5).map(|i| format!("x{i} = {i}")));
        let source = lines.join("\n");
        let signals = CommentStyleAnalyzer.analyze_python(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for high Python comment density"
        );
    }

    #[test]
    fn javascript_high_comment_density_is_claude() {
        let mut lines: Vec<String> = (0..10).map(|_| "// This explains the approach".into()).collect();
        lines.extend((0..5).map(|i| format!("const x{i} = {i};")));
        let source = lines.join("\n");
        let signals = CommentStyleAnalyzer.analyze_javascript(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for high JS comment density"
        );
    }

    #[test]
    fn go_high_comment_density_is_claude() {
        let mut lines: Vec<String> = (0..10).map(|_| "// This explains the approach".into()).collect();
        lines.extend((0..5).map(|i| format!("var x{i} int = {i}")));
        let source = lines.join("\n");
        let signals = CommentStyleAnalyzer.analyze_go(&source);
        assert!(
            signals.iter().any(|s| s.family == ModelFamily::Claude),
            "expected Claude signal for high Go comment density"
        );
    }
}

impl CommentStyleAnalyzer {
    /// New comment signals shared across languages.
    ///
    /// Takes precomputed comment lines (as lowercase strings) and raw lines.
    #[allow(clippy::too_many_arguments)]
    fn detect_extra_signals(
        name: &str,
        comment_lines_lower: &[String],
        total_lines: usize,
        comment_count: usize,
        step_id: &str,
        heres_id: &str,
        bullet_id: &str,
        minimal_id: &str,
        external_id: &str,
        verbose_id: &str,
    ) -> Vec<Signal> {
        let mut signals = Vec::new();
        let density = if total_lines > 0 { comment_count as f64 / total_lines as f64 } else { 0.0 };

        // GPT: step-numbered comments ("step 1", "step 2", etc.)
        let step_count = comment_lines_lower
            .iter()
            .filter(|l| {
                // Match "step N" or "Step N:" patterns
                l.contains("step 1") || l.contains("step 2") || l.contains("step 3")
                    || l.contains("step 4") || l.contains("step 5")
                    || l.contains("1.") || l.contains("2.") || l.contains("3.")
            })
            .count();
        if step_count >= 3 {
            signals.push(Signal::new(
                step_id, name,
                format!("{step_count} step-numbered comments"),
                ModelFamily::Gpt, 1.5,
            ));
        }

        // GPT: "here's" / "let's" phrases
        let heres_count = comment_lines_lower
            .iter()
            .filter(|l| {
                l.contains("here's") || l.contains("let's") || l.contains("here is") || l.contains("let us")
            })
            .count();
        if heres_count >= 3 {
            signals.push(Signal::new(
                heres_id, name,
                format!("{heres_count} here's/let's phrases in comments"),
                ModelFamily::Gpt, 1.0,
            ));
        }

        // Gemini: bullet-point comments ("// - item" or "# - item")
        let bullet_count = comment_lines_lower
            .iter()
            .filter(|l| {
                let t = l.trim();
                // After stripping the comment prefix, check for "- "
                let after = if t.starts_with("//") {
                    t.trim_start_matches("//").trim()
                } else if t.starts_with('#') {
                    t.trim_start_matches('#').trim()
                } else {
                    t
                };
                after.starts_with("- ")
            })
            .count();
        if bullet_count >= 3 {
            signals.push(Signal::new(
                bullet_id, name,
                format!("{bullet_count} bullet-point comments"),
                ModelFamily::Gemini, 1.0,
            ));
        }

        // Copilot: minimal comments (<1% density in file >30 lines)
        if density < 0.01 && total_lines > 30 {
            signals.push(Signal::new(
                minimal_id, name,
                "Extremely low comment density (<1%)",
                ModelFamily::Copilot, 1.5,
            ));
        }

        // Human: external references (JIRA-123, #456, @person)
        let ext_ref_count = comment_lines_lower
            .iter()
            .filter(|l| {
                // Ticket refs: ABC-123, #123, @username
                let has_ticket = l.char_indices().any(|(i, c)| {
                    if c == '-' && i > 1 {
                        let before = &l[..i];
                        let after = &l[i + c.len_utf8()..];
                        before.chars().rev().take(2).all(|c| c.is_ascii_uppercase())
                            && after.chars().take(1).all(|c| c.is_ascii_digit())
                    } else {
                        false
                    }
                });
                let has_issue = l.contains('#') && l.chars().skip_while(|&c| c != '#').skip(1).take(2).all(|c| c.is_ascii_digit());
                let has_mention = l.contains('@') && l.chars().skip_while(|&c| c != '@').skip(1).take(1).all(|c| c.is_alphabetic());
                has_ticket || has_issue || has_mention
            })
            .count();
        if ext_ref_count >= 2 {
            signals.push(Signal::new(
                external_id, name,
                format!("{ext_ref_count} ticket/issue references in comments"),
                ModelFamily::Human, 2.0,
            ));
        }

        // GPT: verbose obvious (comment-to-code ratio >0.8 in simple code)
        let code_lines = total_lines.saturating_sub(comment_count);
        if code_lines > 0 && total_lines > 20 {
            let ratio = comment_count as f64 / code_lines as f64;
            if ratio > 0.8 {
                signals.push(Signal::new(
                    verbose_id, name,
                    format!("High comment-to-code ratio ({:.1})", ratio),
                    ModelFamily::Gpt, 1.2,
                ));
            }
        }

        signals
    }

    /// Comment signals that apply regardless of language (slash-comment languages).
    #[allow(clippy::too_many_arguments)]
    fn analyze_slash_comments(
        name: &str,
        high_density_id: &str,
        low_density_id: &str,
        teaching_id: &str,
        explanatory_id: &str,
        terse_id: &str,
        step_id: &str,
        heres_id: &str,
        bullet_id: &str,
        minimal_id: &str,
        external_id: &str,
        verbose_id: &str,
        source: &str,
    ) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines == 0 {
            return signals;
        }

        let comment_lines: Vec<&&str> = lines
            .iter()
            .filter(|l| l.trim_start().starts_with("//"))
            .collect();
        let comment_count = comment_lines.len();
        let density = comment_count as f64 / total_lines as f64;

        if density > 0.15 {
            signals.push(Signal::new(
                high_density_id,
                name,
                format!("High comment density ({:.0}%)", density * 100.0),
                ModelFamily::Claude,
                1.5,
            ));
        } else if density < 0.03 && total_lines > 20 {
            signals.push(Signal::new(
                low_density_id,
                name,
                "Very low comment density",
                ModelFamily::Human,
                1.0,
            ));
        }

        // Teaching voice
        let teaching_phrases = [
            "note that", "this ensures", "this allows", "we need to",
            "important:", "this is necessary", "here we", "this handles",
            "this converts", "this creates", "this returns",
        ];
        let mut teaching_count = 0;
        for line in &comment_lines {
            let lower = line.to_lowercase();
            if teaching_phrases.iter().any(|p| lower.contains(p)) {
                teaching_count += 1;
            }
        }
        if teaching_count >= 3 {
            signals.push(Signal::new(
                teaching_id,
                name,
                format!("{teaching_count} comments with teaching/explanatory voice"),
                ModelFamily::Claude,
                1.5,
            ));
        } else if teaching_count >= 1 {
            signals.push(Signal::new(
                explanatory_id,
                name,
                "Some explanatory comments present",
                ModelFamily::Gpt,
                0.8,
            ));
        }

        // Terse frustration markers
        let terse_markers = ["todo", "hack", "fixme", "xxx", "wtf", "ugh"];
        let terse_count = comment_lines
            .iter()
            .filter(|l| {
                let lower = l.to_lowercase();
                terse_markers.iter().any(|m| lower.contains(m))
            })
            .count();
        if terse_count >= 2 {
            signals.push(Signal::new(
                terse_id,
                name,
                format!("{terse_count} terse/frustrated comments (TODO, HACK, etc.)"),
                ModelFamily::Human,
                2.0,
            ));
        }

        let comment_lower: Vec<String> = comment_lines.iter().map(|l| l.to_lowercase()).collect();
        signals.extend(Self::detect_extra_signals(
            name, &comment_lower, total_lines, comment_count,
            step_id, heres_id, bullet_id, minimal_id, external_id, verbose_id,
        ));

        signals
    }

    fn analyze_python_impl(source: &str) -> Vec<Signal> {
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines == 0 {
            return vec![];
        }

        // Python uses # for line comments
        let comment_lines: Vec<&&str> = lines
            .iter()
            .filter(|l| l.trim_start().starts_with('#'))
            .collect();
        let comment_count = comment_lines.len();
        let density = comment_count as f64 / total_lines as f64;
        let mut signals = Vec::new();

        if density > 0.15 {
            signals.push(Signal::new(
                signal_ids::PYTHON_COMMENTS_HIGH_DENSITY,
                "comments",
                format!("High comment density ({:.0}%)", density * 100.0),
                ModelFamily::Claude,
                1.5,
            ));
        } else if density < 0.03 && total_lines > 20 {
            signals.push(Signal::new(
                signal_ids::PYTHON_COMMENTS_LOW_DENSITY,
                "comments",
                "Very low comment density",
                ModelFamily::Human,
                1.0,
            ));
        }

        // Teaching voice in # comments
        let teaching_phrases = [
            "note that", "this ensures", "this allows", "we need to",
            "important:", "this is necessary", "here we", "this handles",
        ];
        let teaching_count = comment_lines
            .iter()
            .filter(|l| {
                let lower = l.to_lowercase();
                teaching_phrases.iter().any(|p| lower.contains(p))
            })
            .count();
        if teaching_count >= 3 {
            signals.push(Signal::new(
                signal_ids::PYTHON_COMMENTS_TEACHING_VOICE,
                "comments",
                format!("{teaching_count} comments with teaching/explanatory voice"),
                ModelFamily::Claude,
                1.5,
            ));
        } else if teaching_count >= 1 {
            signals.push(Signal::new(
                signal_ids::PYTHON_COMMENTS_SOME_EXPLANATORY,
                "comments",
                "Some explanatory comments present",
                ModelFamily::Gpt,
                0.8,
            ));
        }

        // Docstrings — triple-quoted strings (""" or ''') as first statement
        let docstring_count = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("\"\"\"") || t.starts_with("'''")
            })
            .count();
        if docstring_count >= 5 {
            signals.push(Signal::new(
                signal_ids::PYTHON_COMMENTS_DOCSTRING_BLOCKS,
                "comments",
                format!("{docstring_count} docstring blocks — thorough documentation"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        // Terse markers
        let terse_markers = ["todo", "hack", "fixme", "xxx"];
        let terse_count = comment_lines
            .iter()
            .filter(|l| {
                let lower = l.to_lowercase();
                terse_markers.iter().any(|m| lower.contains(m))
            })
            .count();
        if terse_count >= 2 {
            signals.push(Signal::new(
                signal_ids::PYTHON_COMMENTS_TERSE_MARKERS,
                "comments",
                format!("{terse_count} terse/frustrated comments"),
                ModelFamily::Human,
                2.0,
            ));
        }

        let comment_lower: Vec<String> = comment_lines.iter().map(|l| l.to_lowercase()).collect();
        signals.extend(Self::detect_extra_signals(
            "comments", &comment_lower, total_lines, comment_count,
            signal_ids::PYTHON_COMMENTS_STEP_NUMBERED,
            signal_ids::PYTHON_COMMENTS_HERES_LETS,
            signal_ids::PYTHON_COMMENTS_BULLET_STYLE,
            signal_ids::PYTHON_COMMENTS_MINIMAL,
            signal_ids::PYTHON_COMMENTS_EXTERNAL_REFS,
            signal_ids::PYTHON_COMMENTS_VERBOSE_OBVIOUS,
        ));

        signals
    }

    fn analyze_javascript_impl(source: &str) -> Vec<Signal> {
        let mut signals = Self::analyze_slash_comments(
            "comments",
            signal_ids::JS_COMMENTS_HIGH_DENSITY,
            signal_ids::JS_COMMENTS_LOW_DENSITY,
            signal_ids::JS_COMMENTS_TEACHING_VOICE,
            signal_ids::JS_COMMENTS_SOME_EXPLANATORY,
            signal_ids::JS_COMMENTS_TERSE_MARKERS,
            signal_ids::JS_COMMENTS_STEP_NUMBERED,
            signal_ids::JS_COMMENTS_HERES_LETS,
            signal_ids::JS_COMMENTS_BULLET_STYLE,
            signal_ids::JS_COMMENTS_MINIMAL,
            signal_ids::JS_COMMENTS_EXTERNAL_REFS,
            signal_ids::JS_COMMENTS_VERBOSE_OBVIOUS,
            source,
        );
        let lines: Vec<&str> = source.lines().collect();

        // JSDoc blocks (/** ... */)
        let jsdoc_count = lines.iter().filter(|l| l.trim().starts_with("/**")).count();
        if jsdoc_count >= 5 {
            signals.push(Signal::new(
                signal_ids::JS_COMMENTS_JSDOC_BLOCKS,
                "comments",
                format!("{jsdoc_count} JSDoc comment blocks — thorough API documentation"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        signals
    }

    fn analyze_go_impl(source: &str) -> Vec<Signal> {
        // Go uses // for all comments, same as Rust — reuse slash comment logic
        Self::analyze_slash_comments(
            "comments",
            signal_ids::GO_COMMENTS_HIGH_DENSITY,
            signal_ids::GO_COMMENTS_LOW_DENSITY,
            signal_ids::GO_COMMENTS_TEACHING_VOICE,
            signal_ids::GO_COMMENTS_SOME_EXPLANATORY,
            signal_ids::GO_COMMENTS_TERSE_MARKERS,
            signal_ids::GO_COMMENTS_STEP_NUMBERED,
            signal_ids::GO_COMMENTS_HERES_LETS,
            signal_ids::GO_COMMENTS_BULLET_STYLE,
            signal_ids::GO_COMMENTS_MINIMAL,
            signal_ids::GO_COMMENTS_EXTERNAL_REFS,
            signal_ids::GO_COMMENTS_VERBOSE_OBVIOUS,
            source,
        )
    }
}

impl Analyzer for CommentStyleAnalyzer {
    fn name(&self) -> &str {
        "comments"
    }

    fn analyze_python(&self, source: &str) -> Vec<Signal> { Self::analyze_python_impl(source) }
    fn analyze_javascript(&self, source: &str) -> Vec<Signal> { Self::analyze_javascript_impl(source) }
    fn analyze_go(&self, source: &str) -> Vec<Signal> { Self::analyze_go_impl(source) }

    fn analyze(&self, source: &str) -> Vec<Signal> {
        let mut signals = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        if total_lines == 0 {
            return signals;
        }

        let comment_lines: Vec<&&str> = lines.iter().filter(|l| l.trim_start().starts_with("//")).collect();
        let comment_count = comment_lines.len();
        let density = comment_count as f64 / total_lines as f64;

        // High comment density is an AI signal
        if density > 0.15 {
            signals.push(Signal::new(
                signal_ids::RUST_COMMENTS_HIGH_DENSITY,
                self.name(),
                format!("High comment density ({:.0}%)", density * 100.0),
                ModelFamily::Claude,
                1.5,
            ));
        } else if density < 0.03 && total_lines > 20 {
            signals.push(Signal::new(
                signal_ids::RUST_COMMENTS_LOW_DENSITY,
                self.name(),
                "Very low comment density",
                ModelFamily::Human,
                1.0,
            ));
        }

        // Teaching voice: comments that explain "why" or use pedagogical language
        let teaching_phrases = [
            "note that", "this ensures", "this allows", "we need to",
            "important:", "this is necessary", "here we", "this handles",
            "this converts", "this creates", "this returns",
        ];
        let mut teaching_count = 0;
        for line in &comment_lines {
            let lower = line.to_lowercase();
            if teaching_phrases.iter().any(|p| lower.contains(p)) {
                teaching_count += 1;
            }
        }

        if teaching_count >= 3 {
            signals.push(Signal::new(
                signal_ids::RUST_COMMENTS_TEACHING_VOICE,
                self.name(),
                format!("{teaching_count} comments with teaching/explanatory voice"),
                ModelFamily::Claude,
                1.5,
            ));
        } else if teaching_count >= 1 {
            signals.push(Signal::new(
                signal_ids::RUST_COMMENTS_SOME_EXPLANATORY,
                self.name(),
                "Some explanatory comments present",
                ModelFamily::Gpt,
                0.8,
            ));
        }

        // Doc comments (///) — AI loves these
        let doc_comment_count = lines
            .iter()
            .filter(|l| l.trim_start().starts_with("///"))
            .count();
        if doc_comment_count >= 5 {
            signals.push(Signal::new(
                signal_ids::RUST_COMMENTS_DOC_COMMENTS,
                self.name(),
                format!("{doc_comment_count} doc comments — thorough documentation"),
                ModelFamily::Claude,
                1.5,
            ));
        }

        // Inline comments that are terse ("// TODO", "// hack", "// fix")
        let terse_markers = ["todo", "hack", "fixme", "xxx", "wtf", "ugh"];
        let terse_count = comment_lines
            .iter()
            .filter(|l| {
                let lower = l.to_lowercase();
                terse_markers.iter().any(|m| lower.contains(m))
            })
            .count();
        if terse_count >= 2 {
            signals.push(Signal::new(
                signal_ids::RUST_COMMENTS_TERSE_MARKERS,
                self.name(),
                format!("{terse_count} terse/frustrated comments (TODO, HACK, etc.)"),
                ModelFamily::Human,
                2.0,
            ));
        }

        let comment_lower: Vec<String> = comment_lines.iter().map(|l| l.to_lowercase()).collect();
        signals.extend(Self::detect_extra_signals(
            self.name(), &comment_lower, total_lines, comment_count,
            signal_ids::RUST_COMMENTS_STEP_NUMBERED,
            signal_ids::RUST_COMMENTS_HERES_LETS,
            signal_ids::RUST_COMMENTS_BULLET_STYLE,
            signal_ids::RUST_COMMENTS_MINIMAL,
            signal_ids::RUST_COMMENTS_EXTERNAL_REFS,
            signal_ids::RUST_COMMENTS_VERBOSE_OBVIOUS,
        ));

        signals
    }
}
