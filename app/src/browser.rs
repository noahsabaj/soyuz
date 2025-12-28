//! Explorer panel - VSCode-style expandable file tree

use crate::state::AppState;
use dioxus::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;

/// A node in the file tree (flattened for easy rendering)
#[derive(Clone, PartialEq)]
struct TreeNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    depth: usize,
}

/// Explorer component - VSCode style expandable file tree
#[component]
pub fn AssetBrowser() -> Element {
    let mut state = use_context::<Signal<AppState>>();

    // Tree state - kept local to this component
    let mut nodes: Signal<Vec<TreeNode>> = use_signal(Vec::new);
    let mut expanded: Signal<HashSet<PathBuf>> = use_signal(HashSet::new);
    let mut loading: Signal<HashSet<PathBuf>> = use_signal(HashSet::new);

    // Load root directory when working_dir changes
    let working_dir = state.read().working_dir.clone();
    use_effect(move || {
        let dir = working_dir.clone();
        spawn(async move {
            if let Ok(entries) = load_directory(&dir, 0).await {
                nodes.set(entries);
                // Clear expanded state when root changes
                expanded.write().clear();
            }
        });
    });

    // Toggle expand/collapse for a directory
    let mut toggle_dir = move |path: PathBuf, depth: usize| {
        let is_expanded = expanded.read().contains(&path);

        if is_expanded {
            // Collapse: remove all children (nodes with greater depth that come after this node)
            let mut nodes_write = nodes.write();
            if let Some(idx) = nodes_write.iter().position(|n| n.path == path) {
                let mut remove_count = 0;
                for i in (idx + 1)..nodes_write.len() {
                    if nodes_write[i].depth > depth {
                        remove_count += 1;
                    } else {
                        break;
                    }
                }
                for _ in 0..remove_count {
                    nodes_write.remove(idx + 1);
                }
            }
            expanded.write().remove(&path);
        } else {
            // Expand: load and insert children
            let path_clone = path.clone();
            let child_depth = depth + 1;

            // Mark as loading
            loading.write().insert(path.clone());

            spawn(async move {
                if let Ok(children) = load_directory(&path_clone, child_depth).await {
                    // Insert children after the parent node
                    let mut nodes_write = nodes.write();
                    if let Some(idx) = nodes_write.iter().position(|n| n.path == path_clone) {
                        for (i, child) in children.into_iter().enumerate() {
                            nodes_write.insert(idx + 1 + i, child);
                        }
                    }
                    expanded.write().insert(path_clone.clone());
                }
                loading.write().remove(&path_clone);
            });
        }
    };

    // Open a file in the editor
    let open_file = move |path: PathBuf| {
        spawn(async move {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                state.write().open_file(path, content);
            }
        });
    };

    // Refresh the tree
    let refresh = move |_| {
        let dir = state.read().working_dir.clone();
        spawn(async move {
            if let Ok(entries) = load_directory(&dir, 0).await {
                nodes.set(entries);
                expanded.write().clear();
            }
        });
    };

    // Open folder dialog
    let open_folder = move |_| {
        spawn(async move {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                state.write().working_dir = folder.path().to_path_buf();
            }
        });
    };

    let root_name = state.read().working_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| state.read().working_dir.display().to_string());

    rsx! {
        div { class: "explorer-container",
            // Header
            div { class: "explorer-header",
                span { class: "explorer-title", "Explorer" }
                button {
                    class: "explorer-button",
                    title: "Open folder",
                    onclick: open_folder,
                    "…"
                }
                button {
                    class: "explorer-button",
                    title: "Refresh",
                    onclick: refresh,
                    "↻"
                }
            }

            // Root folder name
            div { class: "explorer-path", "{root_name}" }

            // Tree content
            div { class: "explorer-content",
                for node in nodes.read().iter() {
                    {
                        let path = node.path.clone();
                        let is_dir = node.is_dir;
                        let depth = node.depth;
                        let name = node.name.clone();
                        let size = node.size;
                        let is_expanded = expanded.read().contains(&path);
                        let is_loading = loading.read().contains(&path);
                        let indent = depth * 16;

                        rsx! {
                            div {
                                key: "{path:?}",
                                class: "tree-item",
                                style: "padding-left: {indent}px;",
                                onclick: {
                                    let path = path.clone();
                                    move |_| {
                                        if is_dir {
                                            toggle_dir(path.clone(), depth);
                                        } else {
                                            open_file(path.clone());
                                        }
                                    }
                                },

                                // Expand/collapse arrow (or spacer for files)
                                span {
                                    class: if is_dir {
                                        if is_expanded { "tree-arrow expanded" } else { "tree-arrow" }
                                    } else {
                                        "tree-arrow hidden"
                                    },
                                    if is_loading {
                                        "○"  // Loading indicator
                                    } else if is_dir {
                                        "▶"
                                    }
                                }

                                // File/folder icon
                                span { class: if is_dir { "tree-icon folder" } else { "tree-icon file" },
                                    {get_icon(&path, is_dir)}
                                }

                                // Name
                                span { class: "tree-name", "{name}" }

                                // Size (files only)
                                if !is_dir {
                                    span { class: "tree-size", {format_size(size)} }
                                }
                            }
                        }
                    }
                }

                if nodes.read().is_empty() {
                    div { class: "explorer-empty", "Empty folder" }
                }
            }
        }
    }
}

/// Load directory contents as TreeNodes
async fn load_directory(path: &PathBuf, depth: usize) -> anyhow::Result<Vec<TreeNode>> {
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
fn get_icon(path: &PathBuf, is_dir: bool) -> &'static str {
    if is_dir {
        ""  // Folder icon handled by CSS
    } else {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rhai") => "*",
            Some("rs") => "#",
            Some("toml") => "@",
            Some("md") => "~",
            Some("glb") | Some("gltf") | Some("obj") => "+",
            Some("png") | Some("jpg") | Some("jpeg") | Some("svg") => "%",
            Some("json") => "{}",
            Some("css") => "#",
            Some("lock") => "!",
            _ => "-",
        }
    }
}

/// Format file size for display
fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}
