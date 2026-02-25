use anyhow::Result;
use vibecheck_core::heuristics::{all_heuristics, signal_ids};

pub fn run(format: &str) -> Result<()> {
    match format {
        "toml" => print_toml(),
        _ => print_table(),
    }
    Ok(())
}

fn print_table() {
    // Group by language then analyzer
    let col_widths = (8usize, 10usize, 38usize, 6usize, 7usize);

    println!(
        "{:<lang$}  {:<ana$}  {:<id$}  {:<att$}  {:<w$}  Description",
        "Language",
        "Analyzer",
        "Signal ID",
        "Family",
        "Weight",
        lang = col_widths.0,
        ana = col_widths.1,
        id = col_widths.2,
        att = col_widths.3,
        w = col_widths.4,
    );
    let separator = format!(
        "{}\u{2500}{}\u{2500}{}\u{2500}{}\u{2500}{}\u{2500}{}",
        "\u{2500}".repeat(col_widths.0),
        "\u{2500}".repeat(col_widths.1),
        "\u{2500}".repeat(col_widths.2),
        "\u{2500}".repeat(col_widths.3),
        "\u{2500}".repeat(col_widths.4),
        "\u{2500}".repeat(40),
    );
    println!("{separator}");

    for h in all_heuristics() {
        let family = format!("{:?}", h.family);
        let lang_str = h.language.to_string();
        println!(
            "{:<lang$}  {:<ana$}  {:<id$}  {:<att$}  {:<w$.2}  {}",
            lang_str,
            h.analyzer,
            h.id,
            family,
            h.default_weight,
            h.description,
            lang = col_widths.0,
            ana = col_widths.1,
            id = col_widths.2,
            att = col_widths.3,
            w = col_widths.4,
        );
    }
}

fn print_toml() {
    println!("[heuristics]");
    println!("# Adjust signal weights (0.0 = disabled).");
    println!("# Uncomment and edit lines to override defaults.");
    println!();

    let mut last_lang = None;
    for h in all_heuristics() {
        if last_lang != Some(h.language) {
            if last_lang.is_some() {
                println!();
            }
            println!("# ── {} ──", h.language);
            last_lang = Some(h.language);
        }
        let family = format!("{:?}", h.family);
        println!(
            "# \"{id}\" = {w:.1}   # {fam}: {desc}",
            id = h.id,
            w = h.default_weight,
            fam = family,
            desc = h.description,
        );
    }
}

// Suppress dead_code lint — signal_ids is referenced at compile time for completeness checks.
#[allow(dead_code)]
const _ALL_IDS_REFERENCED: () = {
    let _ = signal_ids::RUST_ERRORS_ZERO_UNWRAP;
    let _ = signal_ids::RUST_CST_COMPLEXITY_LOW;
};
