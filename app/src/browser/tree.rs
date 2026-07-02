//! File tree data structures and utilities
//!
//! Contains the tree node structure and functions for loading directory
//! contents in the file explorer.

// PathBuf is more convenient than Path for file operations
#![allow(clippy::ptr_arg)]

use std::path::{Path, PathBuf};

/// A node in the file tree (flattened for easy rendering)
#[derive(Clone, PartialEq)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
}

/// Load directory contents as TreeNodes
pub async fn load_directory(path: &PathBuf, depth: usize) -> anyhow::Result<Vec<TreeNode>> {
    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(path).await?;

    while let Some(entry) = dir.next_entry().await? {
        let metadata = entry.metadata().await?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        entries.push(TreeNode {
            name,
            path: entry.path(),
            is_dir: metadata.is_dir(),
            depth,
        });
    }

    // Sort: directories first, then by name (case-insensitive)
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

/// Replace the descendant rows of `parent_path` in the flat tree with
/// `new_children`.
///
/// The flat tree stores an expanded directory's contents as the contiguous run
/// of nodes after the directory whose depth is greater than the directory's
/// own. This splices that run out and inserts `new_children` in its place, so
/// the same helper covers expanding a collapsed directory (existing run is
/// empty), collapsing an expanded one (`new_children` is empty), and
/// refreshing its listing in place. Leaves `nodes` untouched and returns
/// `false` when `parent_path` is not in the tree.
pub fn splice_dir_children(
    nodes: &mut Vec<TreeNode>,
    parent_path: &Path,
    new_children: Vec<TreeNode>,
) -> bool {
    let Some(parent_idx) = nodes.iter().position(|n| n.path == parent_path) else {
        return false;
    };
    let parent_depth = nodes[parent_idx].depth;
    let descendant_count = nodes[parent_idx + 1..]
        .iter()
        .take_while(|n| n.depth > parent_depth)
        .count();
    nodes.splice(
        parent_idx + 1..parent_idx + 1 + descendant_count,
        new_children,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, is_dir: bool, depth: usize) -> TreeNode {
        TreeNode {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            path: PathBuf::from(path),
            is_dir,
            depth,
        }
    }

    fn paths(nodes: &[TreeNode]) -> Vec<&str> {
        nodes
            .iter()
            .map(|n| n.path.to_str().unwrap_or_default())
            .collect()
    }

    #[test]
    fn splice_expands_a_collapsed_directory() {
        let mut nodes = vec![node("/ws/a", true, 0), node("/ws/z.rhai", false, 0)];
        let inserted = splice_dir_children(
            &mut nodes,
            Path::new("/ws/a"),
            vec![node("/ws/a/b", true, 1), node("/ws/a/c.rhai", false, 1)],
        );
        assert!(inserted);
        assert_eq!(
            paths(&nodes),
            ["/ws/a", "/ws/a/b", "/ws/a/c.rhai", "/ws/z.rhai"]
        );
    }

    #[test]
    fn splice_with_empty_children_collapses_all_descendants() {
        let mut nodes = vec![
            node("/ws/a", true, 0),
            node("/ws/a/b", true, 1),
            node("/ws/a/b/d.rhai", false, 2),
            node("/ws/z.rhai", false, 0),
        ];
        assert!(splice_dir_children(
            &mut nodes,
            Path::new("/ws/a"),
            Vec::new()
        ));
        assert_eq!(paths(&nodes), ["/ws/a", "/ws/z.rhai"]);
    }

    #[test]
    fn splice_refreshes_children_in_place() {
        let mut nodes = vec![
            node("/ws/a", true, 0),
            node("/ws/a/old.rhai", false, 1),
            node("/ws/z.rhai", false, 0),
        ];
        assert!(splice_dir_children(
            &mut nodes,
            Path::new("/ws/a"),
            vec![node("/ws/a/new.rhai", false, 1)],
        ));
        assert_eq!(paths(&nodes), ["/ws/a", "/ws/a/new.rhai", "/ws/z.rhai"]);
    }

    #[test]
    fn splice_is_a_noop_when_parent_is_missing() {
        let mut nodes = vec![node("/ws/a", true, 0)];
        assert!(!splice_dir_children(
            &mut nodes,
            Path::new("/ws/missing"),
            vec![node("/ws/missing/x.rhai", false, 1)],
        ));
        assert_eq!(paths(&nodes), ["/ws/a"]);
    }
}
