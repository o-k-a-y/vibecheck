use vibecheck::report::{ModelFamily, Report};

const FONT: &str = "ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace";
const BG: &str = "#161b22";
const FG: &str = "#e6edf3";
const BOLD_FG: &str = "#ffffff";
const POS_C: &str = "#7ee787";
const NEG_C: &str = "#f85149";
const FS: u32 = 13;
const LH: f64 = 19.0;
const CW: f64 = 7.8;
const PAD_X: f64 = 16.0;
const PAD_TOP: f64 = 44.0;
const PAD_BOT: f64 = 16.0;
const BAR_H: f64 = 12.0;


fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

enum Elem {
    Span { text: String, bold: bool, dim: bool, color: &'static str },
    Bar  { n: usize, color: &'static str },
}

fn sp(s: impl Into<String>) -> Elem {
    Elem::Span { text: s.into(), bold: false, dim: false, color: FG }
}
fn bo(s: impl Into<String>) -> Elem {
    Elem::Span { text: s.into(), bold: true, dim: false, color: BOLD_FG }
}
fn di(s: impl Into<String>) -> Elem {
    Elem::Span { text: s.into(), bold: false, dim: true, color: FG }
}
fn co(s: impl Into<String>, color: &'static str) -> Elem {
    Elem::Span { text: s.into(), bold: false, dim: false, color }
}
fn boco(s: impl Into<String>, color: &'static str) -> Elem {
    Elem::Span { text: s.into(), bold: true, dim: false, color }
}
fn wt(w: f64) -> Elem {
    let s = if w >= 0.0 { format!("+{w:.1}") } else { format!("{w:.1}") };
    co(s, if w >= 0.0 { POS_C } else { NEG_C })
}
fn bar(pct: f64, color: &'static str) -> Elem {
    let n = (pct / 100.0 * 30.0) as usize;
    Elem::Bar { n, color }
}

fn render_line(elems: &[Elem], x0: f64, y: f64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut x = x0;
    let mut tspans: Vec<String> = Vec::new();
    let mut tx0 = x;

    macro_rules! flush {
        () => {
            if !tspans.is_empty() {
                let inner = tspans.join("");
                out.push(format!(
                    "<text x=\"{tx0:.1}\" y=\"{}\" font-family=\"{FONT}\" \
                     font-size=\"{FS}px\">{inner}</text>",
                    y as i32
                ));
                tspans.clear();
            }
        };
    }

    for elem in elems {
        match elem {
            Elem::Span { text, .. } if text.is_empty() => {}
            Elem::Span { text, bold, dim, color } => {
                if tspans.is_empty() {
                    tx0 = x;
                }
                let mut a = format!("fill=\"{color}\"");
                if *dim  { a.push_str(" opacity=\"0.5\""); }
                if *bold { a.push_str(" font-weight=\"bold\""); }
                let chars = text.chars().count();
                tspans.push(format!("<tspan {a}>{}</tspan>", xml_esc(text)));
                x += chars as f64 * CW;
            }
            Elem::Bar { n, color } => {
                flush!();
                let bw = *n as f64 * CW;
                let by = y - BAR_H;
                out.push(format!(
                    "<rect x=\"{x:.1}\" y=\"{by:.1}\" width=\"{bw:.1}\" \
                     height=\"{}\" fill=\"{color}\" rx=\"1\"/>",
                    BAR_H as i32
                ));
                x += bw;
            }
        }
    }
    flush!();
    out
}

fn line_w(elems: &[Elem]) -> f64 {
    elems.iter().map(|e| match e {
        Elem::Span { text, .. } => text.chars().count() as f64 * CW,
        Elem::Bar  { n, .. }   => *n as f64 * CW,
    }).sum()
}

fn generate_svg(report: &Report, display_path: &str) -> String {
    let mut lines: Vec<Option<Vec<Elem>>> = Vec::new();

    lines.push(Some(vec![co(format!("$ vibecheck {display_path}"), FG)]));
    lines.push(None);
    lines.push(Some(vec![bo("File:"), sp(format!(" {display_path}"))]));

    let confidence = (report.attribution.confidence * 100.0).round() as i32;
    let verdict = format!("{} ({confidence}% confidence)", report.attribution.primary);
    let vcolor = report.attribution.primary.svg_color();
    lines.push(Some(vec![bo("Verdict:"), sp(" "), boco(verdict, vcolor)]));
    lines.push(Some(vec![
        di("Lines:"), sp(format!(" {} | ", report.metadata.lines_of_code)),
        di("Signals:"), sp(format!(" {}", report.metadata.signal_count)),
    ]));

    lines.push(None);
    lines.push(Some(vec![bo("Scores:")]));

    let mut scores: Vec<(ModelFamily, f64)> = report.attribution.scores.iter()
        .map(|(&f, &s)| (f, s)).collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    for (fam, score) in &scores {
        let label = format!("  {:<10} ", fam.to_string());
        let pct   = score * 100.0;
        let color = fam.svg_color();
        lines.push(Some(vec![sp(label), bar(pct, color), sp(format!(" {pct:.1}%"))]));
    }

    lines.push(None);
    lines.push(Some(vec![bo("Signals:")]));

    for sig in &report.signals {
        lines.push(Some(vec![
            sp("  "),
            di(format!("[{}]", sig.source)),
            sp(" "),
            wt(sig.weight),
            sp(" "),
            bo(sig.family.to_string()),
            sp(format!(" \u{2014} {}", sig.description)),
        ]));
    }

    let max_w = lines.iter()
        .filter_map(|l| l.as_ref())
        .map(|el| line_w(el))
        .fold(0.0f64, f64::max);
    let width  = (PAD_X * 2.0 + max_w + 24.0) as u32;
    let height = (PAD_TOP + lines.len() as f64 * LH + PAD_BOT) as u32;

    let mut svg: Vec<String> = Vec::new();
    svg.push(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" \
         width=\"{width}\" height=\"{height}\">"
    ));
    svg.push(format!("  <rect width=\"{width}\" height=\"{height}\" fill=\"{BG}\" rx=\"8\"/>"));
    svg.push("  <circle cx=\"16\" cy=\"16\" r=\"6\" fill=\"#ff5f57\"/>".to_string());
    svg.push("  <circle cx=\"34\" cy=\"16\" r=\"6\" fill=\"#febc2e\"/>".to_string());
    svg.push("  <circle cx=\"52\" cy=\"16\" r=\"6\" fill=\"#28c840\"/>".to_string());
    svg.push(format!(
        "  <line x1=\"0\" y1=\"30\" x2=\"{width}\" y2=\"30\" \
         stroke=\"#ffffff\" stroke-opacity=\"0.08\" stroke-width=\"1\"/>"
    ));

    for (i, line) in lines.iter().enumerate() {
        if let Some(elems) = line {
            let y = PAD_TOP + i as f64 * LH;
            for s in render_line(elems, PAD_X, y) {
                svg.push(format!("  {s}"));
            }
        }
    }

    svg.push("</svg>".to_string());
    svg.join("\n") + "\n"
}

fn main() {
    println!("cargo:rerun-if-changed=src/output.rs");

    let source = match std::fs::read_to_string("src/output.rs") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("build.rs: skipping SVG generation ({e})");
            return;
        }
    };

    let report = vibecheck::analyze(&source);
    let svg = generate_svg(&report, "./vibecheck-cli/src/output.rs");

    let out = "../.github/assets/example.svg";
    if let Err(e) = std::fs::write(out, &svg) {
        eprintln!("build.rs: failed to write SVG: {e}");
    }
}
