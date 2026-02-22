use crate::analyzers::Analyzer;
use crate::report::Signal;

pub struct CommentStyleAnalyzer;

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
