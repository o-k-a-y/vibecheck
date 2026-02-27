#![deny(dead_code)]

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

mod commands;
mod output;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "vibecheck",
    version,
    about = "Detect AI-generated code and attribute it to a model family",
    long_about = "vibecheck detects AI-generated code and attributes it to a model family \
                  (Claude, GPT, Copilot, Gemini, or Human). It analyzes source code using \
                  text-pattern heuristics and tree-sitter CST metrics, then aggregates \
                  signals into a probability distribution.\n\n\
                  Run with no arguments to open the TUI browser. Pass a path to analyze \
                  files directly. Use subcommands for specific workflows.",
    after_help = "EXAMPLES:\n  \
                  vibecheck                           Open TUI in current directory\n  \
                  vibecheck src/main.rs               Analyze a single file\n  \
                  vibecheck src/ --format json         Analyze a directory as JSON\n  \
                  vibecheck src/ --assert-family human  CI gate: fail if AI-generated\n  \
                  vibecheck analyze --symbols src/lib.rs  Symbol-level attribution\n  \
                  vibecheck heuristics --format toml   Dump signal weights as TOML",
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// File or directory to analyze (shorthand for `vibecheck analyze <path>`).
    path: Option<PathBuf>,

    /// Output format: pretty (colored), text (plain), or json (machine-readable).
    #[arg(long, default_value = "pretty", requires = "path")]
    format: String,

    /// Exit 1 if any file is NOT attributed to one of these families.
    /// Comma-separated, e.g. `--assert-family claude,gpt`
    #[arg(long, value_delimiter = ',', requires = "path")]
    assert_family: Option<Vec<String>>,

    /// Skip the content-addressed cache (always re-analyze).
    #[arg(long, requires = "path")]
    no_cache: bool,

    /// Perform symbol-level analysis and show per-function attribution.
    #[arg(long, requires = "path")]
    symbols: bool,

    /// Path to a `.vibecheck` config file (default: auto-discovered from project root).
    #[arg(long, requires = "path")]
    ignore_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a file or directory for AI-generated code.
    #[command(
        long_about = "Analyze source files for AI-generated code patterns and attribute each \
                      file to a model family. Supports Rust, Python, JavaScript, and Go.\n\n\
                      By default, results are cached by file content hash (SHA-256). Use \
                      --no-cache to force re-analysis. Use --symbols for per-function attribution.",
        after_help = "EXAMPLES:\n  \
                      vibecheck analyze src/main.rs\n  \
                      vibecheck analyze src/ --format json\n  \
                      vibecheck analyze src/ --assert-family human --no-cache\n  \
                      vibecheck analyze --symbols src/lib.rs",
    )]
    Analyze(AnalyzeArgs),

    /// Interactive TUI codebase browser with confidence bars.
    #[command(
        long_about = "Open a two-pane terminal browser: file tree with family badges on the \
                      left, signal/score/symbol breakdown on the right. Press 'h' on any file \
                      to view per-commit git history attribution.",
        after_help = "KEYBINDINGS:\n  \
                      j/↓       Move down\n  \
                      k/↑       Move up\n  \
                      Enter/→/l Expand directory\n  \
                      ←         Collapse / go to parent\n  \
                      d/PgDn    Scroll detail pane down\n  \
                      u/PgUp    Scroll detail pane up\n  \
                      h         Toggle git history panel\n  \
                      q/Ctrl+C  Quit",
    )]
    Tui(TuiArgs),

    /// Watch a path and re-analyze files on every save.
    #[command(
        long_about = "Monitor a file or directory for changes using OS file-system events \
                      (inotify/kqueue/FSEvents). On each save, re-analyze the changed file \
                      and print the updated attribution to stdout. Uses a 300ms debounce \
                      and 2s per-file cooldown.",
    )]
    Watch(WatchArgs),

    /// Walk git history and show per-commit attribution over time.
    #[command(
        long_about = "Replay git history for a file and show how attribution changed over \
                      commits. Reads blobs directly from the git object store (no working-tree \
                      checkout). Prints a table: COMMIT | DATE | FAMILY | CONFIDENCE | CHANGE.",
        after_help = "EXAMPLES:\n  \
                      vibecheck history src/pipeline.rs\n  \
                      vibecheck history src/lib.rs --limit 5",
    )]
    History(HistoryArgs),

    /// List all detection signals with their default weights.
    #[command(
        long_about = "Display the full catalogue of detection heuristics. Each signal has a \
                      stable ID, weight, and target model family. Use --format toml to generate \
                      a block ready to paste into your .vibecheck config for weight overrides.",
        after_help = "EXAMPLES:\n  \
                      vibecheck heuristics\n  \
                      vibecheck heuristics --format toml",
    )]
    Heuristics(HeuristicsArgs),
}

