#![deny(warnings)]

use vibecheck_core::heuristics::{all_heuristics, HeuristicLanguage};
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

        let fam_str = format!("{} ", sig.family);
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
        let fam_str = format!("{} ", sig.family);
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
        ("?",     " help  "),
        ("↑↓",    " navigate  "),
        ("Enter/→", " expand  "),
        ("←",     " collapse  "),
        ("d/u",   " scroll ↕  "),
        ("⇧←/⇧→", " scroll ↔  "),
        ("h",     " history  "),
        ("q",     " quit"),
    ];
    let sb_text_y = sb_y + SB_H * 0.72;
    let mut sx = 8.0f64;
    for &(key, rest) in keys {
        p!(format!("  <text x=\"{sx:.0}\" y=\"{sb_text_y:.0}\" font-family=\"{FONT}\" font-size=\"11px\" fill=\"{KEY_COLOR}\"> {key} </text>"));
        // Use char count (not byte len) so multi-byte unicode chars don't over-space.
        sx += (key.chars().count() + 2) as f64 * 6.8;
        p!(format!("  <text x=\"{sx:.0}\" y=\"{sb_text_y:.0}\" font-family=\"{FONT}\" font-size=\"11px\" fill=\"{SB_TEXT}\">{rest}</text>"));
        sx += rest.chars().count() as f64 * 6.8;
    }

    svg.push("</svg>".into());
    Some(svg.join("\n") + "\n")
}

// ---------------------------------------------------------------------------
// Architecture diagram SVG
// ---------------------------------------------------------------------------

