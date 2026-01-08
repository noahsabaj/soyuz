//! File tree data structures and utilities
//!
//! Contains the tree node structure and functions for loading and displaying
//! directory contents in the file explorer.

// Icon matching uses separate arms for semantic clarity even if bodies match
#![allow(clippy::match_same_arms)]
// Nested or-patterns reduce readability for file extension matching
#![allow(clippy::unnested_or_patterns)]
// PathBuf is more convenient than Path for file operations
#![allow(clippy::ptr_arg)]

use std::path::PathBuf;

/// A node in the file tree (flattened for easy rendering)
#[derive(Clone, PartialEq)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
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
            size: metadata.len(),
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

/// Get icon for file/folder based on type
pub fn get_icon(path: &PathBuf, is_dir: bool) -> &'static str {
    if is_dir {
        "" // Folder icon handled by CSS
    } else {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rhai") => "", // Script/code file
            Some("rs") => "",   // Rust file
            Some("toml") => "", // Config file
            Some("md") => "",   // Markdown
            Some("glb") | Some("gltf") | Some("obj") => "", // 3D model
            Some("png") | Some("jpg") | Some("jpeg") | Some("svg") => "", // Image
            Some("json") => "", // JSON
            Some("css") => "",  // CSS
            Some("lock") => "", // Lock file
            _ => "",            // Generic file
        }
    }
}

/// Format file size for display
pub fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}
