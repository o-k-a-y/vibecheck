use std::path::Path;

use anyhow::{Context, Result};
use git2::{Repository, Sort};

use vibecheck_core::report::ModelFamily;

const DEFAULT_LIMIT: usize = 20;

pub fn run(path: &Path, limit: Option<usize>) -> Result<()> {
    if path.is_dir() {
        anyhow::bail!(
            "`vibecheck history` requires a file path, not a directory.\n\
             Try: vibecheck history <file>   e.g. vibecheck history src/main.rs"
        );
    }

    let limit = limit.unwrap_or(DEFAULT_LIMIT);

    let repo = Repository::discover(path)
        .context("not inside a git repository (or no .git found)")?;

    let workdir = repo
        .workdir()
        .context("bare repositories are not supported")?;

    let relative = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .strip_prefix(workdir.canonicalize().unwrap_or_else(|_| workdir.to_path_buf()))
        .context("path is not inside the repository work tree")?
        .to_path_buf();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head().context("no HEAD commit found")?;
    revwalk.set_sorting(Sort::TIME)?;

    println!(
        "Attribution history for {}\n",
        relative.display()
    );
    println!(
        "{:<10}  {:<12}  {:<8}  {:<6}  {}",
        "COMMIT", "DATE", "FAMILY", "CONF", "CHANGE"
    );
    println!("{}", "─".repeat(62));

    let mut prev_family: Option<ModelFamily> = None;
    let mut prev_conf: Option<f64> = None;
    let mut shown = 0;

    for oid_result in revwalk {
        if shown >= limit {
            break;
        }
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let entry = match tree.get_path(&relative) {
            Ok(e) => e,
            Err(_) => continue, // file didn't exist in this commit
        };

        if entry.kind() != Some(git2::ObjectType::Blob) {
            continue;
        }

        let blob = repo.find_blob(entry.id())?;
        let content = match std::str::from_utf8(blob.content()) {
            Ok(s) => s,
            Err(_) => continue, // binary or non-UTF-8 file
        };

        let report = vibecheck_core::analyze(content);
        let family = report.attribution.primary;
        let conf = report.attribution.confidence;

        // Format the timestamp from the committer time.
        let ts = commit.time().seconds();
        let date = format_date(ts);

        // Compute change indicator vs previous entry.
        let change = match (prev_family, prev_conf) {
            (Some(pf), Some(_pc)) if pf != family => {
                format!("⚠ family changed from {pf}")
            }
            (_, Some(pc)) => {
                let delta = conf - pc;
                if delta.abs() < 0.005 {
                    "—".to_string()
                } else if delta > 0.0 {
                    format!("+{:.0}%", delta * 100.0)
                } else {
                    format!("{:.0}%", delta * 100.0)
                }
            }
            _ => "—".to_string(),
        };

        let short_hash = &oid.to_string()[..8];
        println!(
            "{:<10}  {:<12}  {:<8}  {:>5.0}%  {}",
            short_hash,
            date,
            family.to_string(),
            conf * 100.0,
            change,
        );

        prev_family = Some(family);
        prev_conf = Some(conf);
        shown += 1;
    }

    if shown == 0 {
        println!("(no commits found that touched {})", relative.display());
    }

    Ok(())
}

/// Format a Unix timestamp as `YYYY-MM-DD`.
fn format_date(unix_secs: i64) -> String {
    // Hand-rolled to avoid a chrono dependency.
    let secs = unix_secs as u64;
    let days_since_epoch = secs / 86400;

    // Gregorian calendar algorithm.
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}")
}