fn generate_architecture_svg() -> String {
    // ── Palette (handcrafted for #161b22 dark background) ─────────────────────
    // Tailwind-inspired pastels — readable, harmonious, no purple, no yellow/amber.
    const SRC_C:   &str = "#6ee7b7";  // emerald-300  — source input / consumers
    const CACHE_C: &str = "#38bdf8";  // sky-400      — incremental cache / core wrapper
    const STORE_C: &str = "#2dd4bf";  // teal-400     — cache write-back store
    const ANA_C:   &str = "#60a5fa";  // blue-400     — analysis pipeline stages
    const REP_C:   &str = "#f472b6";  // pink-400     — report output
    const ARROW:   &str = "#e2e8f0";  // slate-200    — arrows
    const DIM:     &str = "#94a3b8";  // slate-400    — body / subtitle text

    const W: u32 = 1000;
    const H: u32 = 630;
    const RX: u32 = 8;

    let mut out: Vec<String> = Vec::new();
    macro_rules! p { ($s:expr) => { out.push($s) } }

    p!(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\">"));
    p!(format!("  <rect width=\"{W}\" height=\"{H}\" fill=\"{BG}\" rx=\"10\"/>"));

    // Defs: two arrowhead markers — solid (pipeline) and dashed (cache-hit bypass)
    p!(format!("  <defs>\n\
         \x20   <marker id=\"arr\" viewBox=\"0 0 14 14\" markerWidth=\"14\" markerHeight=\"14\"\n\
         \x20           refX=\"11\" refY=\"7\" orient=\"auto\" markerUnits=\"userSpaceOnUse\">\n\
         \x20     <path d=\"M 2 2 L 11 7 L 2 12\" fill=\"none\" stroke=\"{ARROW}\"\n\
         \x20           stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n\
         \x20   </marker>\n\
         \x20   <marker id=\"arr-cache\" viewBox=\"0 0 14 14\" markerWidth=\"14\" markerHeight=\"14\"\n\
         \x20           refX=\"11\" refY=\"7\" orient=\"auto\" markerUnits=\"userSpaceOnUse\">\n\
         \x20     <path d=\"M 2 2 L 11 7 L 2 12\" fill=\"none\" stroke=\"#fbbf24\"\n\
         \x20           stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n\
         \x20   </marker>\n\
         \x20 </defs>"));

    // Title bar
    p!("  <circle cx=\"22\" cy=\"18\" r=\"5\" fill=\"#ff5f57\"/>".into());
    p!("  <circle cx=\"38\" cy=\"18\" r=\"5\" fill=\"#febc2e\"/>".into());
    p!("  <circle cx=\"54\" cy=\"18\" r=\"5\" fill=\"#28c840\"/>".into());
    p!(format!("  <line x1=\"0\" y1=\"32\" x2=\"{W}\" y2=\"32\" stroke=\"{BOLD_FG}\" stroke-opacity=\"0.06\" stroke-width=\"1\"/>"));
    p!(format!("  <text x=\"{:.0}\" y=\"26\" font-family=\"{FONT}\" font-size=\"14px\" fill=\"{BOLD_FG}\" font-weight=\"bold\" text-anchor=\"middle\" letter-spacing=\"0.5\">vibecheck \u{2014} analysis pipeline</text>",
        W as f64 / 2.0));

    // ── Helpers ───────────────────────────────────────────────────────────────
    // Container: dashed border, barely-there tinted fill (section wrappers)
    let container = |x: u32, y: u32, w: u32, h: u32, c: &str| -> Vec<String> { vec![
        format!("  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{RX}\" fill=\"{c}\" fill-opacity=\"0.05\" stroke=\"none\"/>"),
        format!("  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{RX}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"1.2\" stroke-opacity=\"0.40\" stroke-dasharray=\"6 3\"/>"),
    ]};
    // Card: clear tinted fill + crisp border (pipeline stage boxes)
    let card = |x: u32, y: u32, w: u32, h: u32, c: &str| -> Vec<String> { vec![
        format!("  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{RX}\" fill=\"{c}\" fill-opacity=\"0.10\" stroke=\"none\"/>"),
        format!("  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"{RX}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"1.8\" stroke-opacity=\"0.90\"/>"),
    ]};
    let bold = |cx: u32, y: u32, c: &str, fs: u32, s: &str| -> String {
        format!("  <text x=\"{cx}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"{fs}px\" fill=\"{c}\" font-weight=\"bold\" text-anchor=\"middle\">{}</text>",
            xml_esc(s))
    };
    let section_label = |cx: u32, y: u32, c: &str, s: &str| -> String {
        format!("  <text x=\"{cx}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"10px\" fill=\"{c}\" font-weight=\"bold\" text-anchor=\"middle\" letter-spacing=\"1.5\">{}</text>",
            xml_esc(s))
    };
    let dim = |cx: u32, y: u32, s: &str| -> String {
        format!("  <text x=\"{cx}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"11px\" fill=\"{DIM}\" text-anchor=\"middle\">{}</text>",
            xml_esc(s))
    };
    let note = |cx: u32, y: u32, s: &str| -> String {
        format!("  <text x=\"{cx}\" y=\"{y}\" font-family=\"{FONT}\" font-size=\"12px\" fill=\"#fbbf24\" font-weight=\"bold\" text-anchor=\"middle\">{}</text>",
            xml_esc(s))
    };
    let arrow = |x1: u32, y1: u32, x2: u32, y2: u32| -> String {
        format!("  <line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{ARROW}\" stroke-width=\"2\" marker-end=\"url(#arr)\"/>")
    };
    let cache_arrow = |x1: u32, y1: u32, x2: u32, y2: u32| -> String {
        format!("  <line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#fbbf24\" stroke-width=\"1.8\" stroke-dasharray=\"5 3\" marker-end=\"url(#arr-cache)\"/>")
    };

    // ══ Layout ════════════════════════════════════════════════════════════════
    //  source:      x=10  w=142  cx=81    y=48  h=72   (right=152)
    //  core:        x=162 w=818  cx=571   y=44  h=458  (dashed, bottom=502)
    //    cache:     x=178 w=256  cx=306   y=66  h=88   (right=434, bottom=154)
    //    text:      x=178 w=256  cx=306   y=206 h=68   (gap=52, bottom=274)
    //    syntax:    x=178 w=256  cx=306   y=326 h=68   (gap=52, bottom=394)
    //    agg:       x=178 w=256  cx=306   y=446 h=50   (gap=52, bottom=496)
    //    report:    x=620 w=340  cx=790   y=200 h=164  (gap=186, bottom=364)
    //    cache-wbk: x=680 w=220  cx=790   y=406 h=46   (bottom=452)
    //  consumers:   x=162 w=818           y=522 h=78   (dashed, bottom=600)
    //    cli panel: x=162–571  cx=366     divider x=571
    //    ext panel: x=571–980  cx=775
    // ═════════════════════════════════════════════════════════════════════════

    // Source files
    for s in card(10, 48, 142, 72, SRC_C) { p!(s); }
    p!(bold(81, 78, BOLD_FG, 13, "source files"));
    p!(dim(81, 97, ".rs  .py  .js  .go"));

    // vibecheck-core wrapper
    for s in container(162, 44, 818, 458, CACHE_C) { p!(s); }
    p!(section_label(571, 59, CACHE_C, "vibecheck-core"));

    // Incremental cache
    for s in card(178, 66, 256, 88, CACHE_C) { p!(s); }
    p!(bold(306, 91,  BOLD_FG, 13, "incremental cache"));
    p!(dim(306, 111, "SHA-256 content hash per file"));
    p!(dim(306, 129, "hit \u{2192} return cached Report"));
    p!(dim(306, 147, "miss \u{2192} run analysis pipeline \u{2193}"));

    // Text-pattern analysis
    for s in card(178, 206, 256, 68, ANA_C) { p!(s); }
    p!(bold(306, 232, BOLD_FG, 13, "text-pattern analysis"));
    p!(dim(306, 252, "comments \u{b7} naming \u{b7} structure \u{b7} idioms"));

    // Syntax tree analysis
    for s in card(178, 326, 256, 68, ANA_C) { p!(s); }
    p!(bold(306, 352, BOLD_FG, 13, "syntax tree analysis"));
    p!(dim(306, 372, "language-aware \u{b7} per function / class"));

    // Aggregate + normalize
    for s in card(178, 446, 256, 50, ANA_C) { p!(s); }
    p!(bold(306, 469, BOLD_FG, 13, "aggregate + normalize"));
    p!(dim(306, 487, "weighted scoring per model family"));

    // Report
    for s in card(620, 200, 340, 164, REP_C) { p!(s); }
    p!(bold(790, 226, REP_C, 14, "Report"));
    p!(dim(790, 250, "primary attribution"));
    p!(dim(790, 268, "confidence score"));
    p!(dim(790, 286, "score distribution"));
    p!(dim(790, 304, "per-signal breakdown"));
    p!(dim(790, 322, "symbol-level reports"));

    // Cache write-back — intermediate store between Report and consumers
    for s in card(680, 406, 220, 46, STORE_C) { p!(s); }
    p!(bold(790, 432, STORE_C, 12, "cache"));

    // Consumers wrapper
    for s in container(162, 522, 818, 78, SRC_C) { p!(s); }

    // Left panel — vibecheck-cli (3×2 grid, columns at cx=230/366/502)
    p!(section_label(366, 538, SRC_C, "vibecheck-cli"));
    p!(dim(230, 560, "analyze")); p!(dim(366, 560, "tui")); p!(dim(502, 560, "watch"));
    p!(dim(230, 578, "history")); p!(dim(366, 578, "heuristics")); p!(dim(502, 578, "help"));

    // Divider
    p!("  <line x1=\"571\" y1=\"530\" x2=\"571\" y2=\"594\" stroke=\"#30363d\" stroke-width=\"1\"/>".into());

    // Right panel — external crates
    p!(section_label(775, 538, SRC_C, "external crates"));
    p!(dim(775, 560, "analyze()  \u{b7}  analyze_file_symbols()"));
    p!(dim(775, 578, "analyze_directory_with(path, \u{2026})"));

    // ── Arrows ────────────────────────────────────────────────────────────────
    p!(arrow(152, 84,  178, 103));   // source → cache
    p!(arrow(306, 154, 306, 206));   // cache → text
    p!(arrow(306, 274, 306, 326));   // text → syntax
    p!(arrow(306, 394, 306, 446));   // syntax → agg
    p!(arrow(434, 240, 620, 250));   // text → report
    p!(arrow(434, 360, 620, 304));   // syntax → report
    p!(arrow(434, 471, 620, 350));   // agg → report
    p!(arrow(790, 364, 790, 406));   // report → cache write-back
    p!(arrow(710, 452, 366, 522));   // cache → vibecheck-cli
    p!(arrow(870, 452, 775, 522));   // cache → external crates

    // Cache-hit bypass: right of cache box → top of Report
    // Midpoint at (527,155); label offset above the line
    p!(cache_arrow(434, 110, 620, 200));
    p!(note(540, 132, "cache hit"));

    out.push("</svg>".into());
    out.join("\n") + "\n"
}

