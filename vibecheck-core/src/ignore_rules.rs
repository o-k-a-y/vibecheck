//! Path-based ignore rules for vibecheck directory traversal.
//!
//! The central abstraction is the [`IgnoreRules`] trait — all traversal
//! internals depend only on the trait, enabling full dependency injection
//! for testing and alternative config formats.
//!
//! # Production use
//! [`IgnoreConfig`] is the production implementation.  It discovers and
//! parses a `.vibecheck` TOML file (walking upward to the git root) and
//! honours `.gitignore` by default.
//!
//! # Testing / DI
//! [`AllowAll`] and [`PatternIgnore`] are lightweight test doubles that
//! implement the same trait with no filesystem access.

use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Seam for dependency injection across all traversal entry points.
///
/// Implement this trait to supply custom ignore logic — different config
/// formats, in-memory rules for tests, etc. — without touching the traversal
/// code.
pub trait IgnoreRules: Send + Sync {
    /// Return `true` if this path should be excluded from scanning.
    fn is_ignored(&self, path: &Path) -> bool;

    /// Return `true` if this *directory* should be skipped entirely,
    /// avoiding descent into it.  Defaults to [`is_ignored`].
    ///
    /// Override for performance when a cheap directory check is available
    /// (e.g. matching against a pattern list before stat'ing children).
    fn is_ignored_dir(&self, path: &Path) -> bool {
        self.is_ignored(path)
    }
}

// ---------------------------------------------------------------------------
// AllowAll — permits everything (zero-cost baseline / test default)
// ---------------------------------------------------------------------------

/// Permits all paths.  Zero-cost; matches the pre-ignore-system behaviour.
pub struct AllowAll;

impl IgnoreRules for AllowAll {
    #[inline]
    fn is_ignored(&self, _path: &Path) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// PatternIgnore — substring-match test double
// ---------------------------------------------------------------------------

/// Ignores paths whose display string contains any of the given substrings.
///
/// Designed for unit tests.  Not a full glob engine — use [`IgnoreConfig`]
/// for production pattern matching.
///
/// ```
/// use vibecheck_core::ignore_rules::{IgnoreRules, PatternIgnore};
/// use std::path::Path;
///
/// let rules = PatternIgnore(vec!["vendor".into(), "dist".into()]);
/// assert!(rules.is_ignored(Path::new("/project/vendor/lib.rs")));
/// assert!(!rules.is_ignored(Path::new("/project/src/main.rs")));
/// ```
pub struct PatternIgnore(pub Vec<String>);

impl IgnoreRules for PatternIgnore {
    fn is_ignored(&self, path: &Path) -> bool {
        let s = path.to_string_lossy();
        self.0.iter().any(|p| s.contains(p.as_str()))
    }
}

// ---------------------------------------------------------------------------
// TOML config types (private)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    ignore: IgnoreSection,
}

#[derive(serde::Deserialize)]
struct IgnoreSection {
    /// Additional gitignore-style patterns to exclude.
    #[serde(default)]
    patterns: Vec<String>,
    /// Respect the project's `.gitignore` (default: `true`).
    #[serde(default = "bool_true")]
    use_gitignore: bool,
    /// Respect the global gitignore (`~/.gitignore_global`, etc.) (default: `true`).
    #[serde(default = "bool_true")]
    use_global_gitignore: bool,
}

impl Default for IgnoreSection {
    fn default() -> Self {
        Self {
            patterns: vec![],
            use_gitignore: true,
            use_global_gitignore: true,
        }
    }
}

fn bool_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// IgnoreConfig — production implementation
// ---------------------------------------------------------------------------

