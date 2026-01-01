//! Explorer panel - VSCode-style expandable file tree

// Icon matching uses separate arms for semantic clarity even if bodies match
#![allow(clippy::match_same_arms)]
// Nested or-patterns reduce readability for file extension matching
#![allow(clippy::unnested_or_patterns)]
// map_or_else is less readable for file path operations
#![allow(clippy::map_unwrap_or)]
// PathBuf is more convenient than Path for file operations
#![allow(clippy::ptr_arg)]

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

/// What type of item is being created
#[derive(Clone, Copy, PartialEq)]
enum CreatingType {
    File,
    Folder,
}

/// Context menu state
#[derive(Clone, PartialEq, Default)]
struct ContextMenuState {
    visible: bool,
    x: i32,
    y: i32,
    target_path: Option<PathBuf>,
    is_dir: bool,
}

/// Rename state
#[derive(Clone, PartialEq, Default)]
struct RenameState {
    active: bool,
    path: Option<PathBuf>,
    new_name: String,
}

/// Empty state component shown when no folder is opened
#[component]
fn EmptyExplorerState() -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let open_folder = move |_| {
        spawn(async move {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                state.write().open_folder(folder.path().to_path_buf());
            }
        });
    };

    rsx! {
        div { class: "explorer-container",
            div { class: "explorer-header",
                span { class: "explorer-title", "Explorer" }
            }
            div { class: "explorer-empty-state",
                p { class: "explorer-empty-title", "No Folder Opened" }
                p { class: "explorer-empty-subtitle",
                    "You have not yet opened a folder."
                }
                button {
                    class: "explorer-empty-button",
                    onclick: open_folder,
                    "Open Folder"
                }
            }
        }
    }
}