// ---------------------------------------------------------------------------
// Logo SVG
// ---------------------------------------------------------------------------

fn generate_logo_svg() -> String {
    const W: u32 = 760;
    const H: u32 = 215;
    const LOGO_BG: &str = "#0d1117";
    const FS: u32 = 76;
    const WORD: &str = "vibecheck";
    const CW_RATIO: f64 = 0.601;

    let models: &[(&str, &str)] = &[
        ("Claude",  "#d2a8ff"),
        ("Gemini",  "#79c0ff"),
        ("Copilot", "#39c5cf"),
        ("GPT",     "#7ee787"),
        ("Human",   "#e3b341"),
    ];
    let grad_stops: &[(u32, &str)] = &[
        (  0, "#d2a8ff"),
        ( 25, "#79c0ff"),
        ( 50, "#39c5cf"),
        ( 75, "#7ee787"),
        (100, "#e3b341"),
    ];

    let text_w = WORD.len() as f64 * FS as f64 * CW_RATIO;
    let x1 = (W as f64 - text_w) / 2.0;
    let x2 = x1 + text_w;

    let stop_xml: String = grad_stops
        .iter()
        .map(|(p, c)| format!("      <stop offset=\"{p}%\" stop-color=\"{c}\"/>"))
        .collect::<Vec<_>>()
        .join("\n");

    let n = models.len();
    let dot_cy: u32 = 175;
    let label_y: u32 = 195;
    let sep_y: u32   = 152;

    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {W} {H}\" width=\"{W}\" height=\"{H}\">\n\
         \x20 <defs>\n\
         \x20   <linearGradient id=\"g\" gradientUnits=\"userSpaceOnUse\" x1=\"{x1:.1}\" y1=\"0\" x2=\"{x2:.1}\" y2=\"0\">\n\
         {stop_xml}\n\
         \x20   </linearGradient>\n\
         \x20   <filter id=\"glow\" x=\"-20%\" y=\"-80%\" width=\"140%\" height=\"260%\">\n\
         \x20     <feGaussianBlur in=\"SourceGraphic\" stdDeviation=\"9\" result=\"blur\"/>\n\
         \x20     <feMerge>\n\
         \x20       <feMergeNode in=\"blur\"/>\n\
         \x20       <feMergeNode in=\"SourceGraphic\"/>\n\
         \x20     </feMerge>\n\
         \x20   </filter>\n\
         \x20 </defs>\n\
         \n\
         \x20 <rect width=\"{W}\" height=\"{H}\" fill=\"{LOGO_BG}\" rx=\"14\"/>\n\
         \n\
         \x20 <!-- main wordmark — glow pass -->\n\
         \x20 <text x=\"50%\" y=\"108\" text-anchor=\"middle\"\n\
         \x20       font-family=\"{FONT}\" font-size=\"{FS}px\" font-weight=\"bold\"\n\
         \x20       fill=\"url(#g)\" opacity=\"0.45\" filter=\"url(#glow)\">vibecheck</text>\n\
         \x20 <!-- main wordmark — crisp pass -->\n\
         \x20 <text x=\"50%\" y=\"108\" text-anchor=\"middle\"\n\
         \x20       font-family=\"{FONT}\" font-size=\"{FS}px\" font-weight=\"bold\"\n\
         \x20       fill=\"url(#g)\">vibecheck</text>\n\
         \n\
         \x20 <!-- tagline -->\n\
         \x20 <text x=\"50%\" y=\"134\" text-anchor=\"middle\"\n\
         \x20       font-family=\"{FONT}\" font-size=\"12.5px\" fill=\"#8b949e\" letter-spacing=\"0.5\">detect the AI behind the code</text>\n\
         \n\
         \x20 <!-- separator -->\n\
         \x20 <line x1=\"{:.0}\" y1=\"{sep_y}\" x2=\"{:.0}\" y2=\"{sep_y}\"\n\
         \x20       stroke=\"#21262d\" stroke-width=\"1\"/>\n",
        W as f64 * 0.08,
        W as f64 * 0.92,
    );

    for (i, (name, color)) in models.iter().enumerate() {
        let cx = W as f64 / (n as f64 + 1.0) * (i as f64 + 1.0);
        svg.push_str(&format!(
            "\n\
             \x20 <circle cx=\"{cx:.1}\" cy=\"{dot_cy}\" r=\"5\" fill=\"{color}\" opacity=\"0.9\"/>\n\
             \x20 <text x=\"{cx:.1}\" y=\"{label_y}\" text-anchor=\"middle\"\n\
             \x20       font-family=\"{FONT}\" font-size=\"11px\" fill=\"{color}\">{name}</text>"
        ));
    }

    svg.push_str("\n</svg>\n");
    svg
}

