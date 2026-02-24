use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cache::Cache;
use crate::ignore_rules::{AllowAll, IgnoreRules};

/// A node in the Merkle hash tree representing a directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirNode {
    /// SHA-256 of the sorted concatenation of child hashes.
    pub hash: [u8; 32],
    /// Sorted child paths (files and subdirs) relative to the directory.
    pub children: Vec<String>,
}

/// Compute the Merkle hash for a directory from its children's hashes.
/// Children must be sorted before calling this function.
pub fn compute_dir_hash(child_hashes: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for h in child_hashes {
        hasher.update(h);
    }
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Walk a directory, compute its Merkle hash, and return the `DirNode`.
///
/// Equivalent to [`walk_and_hash_with`] with [`AllowAll`] — all paths are
/// included in the hash.  Use [`walk_and_hash_with`] when ignored files should
/// be excluded from the hash (so that changes to ignored files do not trigger
/// re-analysis).
pub fn walk_and_hash(dir: &Path) -> anyhow::Result<DirNode> {
    walk_and_hash_with(dir, &AllowAll)
}

/// Walk a directory, compute its Merkle hash, and return the `DirNode`,
/// skipping any paths that `ignore` marks as ignored.
///
/// Files are hashed by their content (via `Cache::hash_content`).
/// Subdirectories are recursed into unless [`IgnoreRules::is_ignored_dir`]
/// returns `true`.  The returned node's hash covers the entire visible subtree.
pub fn walk_and_hash_with(dir: &Path, ignore: &dyn IgnoreRules) -> anyhow::Result<DirNode> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    entries.sort();

    let mut child_hashes: Vec<[u8; 32]> = Vec::new();
    let mut children: Vec<String> = Vec::new();

    for entry in &entries {
        let name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if entry.is_dir() {
            if ignore.is_ignored_dir(entry) {
                continue;
            }
            let sub = walk_and_hash_with(entry, ignore)?;
            child_hashes.push(sub.hash);
            children.push(name);
        } else if entry.is_file() {
            if ignore.is_ignored(entry) {
                continue;
            }
            let bytes = std::fs::read(entry)?;
            let h = Cache::hash_content(&bytes);
            child_hashes.push(h);
            children.push(name);
        }
    }

    let hash = compute_dir_hash(&child_hashes);
    Ok(DirNode { hash, children })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ignore_rules::PatternIgnore;

    #[test]
    fn empty_dir_hash_is_deterministic() {
        let h1 = compute_dir_hash(&[]);
        let h2 = compute_dir_hash(&[]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_contents_produce_different_hashes() {
        let a: [u8; 32] = [1u8; 32];
        let b: [u8; 32] = [2u8; 32];
        assert_ne!(compute_dir_hash(&[a]), compute_dir_hash(&[b]));
    }

    #[test]
    fn order_matters() {
        let a: [u8; 32] = [1u8; 32];
        let b: [u8; 32] = [2u8; 32];
        assert_ne!(compute_dir_hash(&[a, b]), compute_dir_hash(&[b, a]));
    }

    #[test]
    fn walk_and_hash_empty_dir_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let h1 = walk_and_hash(dir.path()).unwrap().hash;
        let h2 = walk_and_hash(dir.path()).unwrap().hash;
        assert_eq!(h1, h2);
    }

    #[test]
    fn walk_and_hash_reflects_file_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), b"fn foo() {}").unwrap();

        let h_with = walk_and_hash(dir.path()).unwrap().hash;

        // Change the file — hash must change.
        std::fs::write(dir.path().join("a.rs"), b"fn bar() {}").unwrap();
        let h_changed = walk_and_hash(dir.path()).unwrap().hash;
        assert_ne!(h_with, h_changed);
    }

    #[test]
    fn walk_and_hash_children_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("z.rs"), b"").unwrap();
        std::fs::write(dir.path().join("a.rs"), b"").unwrap();
        std::fs::write(dir.path().join("m.rs"), b"").unwrap();

        let node = walk_and_hash(dir.path()).unwrap();
        let mut expected = node.children.clone();
        expected.sort();
        assert_eq!(node.children, expected);
    }

    #[test]
    fn walk_and_hash_subdirectory_changes_propagate() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("x.py"), b"x = 1").unwrap();

        let h_before = walk_and_hash(dir.path()).unwrap().hash;

        std::fs::write(sub.join("x.py"), b"x = 2").unwrap();
        let h_after = walk_and_hash(dir.path()).unwrap().hash;

        assert_ne!(h_before, h_after,
            "parent hash must change when a file deep in the tree changes");
    }

    #[test]
    fn walk_and_hash_with_ignores_matched_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), b"fn foo() {}").unwrap();
        std::fs::write(dir.path().join("vendor.rs"), b"fn bar() {}").unwrap();

        let ignore = PatternIgnore(vec!["vendor".into()]);
        let node_with = walk_and_hash_with(dir.path(), &ignore).unwrap();
        let node_without = walk_and_hash(dir.path()).unwrap();

        // Hashes differ because vendor.rs is excluded.
        assert_ne!(node_with.hash, node_without.hash);
        assert!(!node_with.children.iter().any(|c| c.contains("vendor")));
    }

    #[test]
    fn walk_and_hash_with_ignored_dir_does_not_affect_hash() {
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        std::fs::create_dir(&vendor).unwrap();
        std::fs::write(vendor.join("lib.rs"), b"// vendored").unwrap();

        let ignore = PatternIgnore(vec!["vendor".into()]);
        let h_ignored = walk_and_hash_with(dir.path(), &ignore).unwrap().hash;

        // Changing content inside the ignored dir must NOT change the hash.
        std::fs::write(vendor.join("lib.rs"), b"// changed").unwrap();
        let h_after = walk_and_hash_with(dir.path(), &ignore).unwrap().hash;

        assert_eq!(h_ignored, h_after);
    }
}
