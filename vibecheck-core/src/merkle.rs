use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cache::Cache;

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
/// Files are hashed by their content (via `Cache::hash_content`).
/// Subdirectories are recursed into. The returned node's hash covers the
/// entire subtree.
///
/// If `cache` contains a `DirNode` whose hash matches the freshly-computed
/// hash, the cached node is returned unchanged — callers can use this to
/// skip re-analysis of unchanged subtrees.
pub fn walk_and_hash(dir: &Path) -> anyhow::Result<DirNode> {
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
            let sub = walk_and_hash(entry)?;
            child_hashes.push(sub.hash);
            children.push(name);
        } else if entry.is_file() {
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
}