// ---------------------------------------------------------------------------
// README signal catalogue injection
// ---------------------------------------------------------------------------

fn generate_readme_signals() {
    const START_MARKER: &str = "<!-- vibecheck:signals-start -->";
    const END_MARKER:   &str = "<!-- vibecheck:signals-end -->";
    const MAX_PER_GROUP: usize = 5;

    // Language groups: (display name, CST/text variants merged together)
    let groups: &[(&str, &[HeuristicLanguage])] = &[
        ("rust",       &[HeuristicLanguage::Rust,   HeuristicLanguage::RustCst]),
        ("python",     &[HeuristicLanguage::Python, HeuristicLanguage::PythonCst]),
        ("javascript", &[HeuristicLanguage::Js,     HeuristicLanguage::JsCst]),
        ("go",         &[HeuristicLanguage::Go,     HeuristicLanguage::GoCst]),
    ];

    let mut rows: Vec<String> = vec![
        "| Language | Signal ID | Family | Weight | Description |".into(),
        "|----------|-----------|--------|--------|-------------|".into(),
    ];

    for (lang_name, variants) in groups {
        let mut signals: Vec<_> = all_heuristics()
            .iter()
            .filter(|s| variants.contains(&s.language))
            .collect();
        // Highest weight first; secondary sort by id for determinism
        signals.sort_by(|a, b| {
            b.default_weight
                .partial_cmp(&a.default_weight)
                .unwrap()
                .then_with(|| a.id.cmp(b.id))
        });
        for sig in signals.iter().take(MAX_PER_GROUP) {
            rows.push(format!(
                "| {} | `{}` | {} | {:.1} | {} |",
                lang_name, sig.id, sig.family, sig.default_weight, sig.description
            ));
        }
    }

    let table = rows.join("\n");

    let readme_path = "../README.md";
    let content = match std::fs::read_to_string(readme_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("build.rs: cannot read README.md: {e}"); return; }
    };

    if let (Some(s), Some(e)) = (content.find(START_MARKER), content.find(END_MARKER)) {
        let new_content = format!(
            "{}{}\n{}\n{}{}",
            &content[..s + START_MARKER.len()],
            "\n",
            table,
            END_MARKER,
            &content[e + END_MARKER.len()..]
        );
        if let Err(e) = std::fs::write(readme_path, new_content) {
            eprintln!("build.rs: failed to update README.md signals table: {e}");
        }
    } else {
        eprintln!("build.rs: README.md missing signal markers — skipping table injection");
    }
}