#[derive(Args)]
struct AnalyzeArgs {
    /// File or directory to analyze.
    path: PathBuf,

    /// Output format: pretty (colored), text (plain), or json (machine-readable).
    #[arg(long, default_value = "pretty")]
    format: String,

    /// Exit 1 if any file is NOT attributed to one of these families.
    /// Comma-separated, e.g. `--assert-family claude,gpt,human`
    #[arg(long, value_delimiter = ',')]
    assert_family: Option<Vec<String>>,

    /// Skip the content-addressed cache (always re-analyze).
    #[arg(long)]
    no_cache: bool,

    /// Perform symbol-level analysis (per-function/method attribution).
    #[arg(long)]
    symbols: bool,

    /// Path to a `.vibecheck` config file (default: auto-discovered from project root).
    #[arg(long)]
    ignore_file: Option<PathBuf>,
}

#[derive(Args)]
struct TuiArgs {
    /// Directory to browse.
    path: PathBuf,

    /// Path to a `.vibecheck` config file (default: auto-discovered from project root).
    #[arg(long)]
    ignore_file: Option<PathBuf>,
}

#[derive(Args)]
struct WatchArgs {
    /// File or directory to watch.
    path: PathBuf,

    /// Skip the cache (always re-analyze on each change).
    #[arg(long)]
    no_cache: bool,

    /// Path to a `.vibecheck` config file (default: auto-discovered from project root).
    #[arg(long)]
    ignore_file: Option<PathBuf>,
}

#[derive(Args)]
struct HistoryArgs {
    /// File whose git history to replay.
    path: PathBuf,

    /// Maximum number of commits to show (default: 20).
    #[arg(long, short = 'n', default_value = "20")]
    limit: usize,
}

#[derive(Args)]
struct HeuristicsArgs {
    /// Output format: `table` (default) or `toml`.
    #[arg(long, default_value = "table")]
    format: String,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_parses_without_error() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_version_is_set() {
        let cmd = Cli::command();
        assert!(
            cmd.get_version().is_some(),
            "CLI should have a version set"
        );
    }

    #[test]
    fn cli_has_all_subcommands() {
        let cmd = Cli::command();
        let names: Vec<_> = cmd.get_subcommands().map(|s| s.get_name().to_string()).collect();
        assert!(names.contains(&"analyze".to_string()));
        assert!(names.contains(&"tui".to_string()));
        assert!(names.contains(&"watch".to_string()));
        assert!(names.contains(&"history".to_string()));
        assert!(names.contains(&"heuristics".to_string()));
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Analyze(a)) => commands::analyze::run(
            &a.path,
            &a.format,
            a.no_cache,
            a.symbols,
            a.assert_family,
            a.ignore_file.as_ref(),
        ),

        Some(Command::Tui(a)) => commands::tui::run(&a.path, a.ignore_file.as_ref()),

        Some(Command::Watch(a)) => commands::watch::run(&a.path, a.no_cache, a.ignore_file.as_ref()),

        Some(Command::History(a)) => commands::history::run(&a.path, Some(a.limit)),

        Some(Command::Heuristics(a)) => commands::heuristics::run(&a.format),

        None => match cli.path {
            Some(path) => commands::analyze::run(
                &path,
                &cli.format,
                cli.no_cache,
                cli.symbols,
                cli.assert_family,
                cli.ignore_file.as_ref(),
            ),
            None => {
                let cwd = std::env::current_dir()?;
                commands::tui::run(&cwd, None)
            }
        },
    }
}