/// Explorer component - VSCode style expandable file tree
#[component]
pub fn AssetBrowser() -> Element {
    let mut state = use_context::<Signal<AppState>>();

    // Check if workspace is set - if not, show empty state
    let Some(workspace_dir) = state.read().workspace.clone() else {
        return rsx! { EmptyExplorerState {} };
    };

    // Tree state - kept local to this component
    let mut nodes: Signal<Vec<TreeNode>> = use_signal(Vec::new);
    let mut expanded: Signal<HashSet<PathBuf>> = use_signal(HashSet::new);
    let mut loading: Signal<HashSet<PathBuf>> = use_signal(HashSet::new);

    // State for inline creation input
    let mut creating: Signal<Option<CreatingType>> = use_signal(|| None);
    let mut creating_parent: Signal<Option<PathBuf>> = use_signal(|| None); // Target folder for new file/folder
    let mut input_value: Signal<String> = use_signal(String::new);

    // Context menu state
    let mut context_menu: Signal<ContextMenuState> = use_signal(ContextMenuState::default);

    // Rename state
    let mut rename_state: Signal<RenameState> = use_signal(RenameState::default);

    // Note: Drag-drop is handled entirely via JavaScript for Linux/WebKitGTK compatibility
    // See the use_effect below that sets up JS event listeners and handles file moves

    // Load root directory when workspace changes
    let workspace_for_effect = workspace_dir.clone();
    use_effect(move || {
        let dir = workspace_for_effect.clone();
        spawn(async move {
            if let Ok(entries) = load_directory(&dir, 0).await {
                nodes.set(entries);
                // Clear expanded state when root changes
                expanded.write().clear();
            }
        });
    });

    // JavaScript-based drag-drop setup (workaround for WebKitGTK bug on Linux)
    // Native ondragover/ondrop don't fire properly, so we use JS event listeners
    use_effect(move || {
        // Set up JavaScript drag-drop handlers and communication channel
        let eval = document::eval(
            r#"
            // Drag-drop state
            let dragSource = null;
            let currentDropTarget = null;

            // Set up event delegation on the explorer content
            const setupDragDrop = () => {
                const container = document.querySelector('.explorer-content');
                if (!container) {
                    // Component not mounted yet, retry
                    setTimeout(setupDragDrop, 100);
                    return;
                }

                // Remove existing listeners to avoid duplicates
                container.removeEventListener('dragstart', handleDragStart, true);
                container.removeEventListener('dragover', handleDragOver, true);
                container.removeEventListener('dragleave', handleDragLeave, true);
                container.removeEventListener('drop', handleDrop, true);
                container.removeEventListener('dragend', handleDragEnd, true);

                // Add listeners using capture phase for tree items
                container.addEventListener('dragstart', handleDragStart, true);
                container.addEventListener('dragover', handleDragOver, true);
                container.addEventListener('dragleave', handleDragLeave, true);
                container.addEventListener('drop', handleDrop, true);
                container.addEventListener('dragend', handleDragEnd, true);
            };

            function handleDragStart(e) {
                const treeItem = e.target.closest('.tree-item');
                if (!treeItem) return;

                const path = treeItem.getAttribute('data-path');
                if (!path) return;

                dragSource = path;
                // Critical: setData is required for drag-drop to work on Linux/WebKitGTK
                e.dataTransfer.setData('text/plain', path);
                e.dataTransfer.effectAllowed = 'move';

                // Add dragging class for visual feedback
                treeItem.classList.add('dragging');
            }

            function handleDragOver(e) {
                if (!dragSource) return;

                const treeItem = e.target.closest('.tree-item');
                const container = e.target.closest('.explorer-content');

                // Allow drop on folders
                if (treeItem) {
                    const isDir = treeItem.getAttribute('data-is-dir') === 'true';
                    if (!isDir) return;

                    e.preventDefault();
                    e.dataTransfer.dropEffect = 'move';

                    // Update visual drop target
                    const path = treeItem.getAttribute('data-path');
                    if (currentDropTarget !== path) {
                        clearDropTargetHighlight();
                        treeItem.classList.add('drop-target');
                        currentDropTarget = path;
                    }
                }
                // Allow drop on empty space (moves to root)
                else if (container && e.target === container) {
                    e.preventDefault();
                    e.dataTransfer.dropEffect = 'move';

                    // Highlight the container as drop target
                    if (currentDropTarget !== '__ROOT__') {
                        clearDropTargetHighlight();
                        container.classList.add('drop-target-root');
                        currentDropTarget = '__ROOT__';
                    }
                }
            }

            function clearDropTargetHighlight() {
                document.querySelectorAll('.tree-item.drop-target').forEach(el => {
                    el.classList.remove('drop-target');
                });
                document.querySelectorAll('.explorer-content.drop-target-root').forEach(el => {
                    el.classList.remove('drop-target-root');
                });
            }

            function handleDragLeave(e) {
                const treeItem = e.target.closest('.tree-item');
                const container = e.target.closest('.explorer-content');

                if (treeItem) {
                    // Only clear if we're leaving the actual item (not moving to a child)
                    const relatedTarget = e.relatedTarget;
                    if (!relatedTarget || !treeItem.contains(relatedTarget)) {
                        treeItem.classList.remove('drop-target');
                        if (currentDropTarget === treeItem.getAttribute('data-path')) {
                            currentDropTarget = null;
                        }
                    }
                } else if (container && e.target === container) {
                    // Leaving the container empty space
                    const relatedTarget = e.relatedTarget;
                    if (!relatedTarget || !container.contains(relatedTarget)) {
                        container.classList.remove('drop-target-root');
                        if (currentDropTarget === '__ROOT__') {
                            currentDropTarget = null;
                        }
                    }
                }
            }

            function handleDrop(e) {
                e.preventDefault();
                e.stopPropagation();

                if (!dragSource) {
                    handleDragEnd(e);
                    return;
                }

                const treeItem = e.target.closest('.tree-item');
                const container = e.target.closest('.explorer-content');

                // Drop on a folder
                if (treeItem) {
                    const isDir = treeItem.getAttribute('data-is-dir') === 'true';
                    const targetPath = treeItem.getAttribute('data-path');

                    if (isDir && targetPath && dragSource !== targetPath) {
                        dioxus.send({ source: dragSource, target: targetPath });
                    }
                }
                // Drop on empty space - move to workspace root
                else if (container) {
                    const rootPath = container.getAttribute('data-root');
                    if (rootPath && dragSource !== rootPath) {
                        dioxus.send({ source: dragSource, target: rootPath });
                    }
                }

                // Clean up
                handleDragEnd(e);
            }

            function handleDragEnd(e) {
                // Remove all visual states
                document.querySelectorAll('.tree-item.dragging').forEach(el => {
                    el.classList.remove('dragging');
                });
                clearDropTargetHighlight();

                dragSource = null;
                currentDropTarget = null;
            }

            // Initial setup
            setupDragDrop();

            // Re-setup when DOM changes (for dynamic content)
            const observer = new MutationObserver(() => {
                setupDragDrop();
            });
            observer.observe(document.body, { childList: true, subtree: true });
            "#,
        );

        // Listen for drop events from JavaScript
        spawn(async move {
            let mut eval = eval;
            loop {
                match eval.recv::<serde_json::Value>().await {
                    Ok(msg) => {
                        if let (Some(source), Some(target)) = (
                            msg.get("source").and_then(|v| v.as_str()),
                            msg.get("target").and_then(|v| v.as_str()),
                        ) {
                            let source_path = PathBuf::from(source);
                            let target_path = PathBuf::from(target);

                            // Validate: don't drop into self or current parent
                            if source_path.parent() != Some(target_path.as_path()) {
                                // Perform the file move
                                if let Some(file_name) = source_path.file_name() {
                                    let new_path = target_path.join(file_name);
                                    if tokio::fs::rename(&source_path, &new_path).await.is_ok() {
                                        // Refresh tree while preserving expanded folders
                                        let workspace = state.read().workspace.clone();
                                        if let Some(dir) = workspace
                                            && let Ok(mut all_nodes) = load_directory(&dir, 0).await
                                        {
                                            let mut expanded_paths: Vec<_> =
                                                expanded.read().iter().cloned().collect();
                                            expanded_paths.sort_by_key(|p| p.components().count());
                                            for exp_path in expanded_paths {
                                                if let Some(parent_idx) = all_nodes
                                                    .iter()
                                                    .position(|n| n.path == exp_path && n.is_dir)
                                                {
                                                    let parent_depth = all_nodes[parent_idx].depth;
                                                    if let Ok(children) =
                                                        load_directory(&exp_path, parent_depth + 1)
                                                            .await
                                                    {
                                                        for (i, child) in
                                                            children.into_iter().enumerate()
                                                        {
                                                            all_nodes
                                                                .insert(parent_idx + 1 + i, child);
                                                        }
                                                    }
                                                }
                                            }
                                            nodes.set(all_nodes);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Channel closed, stop listening
                        break;
                    }
                }
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
    let refresh = {
        let workspace_dir = workspace_dir.clone();
        move |_| {
            let dir = workspace_dir.clone();
            spawn(async move {
                if let Ok(entries) = load_directory(&dir, 0).await {
                    nodes.set(entries);
                    expanded.write().clear();
                }
            });
        }
    };

    // Collapse all expanded folders
    let collapse_all = move |_| {
        let workspace = state.read().workspace.clone();
        spawn(async move {
            if let Some(dir) = workspace
                && let Ok(entries) = load_directory(&dir, 0).await
            {
                nodes.set(entries);
                expanded.write().clear();
            }
        });
    };

    // Start creating a new file (at workspace root)
    let start_new_file = {
        let workspace_dir = workspace_dir.clone();
        move |_| {
            creating.set(Some(CreatingType::File));
            creating_parent.set(Some(workspace_dir.clone()));
            input_value.set(String::new());
        }
    };

    // Start creating a new folder (at workspace root)
    let start_new_folder = {
        let workspace_dir = workspace_dir.clone();
        move |_| {
            creating.set(Some(CreatingType::Folder));
            creating_parent.set(Some(workspace_dir.clone()));
            input_value.set(String::new());
        }
    };

    // Close context menu
    let close_context_menu = move |_| {
        context_menu.set(ContextMenuState::default());
    };

    // Context menu: Copy Path
    let ctx_copy_path = move |_| {
        if let Some(path) = context_menu.read().target_path.clone() {
            let path_str = path.to_string_lossy().to_string();
            spawn(async move {
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(window) = web_sys::window() {
                        if let Some(clipboard) = window.navigator().clipboard() {
                            let _ = clipboard.write_text(&path_str);
                        }
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    use arboard::Clipboard;
                    if let Ok(mut clipboard) = Clipboard::new() {
                        let _ = clipboard.set_text(&path_str);
                    }
                }
            });
        }
        context_menu.set(ContextMenuState::default());
    };

    // Context menu: Rename
    let ctx_rename = move |_| {
        if let Some(path) = context_menu.read().target_path.clone() {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            rename_state.set(RenameState {
                active: true,
                path: Some(path),
                new_name: name,
            });
        }
        context_menu.set(ContextMenuState::default());
    };

    // Context menu: Delete
    let ctx_delete = move |_| {
        if let Some(path) = context_menu.read().target_path.clone() {
            let is_dir = context_menu.read().is_dir;
            spawn(async move {
                let success = if is_dir {
                    tokio::fs::remove_dir_all(&path).await.is_ok()
                } else {
                    tokio::fs::remove_file(&path).await.is_ok()
                };

                if success {
                    // Close any open tabs for the deleted file/directory
                    state.write().close_tabs_for_deleted_path(&path, is_dir);

                    // Refresh tree while preserving expanded folders
                    let workspace = state.read().workspace.clone();
                    if let Some(dir) = workspace
                        && let Ok(mut all_nodes) = load_directory(&dir, 0).await
                    {
                        let mut expanded_paths: Vec<_> = expanded.read().iter().cloned().collect();
                        expanded_paths.sort_by_key(|p| p.components().count());
                        for exp_path in expanded_paths {
                            if let Some(parent_idx) = all_nodes.iter().position(|n| n.path == exp_path && n.is_dir) {
                                let parent_depth = all_nodes[parent_idx].depth;
                                if let Ok(children) = load_directory(&exp_path, parent_depth + 1).await {
                                    for (i, child) in children.into_iter().enumerate() {
                                        all_nodes.insert(parent_idx + 1 + i, child);
                                    }
                                }
                            }
                        }
                        nodes.set(all_nodes);
                    }
                }
            });
        }
        context_menu.set(ContextMenuState::default());
    };

    // Context menu: New File (folders only)
    let ctx_new_file = move |_| {
        if let Some(parent_path) = context_menu.read().target_path.clone() {
            // Expand the folder if not already expanded
            if !expanded.read().contains(&parent_path) {
                let depth = nodes.read().iter().find(|n| n.path == parent_path).map(|n| n.depth).unwrap_or(0);
                toggle_dir(parent_path.clone(), depth);
            }
            // Start inline creation at the folder location
            creating.set(Some(CreatingType::File));
            creating_parent.set(Some(parent_path));
            input_value.set(String::new());
        }
        context_menu.set(ContextMenuState::default());
    };

    // Context menu: New Folder (folders only)
    let ctx_new_folder = move |_| {
        if let Some(parent_path) = context_menu.read().target_path.clone() {
            // Expand the folder if not already expanded
            if !expanded.read().contains(&parent_path) {
                let depth = nodes.read().iter().find(|n| n.path == parent_path).map(|n| n.depth).unwrap_or(0);
                toggle_dir(parent_path.clone(), depth);
            }
            // Start inline creation at the folder location
            creating.set(Some(CreatingType::Folder));
            creating_parent.set(Some(parent_path));
            input_value.set(String::new());
        }
        context_menu.set(ContextMenuState::default());
    };

    // Confirm rename
    let mut confirm_rename = move |()| {
        let rename = rename_state.read().clone();
        if !rename.active || rename.new_name.is_empty() {
            rename_state.set(RenameState::default());
            return;
        }

        if let Some(old_path) = rename.path {
            let new_name = rename.new_name.clone();
            let new_path = old_path.parent().map(|p| p.join(&new_name));

            if let Some(new_path) = new_path {
                spawn(async move {
                    if tokio::fs::rename(&old_path, &new_path).await.is_ok() {
                        // Refresh tree while preserving expanded folders
                        let workspace = state.read().workspace.clone();
                        if let Some(dir) = workspace
                            && let Ok(mut all_nodes) = load_directory(&dir, 0).await
                        {
                            let mut expanded_paths: Vec<_> = expanded.read().iter().cloned().collect();
                            expanded_paths.sort_by_key(|p| p.components().count());
                            for exp_path in expanded_paths {
                                if let Some(parent_idx) = all_nodes.iter().position(|n| n.path == exp_path && n.is_dir) {
                                    let parent_depth = all_nodes[parent_idx].depth;
                                    if let Ok(children) = load_directory(&exp_path, parent_depth + 1).await {
                                        for (i, child) in children.into_iter().enumerate() {
                                            all_nodes.insert(parent_idx + 1 + i, child);
                                        }
                                    }
                                }
                            }
                            nodes.set(all_nodes);
                        }
                    }
                    rename_state.set(RenameState::default());
                });
                return;
            }
        }
        rename_state.set(RenameState::default());
    };

    // Cancel rename
    let mut cancel_rename = move |()| {
        rename_state.set(RenameState::default());
    };

    let root_name = state
        .read()
        .workspace
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            state
                .read()
                .workspace
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        });

    let is_creating = creating.read().is_some();
    let creating_type = *creating.read();

    rsx! {
        div { class: "explorer-container",
            // Header with title
            div { class: "explorer-header",
                span { class: "explorer-title", "Explorer" }
            }

            // Folder name row with action buttons
            div { class: "explorer-folder-row",
                span { class: "explorer-path", "{root_name}" }
                div { class: "explorer-actions",
                    button {
                        class: "explorer-action-btn",
                        title: "New File",
                        onclick: start_new_file,
                        "+"
                    }
                    button {
                        class: "explorer-action-btn",
                        title: "New Folder",
                        onclick: start_new_folder,
                        "+/"
                    }
                    button {
                        class: "explorer-action-btn",
                        title: "Refresh Explorer",
                        onclick: refresh,
                        "↻"
                    }
                    button {
                        class: "explorer-action-btn",
                        title: "Collapse All",
                        onclick: collapse_all,
                        "⊟"
                    }
                }
            }

            // Tree content - right-click on empty space shows workspace context menu
            // data-root attribute allows JS drag-drop to move items to workspace root
            {
                let workspace_root = workspace_dir.to_string_lossy().to_string();
                rsx! {
                    div {
                        class: "explorer-content",
                        "data-root": "{workspace_root}",
                        oncontextmenu: {
                            let workspace_dir = workspace_dir.clone();
                            move |e: Event<MouseData>| {
                                e.prevent_default();
                                // Show context menu for workspace root (like clicking empty space in VS Code)
                                context_menu.set(ContextMenuState {
                                    visible: true,
                                    x: e.client_coordinates().x as i32,
                                    y: e.client_coordinates().y as i32,
                                    target_path: Some(workspace_dir.clone()),
                                    is_dir: true, // Workspace root is a directory
                                });
                            }
                        },

                for node in nodes.read().iter() {
                    {
                        let path = node.path.clone();
                        let is_dir = node.is_dir;
                        let depth = node.depth;
                        let name = node.name.clone();
                        let size = node.size;
                        let is_expanded = expanded.read().contains(&path);
                        let is_loading = loading.read().contains(&path);
                        let indent = 4 + (depth * 12); // Base 4px + depth indentation

                        // Check if this item is currently selected (open in editor)
                        let current_file = state.read().current_file();
                        let is_selected = current_file.as_ref() == Some(&path);

                        // Check if this item is being renamed
                        let is_renaming = rename_state.read().active
                            && rename_state.read().path.as_ref() == Some(&path);

                        // Check if this folder is where we're creating a new item
                        let is_creating_here = is_creating
                            && is_dir
                            && is_expanded
                            && creating_parent.read().as_ref() == Some(&path);
                        let child_indent = 4 + ((depth + 1) * 12); // Indent for child items

                        // Build class string (drop-target class is managed by JavaScript)
                        let mut class = "tree-item".to_string();
                        if is_selected {
                            class.push_str(" selected");
                        }

                        // Convert path to string for data attribute
                        let path_str = path.to_string_lossy().to_string();

                        rsx! {
                            div {
                                key: "{path:?}",
                                class: "{class}",
                                style: "padding-left: {indent}px;",
                                draggable: true,
                                // Data attributes for JavaScript drag-drop handling
                                "data-path": "{path_str}",
                                "data-is-dir": if is_dir { "true" } else { "false" },

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

                                oncontextmenu: {
                                    let path = path.clone();
                                    move |e: Event<MouseData>| {
                                        e.prevent_default();
                                        e.stop_propagation(); // Don't bubble to parent container
                                        context_menu.set(ContextMenuState {
                                            visible: true,
                                            x: e.client_coordinates().x as i32,
                                            y: e.client_coordinates().y as i32,
                                            target_path: Some(path.clone()),
                                            is_dir,
                                        });
                                    }
                                },

                                // Note: Drag-drop is handled via JavaScript for Linux/WebKitGTK compatibility
                                // See the use_effect that sets up JS event listeners

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

                                // Name or rename input
                                if is_renaming {
                                    input {
                                        class: "tree-inline-input tree-rename-input",
                                        r#type: "text",
                                        value: "{rename_state.read().new_name}",
                                        autofocus: true,
                                        // Use onmounted for reliable focus
                                        onmounted: move |evt| {
                                            spawn(async move {
                                                let _ = evt.set_focus(true).await;
                                            });
                                        },
                                        oninput: move |e| {
                                            rename_state.write().new_name.clone_from(&e.value());
                                        },
                                        onkeydown: move |e| {
                                            if e.key() == Key::Enter {
                                                confirm_rename(());
                                            } else if e.key() == Key::Escape {
                                                cancel_rename(());
                                            }
                                        },
                                        onblur: move |_| {
                                            cancel_rename(());
                                        },
                                        onclick: move |e: Event<MouseData>| {
                                            e.stop_propagation();
                                        }
                                    }
                                } else {
                                    span { class: "tree-name", "{name}" }
                                }

                                // Size (files only)
                                if !is_dir && !is_renaming {
                                    span { class: "tree-size", {format_size(size)} }
                                }
                            }

                            // Inline input for creating new file/folder (rendered inside expanded folder)
                            if is_creating_here {
                                {
                                    let parent_for_create = path.clone();
                                    rsx! {
                                        div {
                                            class: "tree-item tree-input-row",
                                            style: "padding-left: {child_indent}px;",
                                            // Invisible arrow spacer for alignment
                                            span { class: "tree-arrow hidden" }
                                            // Invisible icon spacer for alignment
                                            span { class: "tree-icon file" }
                                            input {
                                                class: "tree-inline-input",
                                                r#type: "text",
                                                placeholder: if creating_type == Some(CreatingType::Folder) { "folder name" } else { "filename.rhai" },
                                                value: "{input_value}",
                                                autofocus: true,
                                                onmounted: move |evt| {
                                                    spawn(async move {
                                                        let _ = evt.set_focus(true).await;
                                                    });
                                                },
                                                oninput: move |e| input_value.set(e.value().clone()),
                                                onkeydown: {
                                                    let parent = parent_for_create.clone();
                                                    move |e: Event<KeyboardData>| {
                                                        if e.key() == Key::Enter {
                                                            let name = input_value.read().trim().to_string();
                                                            if !name.is_empty() {
                                                                let target_path = parent.join(&name);
                                                                let parent_for_refresh = parent.clone();
                                                                let ct = *creating.read();
                                                                spawn(async move {
                                                                    let success = match ct {
                                                                        Some(CreatingType::File) => {
                                                                            tokio::fs::write(&target_path, "").await.is_ok()
                                                                        }
                                                                        Some(CreatingType::Folder) => {
                                                                            tokio::fs::create_dir(&target_path).await.is_ok()
                                                                        }
                                                                        None => false,
                                                                    };
                                                                    if success {
                                                                        // Refresh just the parent folder's children
                                                                        let parent_depth = depth; // depth of the folder we're creating in
                                                                        let child_depth = parent_depth + 1;
                                                                        if let Ok(new_children) = load_directory(&parent_for_refresh, child_depth).await {
                                                                            let mut nodes_write = nodes.write();
                                                                            // Find parent folder index
                                                                            if let Some(parent_idx) = nodes_write.iter().position(|n| n.path == parent_for_refresh) {
                                                                                // Remove old children (nodes after parent with greater depth)
                                                                                let mut remove_count = 0;
                                                                                for i in (parent_idx + 1)..nodes_write.len() {
                                                                                    if nodes_write[i].depth > parent_depth {
                                                                                        remove_count += 1;
                                                                                    } else {
                                                                                        break;
                                                                                    }
                                                                                }
                                                                                for _ in 0..remove_count {
                                                                                    nodes_write.remove(parent_idx + 1);
                                                                                }
                                                                                // Insert new children
                                                                                for (i, child) in new_children.into_iter().enumerate() {
                                                                                    nodes_write.insert(parent_idx + 1 + i, child);
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    creating.set(None);
                                                                    creating_parent.set(None);
                                                                    input_value.set(String::new());
                                                                });
                                                            } else {
                                                                creating.set(None);
                                                                creating_parent.set(None);
                                                            }
                                                        } else if e.key() == Key::Escape {
                                                            creating.set(None);
                                                            creating_parent.set(None);
                                                            input_value.set(String::new());
                                                        }
                                                    }
                                                },
                                                onblur: move |_| {
                                                    creating.set(None);
                                                    creating_parent.set(None);
                                                    input_value.set(String::new());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if nodes.read().is_empty() && !is_creating {
                    div { class: "explorer-empty", "Empty folder" }
                }
                    }
                }
            }

            // Context menu (rendered at root level for proper positioning)
            if context_menu.read().visible {
                // Backdrop to close menu on click outside
                div {
                    class: "context-menu-backdrop",
                    onclick: close_context_menu
                }

                // The actual context menu
                div {
                    class: "context-menu",
                    style: "left: {context_menu.read().x}px; top: {context_menu.read().y}px;",

                    // Folder-specific actions
                    if context_menu.read().is_dir {
                        button {
                            class: "context-menu-item",
                            onclick: ctx_new_file,
                            span { class: "context-menu-icon", "+" }
                            span { "New File" }
                        }
                        button {
                            class: "context-menu-item",
                            onclick: ctx_new_folder,
                            span { class: "context-menu-icon", "" }
                            span { "New Folder" }
                        }
                        div { class: "context-menu-separator" }
                    }

                    // Common actions
                    button {
                        class: "context-menu-item",
                        onclick: ctx_copy_path,
                        span { class: "context-menu-icon", "" }
                        span { "Copy Path" }
                    }

                    div { class: "context-menu-separator" }

                    button {
                        class: "context-menu-item",
                        onclick: ctx_rename,
                        span { class: "context-menu-icon", "" }
                        span { "Rename" }
                    }

                    button {
                        class: "context-menu-item context-menu-item-danger",
                        onclick: ctx_delete,
                        span { class: "context-menu-icon", "" }
                        span { "Delete" }
                    }
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
        "" // Folder icon handled by CSS
    } else {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rhai") => "",  // Script/code file
            Some("rs") => "",    // Rust file
            Some("toml") => "",  // Config file
            Some("md") => "",    // Markdown
            Some("glb") | Some("gltf") | Some("obj") => "", // 3D model
            Some("png") | Some("jpg") | Some("jpeg") | Some("svg") => "", // Image
            Some("json") => "",  // JSON
            Some("css") => "",   // CSS
            Some("lock") => "",  // Lock file
            _ => "",             // Generic file
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
