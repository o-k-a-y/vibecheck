use std::path::Path;

use anyhow::{Context, Result};
use git2::{Repository, Sort};

use vibecheck_core::report::ModelFamily;

const DEFAULT_LIMIT: usize = 20;

pub fn run(path: &Path, limit: Option<usize>) -> Result<()> {
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

    let is_dir = path.is_dir();

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head().context("no HEAD commit found")?;
    revwalk.set_sorting(Sort::TIME)?;

    let label = if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        relative.display().to_string()
    };
    println!("Attribution history for {}\n", label);
    println!(
        "{:<10}  {:<12}  {:<8}  {:<6}  CHANGE",
        "COMMIT", "DATE", "FAMILY", "CONF"
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

        let (family, conf) = if is_dir {
            // Aggregate attribution across all source files in the subtree.
            let subtree = if relative.as_os_str().is_empty() {
                // Root directory — use the commit tree directly.
                tree
            } else {
                match tree.get_path(&relative) {
                    Ok(entry) => match repo.find_tree(entry.id()) {
                        Ok(t) => t,
                        Err(_) => continue,
                    },
                    Err(_) => continue, // dir didn't exist in this commit
                }
            };

            match aggregate_tree(&repo, &subtree) {
                Some(result) => result,
                None => continue, // no analysable source files
            }
        } else {
            // Single file — fetch the blob and analyse it.
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
                Err(_) => continue,
            };
            let report = vibecheck_core::analyze(content);
            (report.attribution.primary, report.attribution.confidence)
        };

        let ts = commit.time().seconds();
        let date = format_date(ts);

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
        println!("(no commits found that touched {})", label);
    }

    Ok(())
}

/// Walk `tree` recursively, analyse every source blob, and return the
/// line-weighted dominant family + confidence. Returns `None` if no
/// supported source files are found in the tree.
fn aggregate_tree(repo: &Repository, tree: &git2::Tree) -> Option<(ModelFamily, f64)> {
    use std::collections::HashMap;

    let mut total_lines = 0usize;
    let mut family_scores: HashMap<ModelFamily, f64> = HashMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |_root, entry| {
        if entry.kind() != Some(git2::ObjectType::Blob) {
            return git2::TreeWalkResult::Ok;
        }
        let name = entry.name().unwrap_or("");
        if !is_source_file(name) {
            return git2::TreeWalkResult::Ok;
        }
        let blob = match repo.find_blob(entry.id()) {
            Ok(b) => b,
            Err(_) => return git2::TreeWalkResult::Ok,
        };
        let content = match std::str::from_utf8(blob.content()) {
            Ok(s) => s,
            Err(_) => return git2::TreeWalkResult::Ok,
        };
        let report = vibecheck_core::analyze(content);
        let lines = content.lines().count().max(1);
        total_lines += lines;
        *family_scores
            .entry(report.attribution.primary)
            .or_insert(0.0) += lines as f64 * report.attribution.confidence;
        git2::TreeWalkResult::Ok
    })
    .ok()?;

    if total_lines == 0 {
        return None;
    }
    let total = total_lines as f64;
    family_scores
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(family, score)| (family, score / total))
}

/// Return `true` for file extensions vibecheck can analyse.
fn is_source_file(name: &str) -> bool {
    matches!(
        std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str()),
        Some("rs" | "py" | "js" | "ts" | "go")
    )
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
