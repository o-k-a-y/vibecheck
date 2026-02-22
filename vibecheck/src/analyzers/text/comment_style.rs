use crate::analyzers::Analyzer;
use crate::report::Signal;

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
            signals.iter().any(|s| s.family == ModelFamily::Claude && s.weight == 2.0),
            "expected teaching voice Claude signal (weight 2.0)"
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
}

impl Analyzer for CommentStyleAnalyzer {
    fn name(&self) -> &str {
        "comments"
    }

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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("High comment density ({:.0}%)", density * 100.0),
                family: crate::report::ModelFamily::Claude,
                weight: 1.5,
            });
        } else if density < 0.03 && total_lines > 20 {
            signals.push(Signal {
                source: self.name().into(),
                description: "Very low comment density".into(),
                family: crate::report::ModelFamily::Human,
                weight: 1.0,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{teaching_count} comments with teaching/explanatory voice"),
                family: crate::report::ModelFamily::Claude,
                weight: 2.0,
            });
        } else if teaching_count >= 1 {
            signals.push(Signal {
                source: self.name().into(),
                description: "Some explanatory comments present".into(),
                family: crate::report::ModelFamily::Gpt,
                weight: 0.8,
            });
        }

        // Doc comments (///) — AI loves these
        let doc_comment_count = lines
            .iter()
            .filter(|l| l.trim_start().starts_with("///"))
            .count();
        if doc_comment_count >= 5 {
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{doc_comment_count} doc comments — thorough documentation"),
                family: crate::report::ModelFamily::Claude,
                weight: 1.5,
            });
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
            signals.push(Signal {
                source: self.name().into(),
                description: format!("{terse_count} terse/frustrated comments (TODO, HACK, etc.)"),
                family: crate::report::ModelFamily::Human,
                weight: 2.0,
            });
        }

        signals
    }
}
