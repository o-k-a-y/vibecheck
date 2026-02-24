use vibecheck_core::report::{ModelFamily, Report};

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

fn generate_svg(report: &Report, display_path: &str) -> String {
    const BAR_N: usize  = 30;         // max bar width in chars
    const BAR_MAX: f64  = BAR_N as f64 * CW;

    let confidence = (report.attribution.confidence * 100.0).round() as i32;
    let verdict    = format!("{} ({confidence}% confidence)", report.attribution.primary);
    let vcolor     = report.attribution.primary.svg_color();

    // ── Compute canvas width from widest content line ────────────────────
    let label_w = 12usize; // "  {:<10} " = 12 chars
    let score_w = label_w + BAR_N + 1 + 6; // label + bar + gap + "nn.n%"
    let max_chars = [
        format!("$ vibecheck {display_path}").chars().count(),
        format!("File: {display_path}").chars().count(),
        format!("Verdict: {verdict}").chars().count(),
        format!("Lines: {} | Signals: {}",
            report.metadata.lines_of_code, report.metadata.signal_count).chars().count(),
        score_w,
        report.signals.iter().map(|s| {
            let sign = if s.weight >= 0.0 { "+" } else { "" };
            format!("  [{}] {}{:.1} {} \u{2014} {}",
                s.source, sign, s.weight, s.family, s.description).chars().count()
        }).max().unwrap_or(0),
    ].iter().copied().max().unwrap_or(60);

    // rows: cmd blank File Verdict Lines blank Scores: 5×score blank Signals: N×signal
    let n_rows = 6 + 1 + 5 + 1 + 1 + report.signals.len();
    let width  = (PAD_X * 2.0 + max_chars as f64 * CW + 24.0) as u32;
    let height = (PAD_TOP + n_rows as f64 * LH + PAD_BOT) as u32;

    let mut svg: Vec<String> = Vec::new();
    macro_rules! p { ($fmt:expr) => { svg.push($fmt.to_string()) };
                     ($($arg:tt)*) => { svg.push(format!($($arg)*)) } }

    // ── SVG scaffold ──────────────────────────────────────────────────────
    p!(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\">"));
    p!(format!("  <rect width=\"{width}\" height=\"{height}\" fill=\"{BG}\" rx=\"8\"/>"));
    p!("  <circle cx=\"16\" cy=\"16\" r=\"6\" fill=\"#ff5f57\"/>".to_string());
    p!("  <circle cx=\"34\" cy=\"16\" r=\"6\" fill=\"#febc2e\"/>".to_string());
    p!("  <circle cx=\"52\" cy=\"16\" r=\"6\" fill=\"#28c840\"/>".to_string());
    p!(format!("  <line x1=\"0\" y1=\"30\" x2=\"{width}\" y2=\"30\" stroke=\"{BOLD_FG}\" stroke-opacity=\"0.08\" stroke-width=\"1\"/>"));

    // Helper: y for a given row index
    let row_y = |r: usize| -> i32 { (PAD_TOP + r as f64 * LH) as i32 };

    // Helper closures for common element types
    let text = |svg: &mut Vec<String>, x: f64, y: i32, fill: &str, s: &str| {
        svg.push(format!("  <text x=\"{x:.1}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{fill}\">{}</text>",
            xml_esc(s)));
    };
    let text_bold = |svg: &mut Vec<String>, x: f64, y: i32, fill: &str, s: &str| {
        svg.push(format!("  <text x=\"{x:.1}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{fill}\" font-weight=\"bold\">{}</text>",
            xml_esc(s)));
    };
    let text_dim = |svg: &mut Vec<String>, x: f64, y: i32, s: &str| {
        svg.push(format!("  <text x=\"{x:.1}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{FG}\" opacity=\"0.5\">{}</text>",
            xml_esc(s)));
    };
    let bar_rect = |svg: &mut Vec<String>, x: f64, y: i32, w: f64, fill: &str| {
        let by = y as f64 - BAR_H;
        svg.push(format!("  <rect x=\"{x:.1}\" y=\"{by:.1}\" width=\"{w:.1}\" height=\"{BAR_H}\" fill=\"{fill}\" rx=\"1\"/>"));
    };

    let mut row = 0usize;

    // ── Content rows ──────────────────────────────────────────────────────

    // $ vibecheck {path}
    text(&mut svg, PAD_X, row_y(row), FG, &format!("$ vibecheck {display_path}"));
    row += 2; // skip blank row

    // File: {path}
    let mut x = PAD_X;
    text_bold(&mut svg, x, row_y(row), BOLD_FG, "File:");
    x += "File:".chars().count() as f64 * CW;
    text(&mut svg, x, row_y(row), FG, &format!(" {display_path}"));
    row += 1;

    // Verdict: {family (pct% confidence)}
    x = PAD_X;
    text_bold(&mut svg, x, row_y(row), BOLD_FG, "Verdict:");
    x += "Verdict: ".chars().count() as f64 * CW;
    text_bold(&mut svg, x, row_y(row), &vcolor, &verdict);
    row += 1;

    // Lines: N | Signals: N  (dim labels)
    x = PAD_X;
    text_dim(&mut svg, x, row_y(row), "Lines:");
    x += "Lines:".chars().count() as f64 * CW;
    let loc_str = format!(" {} | ", report.metadata.lines_of_code);
    text(&mut svg, x, row_y(row), FG, &loc_str);
    x += loc_str.chars().count() as f64 * CW;
    text_dim(&mut svg, x, row_y(row), "Signals:");
    x += "Signals:".chars().count() as f64 * CW;
    text(&mut svg, x, row_y(row), FG, &format!(" {}", report.metadata.signal_count));
    row += 2; // skip blank row

    // Scores:
    text_bold(&mut svg, PAD_X, row_y(row), BOLD_FG, "Scores:");
    row += 1;

    // Score bars — solid rect, pct at fixed column
    let mut sorted_scores: Vec<_> = report.attribution.scores.iter().collect();
    sorted_scores.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (fam, &score) in &sorted_scores {
        let label = format!("  {:<10} ", fam.to_string());
        let color = fam.svg_color();
        let bx    = PAD_X + label.chars().count() as f64 * CW;
        let bar_w = score * BAR_MAX;
        let pct   = format!("{:.1}%", score * 100.0);

        text(&mut svg, PAD_X, row_y(row), FG, &label);
        if bar_w > 0.5 { bar_rect(&mut svg, bx, row_y(row), bar_w, &color); }
        let px = bx + BAR_MAX + CW;   // fixed column regardless of bar length
        text(&mut svg, px, row_y(row), FG, &pct);
        row += 1;
    }
    row += 1; // blank row

    // Signals:
    text_bold(&mut svg, PAD_X, row_y(row), BOLD_FG, "Signals:");
    row += 1;

    // Signal rows: "  [source] +weight FamilyName — description"
    for sig in &report.signals {
        let sign   = if sig.weight >= 0.0 { "+" } else { "" };
        let wt_col = if sig.weight >= 0.0 { POS_C } else { NEG_C };
        let fc     = sig.family.svg_color();

        x = PAD_X;
        text(&mut svg, x, row_y(row), FG, "  ");
        x += 2.0 * CW;

        let src = format!("[{}] ", sig.source);
        text_dim(&mut svg, x, row_y(row), &src);
        x += src.chars().count() as f64 * CW;

        let wt_str = format!("{sign}{:.1} ", sig.weight);
        text(&mut svg, x, row_y(row), wt_col, &wt_str);
        x += wt_str.chars().count() as f64 * CW;

        let fam_str = format!("{} ", sig.family.to_string());
        text_bold(&mut svg, x, row_y(row), &fc, &fam_str);
        x += fam_str.chars().count() as f64 * CW;

        let desc = format!("\u{2014} {}", sig.description);
        text(&mut svg, x, row_y(row), FG, &desc);
        row += 1;
    }

    svg.push("</svg>".to_string());
    svg.join("\n") + "\n"
}