fn generate_readme_badges() {
    const START_MARKER: &str = "<!-- vibecheck:badges-start -->";
    const END_MARKER:   &str = "<!-- vibecheck:badges-end -->";

    let dirs = ["../vibecheck-core/src", "../vibecheck-cli/src"];
    let mut family_weighted: std::collections::HashMap<ModelFamily, f64> = std::collections::HashMap::new();
    let mut total_loc: f64 = 0.0;

    for dir in &dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        fn collect_rs(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        collect_rs(&entry.path(), files);
                    }
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path.to_path_buf());
            }
        }
        let mut files = Vec::new();
        for entry in entries.flatten() {
            collect_rs(&entry.path(), &mut files);
        }
        for file in &files {
            let content = match std::fs::read_to_string(file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let report = vibecheck_core::analyze(&content);
            let loc = report.metadata.lines_of_code as f64;
            if loc < 1.0 { continue; }
            total_loc += loc;
            for (fam, &score) in &report.attribution.scores {
                *family_weighted.entry(*fam).or_default() += score * loc;
            }
        }
    }

    if total_loc < 1.0 { return; }

    let mut scores: Vec<(ModelFamily, f64)> = family_weighted
        .iter()
        .map(|(f, &w)| (*f, (w / total_loc * 100.0).round()))
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.to_string().cmp(&b.0.to_string())));

    let badges: Vec<String> = scores
        .iter()
        .map(|(fam, pct)| {
            let (r, g, b) = fam.rgb();
            let color = format!("{r:02x}{g:02x}{b:02x}");
            let pct_int = *pct as u32;
            format!(
                "[![{fam} {pct_int}%](https://img.shields.io/badge/{fam}-{pct_int}%25-{color})](https://github.com/o-k-a-y/vibecheck)"
            )
        })
        .collect();

    let badge_line = badges.join("\n");

    let readme_path = "../README.md";
    let content = match std::fs::read_to_string(readme_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("build.rs: cannot read README.md for badges: {e}"); return; }
    };

    if let (Some(s), Some(e)) = (content.find(START_MARKER), content.find(END_MARKER)) {
        let new_content = format!(
            "{}{}\n{}\n{}{}",
            &content[..s + START_MARKER.len()],
            "\n",
            badge_line,
            END_MARKER,
            &content[e + END_MARKER.len()..]
        );
        if let Err(e) = std::fs::write(readme_path, new_content) {
            eprintln!("build.rs: failed to update README.md badges: {e}");
        }
    } else {
        eprintln!("build.rs: README.md missing badge markers — skipping badge injection");
    }
}

fn main() {
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

    let arch_svg = generate_architecture_svg();
    if let Err(e) = std::fs::write("../.github/assets/architecture.svg", &arch_svg) {
        eprintln!("build.rs: failed to write architecture.svg: {e}");
    }

    let logo_svg = generate_logo_svg();
    if let Err(e) = std::fs::write("../.github/assets/logo.svg", &logo_svg) {
        eprintln!("build.rs: failed to write logo.svg: {e}");
    }

    generate_readme_signals();
    generate_readme_badges();
}
