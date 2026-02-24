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
    about = "Detect AI-generated code",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// File or directory to analyze (shorthand for `vibecheck analyze <path>`).
    path: Option<PathBuf>,

    /// Output format: pretty, text, or json.
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
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a file or directory (same as the bare-path form).
    Analyze(AnalyzeArgs),

    /// Interactive TUI codebase browser with confidence bars.
    Tui(TuiArgs),

    /// Watch a path and re-analyze files on every save.
    Watch(WatchArgs),

    /// Walk git history and show how attribution changed over time.
    History(HistoryArgs),
}

#[derive(Args)]
struct AnalyzeArgs {
    /// File or directory to analyze.
    path: PathBuf,

    #[arg(long, default_value = "pretty")]
    format: String,

    #[arg(long, value_delimiter = ',')]
    assert_family: Option<Vec<String>>,

    #[arg(long)]
    no_cache: bool,

    #[arg(long)]
    symbols: bool,
}

#[derive(Args)]
struct TuiArgs {
    /// Directory to browse.
    path: PathBuf,
}

#[derive(Args)]
struct WatchArgs {
    /// File or directory to watch.
    path: PathBuf,

    /// Skip the cache (always re-analyze on each change).
    #[arg(long)]
    no_cache: bool,
}

#[derive(Args)]
struct HistoryArgs {
    /// File whose git history to replay.
    path: PathBuf,

    /// Maximum number of commits to show (default: 20).
    #[arg(long, short = 'n', default_value = "20")]
    limit: usize,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Analyze(a)) => commands::analyze::run(
            &a.path,
            &a.format,
            a.no_cache,
            a.symbols,
            a.assert_family,
        ),

        Some(Command::Tui(a)) => commands::tui::run(&a.path),

        Some(Command::Watch(a)) => commands::watch::run(&a.path, a.no_cache),

        Some(Command::History(a)) => commands::history::run(&a.path, Some(a.limit)),

        None => match cli.path {
            Some(path) => commands::analyze::run(
                &path,
                &cli.format,
                cli.no_cache,
                cli.symbols,
                cli.assert_family,
            ),
            None => {
                let cwd = std::env::current_dir()?;
                commands::tui::run(&cwd)
            }
        },
    }
}