/// Full implementation: reads `.vibecheck` TOML + respects `.gitignore`.
///
/// # Config file format (`.vibecheck`)
///
/// ```toml
/// [ignore]
/// # Extra patterns (gitignore glob syntax), additive on top of .gitignore.
/// patterns = ["vendor/", "dist/", "*.min.js"]
///
/// # Set to false to disable .gitignore integration (default: true).
/// use_gitignore = true
///
/// # Set to false to disable the global gitignore (default: true).
/// use_global_gitignore = true
/// ```
///
/// # Discovery
/// [`IgnoreConfig::load`] walks upward from the given path looking for a
/// `.vibecheck` file or a `.git` directory, using the first match as the
/// config root.  Falls back to defaults when neither is found.
pub struct IgnoreConfig {
    root: PathBuf,
    pub(crate) use_gitignore: bool,
    pub(crate) use_global_gitignore: bool,
    /// Combined matcher: root `.gitignore` rules + extra `.vibecheck` patterns.
    combined: Gitignore,
    /// Extra patterns only (used by `is_extra_ignored` for walker secondary filter).
    extra: Gitignore,
}

impl IgnoreConfig {
    /// Discover and load the nearest `.vibecheck` config, walking upward from
    /// `start` to the git root.  Silently uses defaults if none is found or
    /// the file cannot be parsed.
    pub fn load(start: &Path) -> Self {
        let root = find_config_root(start);
        Self::load_from_root(root)
    }

    /// Load from an explicit config file path.
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        let f: ConfigFile = toml::from_str(&s)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))?;
        let root = path.parent().unwrap_or(path).to_path_buf();
        Ok(Self::from_section(root, f.ignore))
    }

    /// Build an [`ignore::WalkBuilder`] pre-configured with gitignore settings.
    ///
    /// The walker handles `.gitignore` files across the entire tree natively
    /// (including nested `.gitignore` files in subdirectories).  After
    /// receiving each entry, call [`is_extra_ignored`] to also apply any
    /// additional patterns declared in `.vibecheck`.
    pub fn build_walker(&self, path: &Path) -> ignore::WalkBuilder {
        let mut b = ignore::WalkBuilder::new(path);
        b.git_ignore(self.use_gitignore)
            .git_global(self.use_global_gitignore)
            .git_exclude(self.use_gitignore)
            .hidden(false);
        b
    }

    /// Returns `true` if `path` matches any of the extra patterns declared in
    /// `.vibecheck` (gitignore rules are *not* checked here — the walker
    /// handles those natively).  Use as a secondary filter on walker entries.
    pub fn is_extra_ignored(&self, path: &Path) -> bool {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        self.extra
            .matched_path_or_any_parents(rel, path.is_dir())
            .is_ignore()
    }

    // -- internals -----------------------------------------------------------

    fn load_from_root(root: PathBuf) -> Self {
        let cfg_path = root.join(".vibecheck");
        let section = if cfg_path.is_file() {
            std::fs::read_to_string(&cfg_path)
                .ok()
                .and_then(|s| toml::from_str::<ConfigFile>(&s).ok())
                .map(|f| f.ignore)
                .unwrap_or_else(|| {
                    eprintln!("vibecheck: warning: failed to parse .vibecheck; using defaults");
                    IgnoreSection::default()
                })
        } else {
            IgnoreSection::default()
        };
        Self::from_section(root, section)
    }

    fn from_section(root: PathBuf, section: IgnoreSection) -> Self {
        let combined = build_combined(&root, &section.patterns, section.use_gitignore);
        let extra = build_extra(&root, &section.patterns);
        Self {
            root,
            use_gitignore: section.use_gitignore,
            use_global_gitignore: section.use_global_gitignore,
            combined,
            extra,
        }
    }
}

