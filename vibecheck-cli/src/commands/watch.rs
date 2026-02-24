use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use vibecheck_core::ignore_rules::{IgnoreConfig, IgnoreRules};
use vibecheck_core::output::OutputFormat;

use crate::commands::analyze::format_report;

const DEBOUNCE: Duration = Duration::from_millis(300);
/// Minimum gap between two analyses of the same file. Prevents re-analysis
/// from late-arriving OS events (kernel batching, atomic-rename sequences).
const COOLDOWN: Duration = Duration::from_secs(2);
const SUPPORTED_EXTS: &[&str] = &["rs", "py", "js", "ts", "jsx", "tsx", "go"];

pub fn run(path: &Path, no_cache: bool, ignore_file: Option<&PathBuf>) -> Result<()> {
    let ignore: Box<dyn IgnoreRules> = match ignore_file {
        Some(f) => Box::new(IgnoreConfig::from_file(f)?),
        None => Box::new(IgnoreConfig::load(path)),
    };

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(path, RecursiveMode::Recursive)?;

    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    println!("Watching {} — Ctrl+C to stop\n", abs.display());

    // Debounce: collect events for DEBOUNCE duration, then process unique paths.
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut deadline: Option<Instant> = None;
    // Per-file cooldown: skip re-analysis if the file was analyzed < COOLDOWN ago.
    let mut last_analyzed: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        // Block for up to DEBOUNCE, collecting events.
        let timeout = deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or(DEBOUNCE);

        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                for p in event.paths {
                    if is_supported(&p) && !ignore.is_ignored(&p) {
                        pending.insert(p);
                        deadline.get_or_insert_with(|| Instant::now() + DEBOUNCE);
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Watch error: {e}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Fire when the debounce window has elapsed and we have pending paths.
        let ready = deadline.map(|d| Instant::now() >= d).unwrap_or(false);
        if ready && !pending.is_empty() {
            let now = Instant::now();
            let paths: Vec<PathBuf> = pending.drain().collect();
            for p in &paths {
                if last_analyzed
                    .get(p)
                    .map(|&t| now.duration_since(t) < COOLDOWN)
                    .unwrap_or(false)
                {
                    continue;
                }
                last_analyzed.insert(p.clone(), now);
                analyze_and_print(p, no_cache);
            }
            // Drain events that accumulated during analysis. Keep any for
            // *different* files (user saved a second file while the first was
            // being analyzed); discard re-fires for paths we just processed.
            let just_ran: HashSet<&PathBuf> = paths.iter().collect();
            while let Ok(Ok(event)) = rx.try_recv() {
                for p in event.paths {
                    if is_supported(&p) && !ignore.is_ignored(&p) && !just_ran.contains(&p) {
                        pending.insert(p);
                    }
                }
            }
            deadline = if pending.is_empty() {
                None
            } else {
                Some(Instant::now() + DEBOUNCE)
            };
        }
    }

    Ok(())
}

fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTS.contains(&e))
        .unwrap_or(false)
}

fn analyze_and_print(path: &Path, no_cache: bool) {
    let now = chrono_now();
    let analyze: fn(&Path) -> std::io::Result<vibecheck_core::report::Report> = if no_cache {
        vibecheck_core::analyze_file_no_cache
    } else {
        vibecheck_core::analyze_file
    };

    match analyze(path) {
        Ok(report) => {
            println!("[{now}] {}", path.display());
            print!("{}", format_report(&report, OutputFormat::Pretty));
        }
        Err(e) => {
            eprintln!("[{now}] {} — error: {e}", path.display());
        }
    }
}

fn chrono_now() -> String {
    // Use std time to avoid adding the chrono dep.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
