//! File tree data structures and utilities
//!
//! Contains the tree node structure and functions for loading directory
//! contents in the file explorer.

// PathBuf is more convenient than Path for file operations
#![allow(clippy::ptr_arg)]

use std::path::PathBuf;

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