impl IgnoreRules for IgnoreConfig {
    /// Returns `true` if `path` is excluded by either `.gitignore` rules or
    /// extra patterns from `.vibecheck`.
    ///
    /// Uses `matched_path_or_any_parents` so that a file inside an ignored
    /// directory (e.g. `vendor/lib.rs` when `vendor/` is in the pattern list)
    /// is correctly reported as ignored.
    fn is_ignored(&self, path: &Path) -> bool {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        self.combined
            .matched_path_or_any_parents(rel, path.is_dir())
            .is_ignore()
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Walk upward from `start` (normalised to a directory) looking for a
/// `.vibecheck` file or a `.git` directory.  Returns the first match, or
/// `start` itself if neither is found before the filesystem root.
fn find_config_root(start: &Path) -> PathBuf {
    let dir = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };

    let mut current = dir;
    loop {
        if current.join(".vibecheck").is_file() || current.join(".git").is_dir() {
            return current.to_path_buf();
        }
        match current.parent() {
            Some(p) => current = p,
            None => return dir.to_path_buf(),
        }
    }
}

// ---------------------------------------------------------------------------
// Matcher builders
// ---------------------------------------------------------------------------

/// Build a `Gitignore` matcher that combines the root `.gitignore` (when
/// `use_gitignore` is `true`) with the extra patterns from `.vibecheck`.
fn build_combined(root: &Path, patterns: &[String], use_gitignore: bool) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    if use_gitignore {
        let gi = root.join(".gitignore");
        if gi.is_file() {
            let _ = b.add(gi);
        }
    }
    for p in patterns {
        let _ = b.add_line(None, p);
    }
    b.build().unwrap_or(Gitignore::empty())
}

/// Build a `Gitignore` matcher for the extra patterns only.
fn build_extra(root: &Path, patterns: &[String]) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    for p in patterns {
        let _ = b.add_line(None, p);
    }
    b.build().unwrap_or(Gitignore::empty())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn allow_all_never_ignores() {
        let r = AllowAll;
        assert!(!r.is_ignored(Path::new("/any/path/file.rs")));
        assert!(!r.is_ignored_dir(Path::new("/any/dir")));
    }

    #[test]
    fn pattern_ignore_matches_substring() {
        let r = PatternIgnore(vec!["vendor".into(), "dist".into()]);
        assert!(r.is_ignored(Path::new("/project/vendor/lib.rs")));
        assert!(r.is_ignored(Path::new("/project/dist/bundle.js")));
        assert!(!r.is_ignored(Path::new("/project/src/main.rs")));
    }

    #[test]
    fn pattern_ignore_dir_delegates_to_is_ignored() {
        let r = PatternIgnore(vec!["node_modules".into()]);
        assert!(r.is_ignored_dir(Path::new("/project/node_modules")));
        assert!(!r.is_ignored_dir(Path::new("/project/src")));
    }

    #[test]
    fn ignore_config_load_defaults_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = IgnoreConfig::load(dir.path());
        // Defaults: use_gitignore = true, no extra patterns — nothing ignored.
        assert!(!cfg.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn ignore_config_parses_patterns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".vibecheck"),
            "[ignore]\npatterns = [\"vendor/\"]\n",
        )
        .unwrap();
        let cfg = IgnoreConfig::load(dir.path());
        assert!(cfg.is_ignored(&dir.path().join("vendor/lib.rs")));
        assert!(!cfg.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn ignore_config_from_file_error_on_bad_toml() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.toml");
        std::fs::write(&bad, "not valid toml ][[[").unwrap();
        assert!(IgnoreConfig::from_file(&bad).is_err());
    }

    #[test]
    fn ignore_config_from_file_valid() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join(".vibecheck");
        std::fs::write(&f, "[ignore]\npatterns = [\"dist/\"]\n").unwrap();
        let cfg = IgnoreConfig::from_file(&f).unwrap();
        assert!(cfg.is_ignored(&dir.path().join("dist/main.js")));
        assert!(!cfg.is_ignored(&dir.path().join("src/main.rs")));
    }

    #[test]
    fn find_config_root_stops_at_git() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        // Starting from a subdirectory should find the .git root.
        let root = find_config_root(&sub);
        assert_eq!(root, dir.path());
    }

    #[test]
    fn find_config_root_stops_at_vibecheck_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".vibecheck"), "").unwrap();
        let sub = dir.path().join("deep/nested");
        std::fs::create_dir_all(&sub).unwrap();
        let root = find_config_root(&sub);
        assert_eq!(root, dir.path());
    }
}