// ---------------------------------------------------------------------------
// TUI screenshot SVG
// ---------------------------------------------------------------------------

fn fam_svg_abbrev(f: vibecheck_core::report::ModelFamily) -> &'static str {
    f.abbrev()
}

fn generate_tui_svg() -> Option<String> {
    // FONT, BG, FG, BOLD_FG, POS_C, NEG_C are module-level constants shared
    // with generate_svg().
    const FS: u32    = 12;
    const LH: f64    = 18.5;
    const CW: f64    = 7.21;
    const W: u32     = 900;
    const H: u32     = 480;
    const TOP: f64   = 32.0;
    const SB_H: f64  = 20.0;
    const LEFT_W: f64  = 358.0;
    const RIGHT_X: f64 = LEFT_W + 2.0;
    const PAD: f64   = 7.0;
    const PANE_H: f64  = H as f64 - TOP - SB_H;
    // TUI-specific palette constants
    const BORDER: &str   = "#30363d";
    const DIM: &str      = "#8b949e";
    const SEL_BG: &str   = "#2d333b";
    const KEY_COLOR: &str = "#58a6ff";   // status-bar key highlight
    const SB_TEXT: &str  = "#c9d1d9";   // status-bar body text

    // Analyze a handful of real source files to get live badges.
    let read_and_analyze = |rel: &str| -> Option<vibecheck_core::report::Report> {
        let content = std::fs::read_to_string(rel).ok()?;
        Some(vibecheck_core::analyze(&content))
    };

    let r_ai = read_and_analyze("../vibecheck-core/src/analyzers/text/ai_signals.rs")?;
    let r_cs = read_and_analyze("../vibecheck-core/src/analyzers/text/code_structure.rs")?;
    let r_ca = read_and_analyze("../vibecheck-core/src/cache.rs")?;
    let r_pl = read_and_analyze("../vibecheck-core/src/pipeline.rs")?;
    let r_rp = read_and_analyze("../vibecheck-core/src/report.rs")?;

    let detail = &r_pl;

    type Row<'a> = (usize, bool, &'static str, Option<&'a vibecheck_core::report::Report>);
    let rows: &[Row] = &[
        (0, true,  "src/",              None),
        (1, true,  "analyzers/",        None),
        (2, false, "ai_signals.rs",     Some(&r_ai)),
        (2, false, "code_structure.rs", Some(&r_cs)),
        (1, false, "cache.rs",          Some(&r_ca)),
        (1, false, "pipeline.rs",       Some(&r_pl)),
        (1, false, "report.rs",         Some(&r_rp)),
    ];

    let mut svg: Vec<String> = Vec::new();

    macro_rules! p {
        ($s:expr) => { svg.push($s.to_string()) };
        ($fmt:literal, $($arg:tt)*) => { svg.push(format!($fmt, $($arg)*)) };
    }

    // ── SVG scaffold ──────────────────────────────────────────────────────
    p!(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\">"));
    p!(format!("  <rect width=\"{W}\" height=\"{H}\" fill=\"{BG}\" rx=\"8\"/>"));
    p!("  <circle cx=\"16\" cy=\"16\" r=\"5\" fill=\"#ff5f57\"/>");
    p!("  <circle cx=\"32\" cy=\"16\" r=\"5\" fill=\"#febc2e\"/>");
    p!("  <circle cx=\"48\" cy=\"16\" r=\"5\" fill=\"#28c840\"/>");
    p!(format!("  <line x1=\"0\" y1=\"{TOP}\" x2=\"{W}\" y2=\"{TOP}\" stroke=\"{BOLD_FG}\" stroke-opacity=\"0.07\" stroke-width=\"1\"/>"));

    // ── Pane borders ──────────────────────────────────────────────────────
    p!(format!("  <rect x=\"0.5\" y=\"{TOP}\" width=\"{LEFT_W}\" height=\"{PANE_H}\" fill=\"none\" stroke=\"{BORDER}\" stroke-width=\"1\"/>"));
    p!(format!("  <rect x=\"{RIGHT_X}\" y=\"{TOP}\" width=\"{:.0}\" height=\"{PANE_H}\" fill=\"none\" stroke=\"{BORDER}\" stroke-width=\"1\"/>",
        W as f64 - RIGHT_X));

    // Pane titles (gap knocked into the top border)
    let title_y = TOP + LH * 0.76;
    p!(format!("  <rect x=\"6\" y=\"{TOP:.0}\" width=\"48\" height=\"2\" fill=\"{BG}\"/>"));
    p!(format!("  <text x=\"8\" y=\"{title_y:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{BOLD_FG}\" font-weight=\"bold\"> Files </text>"));
    p!(format!("  <rect x=\"{:.0}\" y=\"{TOP:.0}\" width=\"54\" height=\"2\" fill=\"{BG}\"/>", RIGHT_X + 6.0));
    p!(format!("  <text x=\"{:.0}\" y=\"{title_y:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{BOLD_FG}\" font-weight=\"bold\"> Detail </text>", RIGHT_X + 8.0));

    // ── Left pane: tree ───────────────────────────────────────────────────
    let tree_y0 = TOP + LH * 1.6;

    for (i, (depth, is_dir, name, report)) in rows.iter().enumerate() {
        let y = tree_y0 + i as f64 * LH;
        if y > TOP + PANE_H - LH { break; }

        let is_selected = report.map(|r| std::ptr::eq(r, detail)).unwrap_or(false);

        if is_selected {
            p!(format!("  <rect x=\"1\" y=\"{:.0}\" width=\"{:.0}\" height=\"{LH:.0}\" fill=\"{SEL_BG}\"/>",
                (y - LH * 0.78) as u32, LEFT_W as u32 - 2));
        }

        let indent = "  ".repeat(*depth);
        let prefix = if *is_dir { "▾ " } else { "  " };
        let sel    = if is_selected { "▶ " } else { "  " };
        let label  = format!("{sel}{indent}{prefix}{name}");
        let name_fill  = if *is_dir { BOLD_FG } else { FG };
        let bold_attr  = if *is_dir { " font-weight=\"bold\"" } else { "" };
        p!(format!("  <text x=\"{PAD:.0}\" y=\"{y:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{name_fill}\"{bold_attr}>{}</text>",
            xml_esc(&label)));

        let (conf_pct, color, abbrev) = if let Some(r) = report {
            ((r.attribution.confidence * 100.0) as u32,
             r.attribution.primary.svg_color(),
             fam_svg_abbrev(r.attribution.primary))
        } else {
            (85u32, ModelFamily::Claude.svg_color(), ModelFamily::Claude.abbrev())
        };
        let badge   = format!("{abbrev}  {conf_pct:>3}%");
        let badge_x = LEFT_W - PAD - badge.len() as f64 * CW;
        p!(format!("  <text x=\"{badge_x:.0}\" y=\"{y:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{color}\">{badge}</text>"));
    }

    // ── Right pane: detail ────────────────────────────────────────────────
    let dx = RIGHT_X + PAD;
    let mut dy = TOP + LH * 1.6;

    // Header: path  Family (pct%)
    let verdict   = format!("{} ({:.0}%)", detail.attribution.primary, detail.attribution.confidence * 100.0);
    let hdr_color = detail.attribution.primary.svg_color();
    let path_label = "vibecheck-core/src/pipeline.rs";
    p!(format!("  <text x=\"{dx:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{BOLD_FG}\" font-weight=\"bold\">{path_label}</text>"));
    let verdict_x = dx + path_label.len() as f64 * CW + CW * 2.0;
    p!(format!("  <text x=\"{verdict_x:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{hdr_color}\" font-weight=\"bold\">{}</text>",
        xml_esc(&verdict)));
    dy += LH * 1.5;

    // Score bars — solid <rect> (no pixel gaps), pct at fixed position after bar.
    let mut scores: Vec<_> = detail.attribution.scores.iter().collect();
    scores.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    const BAR_MAX_PX: f64 = 22.0 * CW;  // max bar width (pixels)
    const BAR_H_TUI: f64 = 10.0;        // bar rect height
    for (fam, &score) in &scores {
        if dy > TOP + PANE_H - LH { break; }
        let label = format!("  {:<10} ", fam.to_string());   // trailing space before bar
        let color = fam.svg_color();
        let pct   = format!("{:>5.1}%", score * 100.0);

        p!(format!("  <text x=\"{dx:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{DIM}\">{}</text>",
            xml_esc(&label)));
        let bx    = dx + label.len() as f64 * CW;
        let bar_w = score * BAR_MAX_PX;
        let bar_y = dy - BAR_H_TUI;
        if bar_w > 0.5 {
            p!(format!("  <rect x=\"{bx:.1}\" y=\"{bar_y:.1}\" width=\"{bar_w:.1}\" height=\"{BAR_H_TUI}\" fill=\"{color}\" rx=\"1\"/>"));
        }
        // pct at fixed column: bar_start + max_bar_width + one-space gap
        let px = bx + BAR_MAX_PX + CW;
        p!(format!("  <text x=\"{px:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{FG}\">{}</text>",
            xml_esc(&pct)));
        dy += LH;
    }
    dy += LH * 0.4;

    // Signals header
    p!(format!("  <text x=\"{dx:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{BOLD_FG}\" font-weight=\"bold\"> Signals ({}):</text>",
        detail.signals.len()));
    dy += LH;

    // Signal rows: "  [source] +weight FamilyName — description"
    // Matches the visual layout of generate_svg() with proper spacing.
    for sig in detail.signals.iter().take(7) {
        if dy > TOP + PANE_H - LH { break; }
        let sign   = if sig.weight >= 0.0 { "+" } else { "" };
        let wt_col = if sig.weight >= 0.0 { POS_C } else { NEG_C };
        let fc     = sig.family.svg_color();

        let mut x = dx;

        // "  " indent
        let indent = "  ";
        p!(format!("  <text x=\"{x:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{FG}\">{indent}</text>"));
        x += indent.len() as f64 * CW;

        // "[source] " — dimmed
        let src = format!("[{}] ", sig.source);
        p!(format!("  <text x=\"{x:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{FG}\" opacity=\"0.5\">{}</text>",
            xml_esc(&src)));
        x += src.len() as f64 * CW;

        // "+1.5 " — weight colored
        let wt_str = format!("{sign}{:.1} ", sig.weight);
        p!(format!("  <text x=\"{x:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{wt_col}\">{}</text>",
            xml_esc(&wt_str)));
        x += wt_str.len() as f64 * CW;

        // "FamilyName " — bold model color
        let fam_str = format!("{} ", sig.family.to_string());
        p!(format!("  <text x=\"{x:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{fc}\" font-weight=\"bold\">{}</text>",
            xml_esc(&fam_str)));
        x += fam_str.len() as f64 * CW;

        // "— description" — FG, clipped to fit pane
        let desc = format!("\u{2014} {}", sig.description);
        let max_chars = ((W as f64 - x - PAD) / CW) as usize;
        let clipped: String = desc.chars().take(max_chars).collect();
        p!(format!("  <text x=\"{x:.0}\" y=\"{dy:.0}\" font-family=\"{FONT}\" font-size=\"{FS}px\" fill=\"{FG}\">{}</text>",
            xml_esc(&clipped)));
        dy += LH;
    }

    // ── Status bar ────────────────────────────────────────────────────────
    let sb_y = H as f64 - SB_H;
    p!(format!("  <rect x=\"0\" y=\"{sb_y:.0}\" width=\"{W}\" height=\"{SB_H:.0}\" fill=\"{BORDER}\"/>"));
    let keys: &[(&str, &str)] = &[
        ("↑↓", " navigate  "),
        ("Enter/→", " expand  "),
        ("←", " collapse  "),
        ("d/u", " scroll detail  "),
        ("q", " quit"),
    ];
    let sb_text_y = sb_y + SB_H * 0.72;
    let mut sx = 8.0f64;
    for &(key, rest) in keys {
        p!(format!("  <text x=\"{sx:.0}\" y=\"{sb_text_y:.0}\" font-family=\"{FONT}\" font-size=\"11px\" fill=\"{KEY_COLOR}\"> {key} </text>"));
        sx += (key.len() + 2) as f64 * 6.8;
        p!(format!("  <text x=\"{sx:.0}\" y=\"{sb_text_y:.0}\" font-family=\"{FONT}\" font-size=\"11px\" fill=\"{SB_TEXT}\">{rest}</text>"));
        sx += rest.len() as f64 * 6.8;
    }

    svg.push("</svg>".into());
    Some(svg.join("\n") + "\n")
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/commands/tui.rs");
    println!("cargo:rerun-if-changed=src/output.rs");
    println!("cargo:rerun-if-changed=../vibecheck-core/src/colors.rs");
    println!("cargo:rerun-if-changed=../vibecheck-core/src/report.rs");
    println!("cargo:rerun-if-changed=../vibecheck-core/src/pipeline.rs");
    println!("cargo:rerun-if-changed=../vibecheck-core/src/cache.rs");
    println!("cargo:rerun-if-changed=../vibecheck-core/src/analyzers/text/ai_signals.rs");
    println!("cargo:rerun-if-changed=../vibecheck-core/src/analyzers/text/code_structure.rs");

    let source = match std::fs::read_to_string("src/output.rs") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("build.rs: skipping SVG generation ({e})");
            return;
        }
    };

    let report = vibecheck_core::analyze(&source);
    let svg = generate_svg(&report, "./vibecheck-cli/src/output.rs");

    if let Err(e) = std::fs::write("../.github/assets/example.svg", &svg) {
        eprintln!("build.rs: failed to write example.svg: {e}");
    }

    if let Some(tui_svg) = generate_tui_svg() {
        if let Err(e) = std::fs::write("../.github/assets/tui.svg", &tui_svg) {
            eprintln!("build.rs: failed to write tui.svg: {e}");
        }
    }
}
