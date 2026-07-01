//! Explorer panel - VSCode-style expandable file tree
//!
//! This module provides the AssetBrowser component which displays a file tree
//! for navigating and managing files in the workspace.
//!
//! Note: Some CSS classes are kept global (tree-item, explorer-content) because
//! they are accessed by JavaScript for drag-drop functionality.

// map_or_else is less readable for file path operations
#![allow(clippy::map_unwrap_or)]

mod tree;

use tree::{TreeNode, format_size, get_icon, load_directory};

use crate::components::ConfirmDialog;
use crate::state::{AppStateStoreExt, AppStore};
use dioxus::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{error, warn};

/// What type of item is being created
#[derive(Clone, Copy, PartialEq)]
enum CreatingType {
    File,
    Folder,
}

/// Rename state
#[derive(Clone, PartialEq, Default)]
struct RenameState {
    active: bool,
    path: Option<PathBuf>,
    new_name: String,
}

/// JavaScript that installs the drag-drop event delegation used as a
/// WebKitGTK workaround on Linux (native ondragover/ondrop don't fire there).
///
/// The whole thing is wrapped in an IIFE that registers a teardown hook on
/// `window.__soyuzExplorerDnd` so the Rust `use_drop` can disconnect the
/// `MutationObserver` and remove listeners on unmount (otherwise they leak and
/// compound across remounts). The observer is scoped to the stable
/// `.explorer-panel` ancestor rather than `document.body`: `.explorer-content`
/// itself is destroyed and recreated when the workspace folder is closed and
/// reopened, so an observer bound directly to it would be stranded on a
/// detached node. The panel is the narrowest node that survives those toggles
/// while still excluding the editor/terminal subtrees.
const DRAG_DROP_SETUP_JS: &str = r#"
(function () {
    // Tear down any previous instance so a remount doesn't stack observers/listeners.
    if (window.__soyuzExplorerDnd) {
        window.__soyuzExplorerDnd.teardown();
    }

    // Drag-drop state
    let dragSource = null;
    let currentDropTarget = null;
    let observer = null;
    let boundContainer = null;

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

    function addListeners(container) {
        container.addEventListener('dragstart', handleDragStart, true);
        container.addEventListener('dragover', handleDragOver, true);
        container.addEventListener('dragleave', handleDragLeave, true);
        container.addEventListener('drop', handleDrop, true);
        container.addEventListener('dragend', handleDragEnd, true);
    }

    function removeListeners(container) {
        container.removeEventListener('dragstart', handleDragStart, true);
        container.removeEventListener('dragover', handleDragOver, true);
        container.removeEventListener('dragleave', handleDragLeave, true);
        container.removeEventListener('drop', handleDrop, true);
        container.removeEventListener('dragend', handleDragEnd, true);
    }

    // Set up event delegation on the explorer content
    const setupDragDrop = () => {
        const container = document.querySelector('.explorer-content');
        if (!container) {
            // Component not mounted yet (or no folder open), retry
            setTimeout(setupDragDrop, 100);
            return;
        }

        // Rebind if the container was replaced; always dedupe on the current one.
        if (boundContainer && boundContainer !== container) {
            removeListeners(boundContainer);
        }
        removeListeners(container);
        addListeners(container);
        boundContainer = container;
    };

    // Re-setup when the explorer subtree changes (new tree items, or the whole
    // container being recreated after a folder open/close).
    const observeScope = document.querySelector('.explorer-panel') || document.body;
    observer = new MutationObserver(() => setupDragDrop());
    observer.observe(observeScope, { childList: true, subtree: true });

    // Expose a teardown hook for the Rust use_drop.
    window.__soyuzExplorerDnd = {
        teardown() {
            if (observer) {
                observer.disconnect();
                observer = null;
            }
            if (boundContainer) {
                removeListeners(boundContainer);
                boundContainer = null;
            }
            dragSource = null;
            currentDropTarget = null;
            delete window.__soyuzExplorerDnd;
        }
    };

    // Initial setup
    setupDragDrop();
})();
"#;

/// Create a new file or folder, refusing to overwrite an existing path and
/// surfacing any failure to the terminal log. Returns whether creation succeeded.
async fn create_fs_entry(ct: Option<CreatingType>, target_path: &std::path::Path) -> bool {
    // Refuse to clobber an existing path (a bare `write` would truncate a file).
    if target_path.exists() {
        error!(
            "Cannot create, path already exists: {}",
            target_path.display()
        );
        return false;
    }
    match ct {
        Some(CreatingType::File) => match tokio::fs::write(target_path, "").await {
            Ok(()) => true,
            Err(e) => {
                error!("Failed to create file {}: {e}", target_path.display());
                false
            }
        },
        Some(CreatingType::Folder) => match tokio::fs::create_dir(target_path).await {
            Ok(()) => true,
            Err(e) => {
                error!("Failed to create folder {}: {e}", target_path.display());
                false
            }
        },
        None => false,
    }
}

/// Reload the whole tree from `dir`, then re-expand every folder that is
/// currently open in `expanded` (preserving the user's open folders across a
/// filesystem change). Shared by the move, delete and rename flows.
async fn reload_tree_preserving_expanded(
    dir: PathBuf,
    mut nodes: Signal<Vec<TreeNode>>,
    expanded: Signal<HashSet<PathBuf>>,
) {
    let Ok(mut all_nodes) = load_directory(&dir, 0).await else {
        return;
    };

    let mut expanded_paths: Vec<_> = expanded.peek().iter().cloned().collect();
    expanded_paths.sort_by_key(|p| p.components().count());
    for exp_path in expanded_paths {
        if let Some(parent_idx) = all_nodes
            .iter()
            .position(|n| n.path == exp_path && n.is_dir)
        {
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

/// Empty state component shown when no folder is opened
#[component]
fn EmptyExplorerState() -> Element {
    let mut state = use_context::<AppStore>();

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
            div { class: "empty-state",
                p { class: "empty-title", "No Folder Opened" }
                p { class: "empty-subtitle",
                    "You have not yet opened a folder."
                }
                button {
                    class: "empty-button",
                    onclick: open_folder,
                    "Open Folder"
                }
            }
        }
    }
}

/// Inline `<input>` for creating a new file or folder. Owns the text field
/// (placeholder, autofocus, value binding, key handling); the caller decides
/// what a submitted name means via `on_submit`, and how to tear the input down
/// via `on_cancel`. This dedupes the two previously copy-pasted create blocks
/// (root level vs. inside an expanded folder).
#[component]
fn CreateEntryInput(
    is_folder: bool,
    indent: usize,
    mut input_value: Signal<String>,
    on_submit: EventHandler<String>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "tree-item input-row",
            style: "padding-left: {indent}px;",
            // Invisible arrow spacer for alignment
            span { class: "tree-arrow hidden" }
            // Invisible icon spacer for alignment
            span { class: "tree-icon file" }
            input {
                class: "inline-input",
                r#type: "text",
                placeholder: if is_folder { "folder name" } else { "filename.rhai" },
                value: "{input_value}",
                autofocus: true,
                onmounted: move |evt| {
                    spawn(async move {
                        let _ = evt.set_focus(true).await;
                    });
                },
                oninput: move |e| input_value.set(e.value().clone()),
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Enter {
                        let name = input_value.peek().trim().to_string();
                        if name.is_empty() {
                            on_cancel.call(());
                        } else {
                            on_submit.call(name);
                        }
                    } else if e.key() == Key::Escape {
                        on_cancel.call(());
                    }
                },
                onblur: move |_| on_cancel.call(()),
            }
        }
    }
}

/// A single row in the file tree (the folder/file entry plus, when this folder
/// is the active creation target, the inline create input rendered beneath it).
#[component]
#[allow(clippy::too_many_arguments)]
fn TreeItem(
    node: TreeNode,
    is_expanded: bool,
    is_loading: bool,
    is_selected: bool,
    is_creating_here: bool,
    is_folder_create: bool,
    mut rename_state: Signal<RenameState>,
    mut menu_target: Signal<Option<(PathBuf, bool)>>,
    input_value: Signal<String>,
    on_activate: EventHandler<()>,
    on_confirm_rename: EventHandler<()>,
    on_cancel_rename: EventHandler<()>,
    on_create_submit: EventHandler<String>,
    on_create_cancel: EventHandler<()>,
) -> Element {
    let path = node.path.clone();
    let is_dir = node.is_dir;
    let depth = node.depth;
    let indent = 4 + (depth * 12); // Base 4px + depth indentation
    let child_indent = 4 + ((depth + 1) * 12); // Indent for child items
    let path_str = path.to_string_lossy().to_string();

    // Check if this item is being renamed
    let is_renaming =
        rename_state.read().active && rename_state.read().path.as_ref() == Some(&path);

    // Build class string (drop-target class is managed by JavaScript)
    let mut class = "tree-item".to_string();
    if is_selected {
        class.push_str(" selected");
    }

    let arrow_class = if is_dir {
        if is_expanded {
            "tree-arrow expanded"
        } else {
            "tree-arrow"
        }
    } else {
        "tree-arrow hidden"
    };

    rsx! {
        div {
            class: "{class}",
            style: "padding-left: {indent}px;",
            draggable: true,
            // Data attributes for JavaScript drag-drop handling
            "data-path": "{path_str}",
            "data-is-dir": if is_dir { "true" } else { "false" },

            onclick: move |_| on_activate.call(()),

            // Record which node was right-clicked; the ContextMenuTrigger
            // wrapping the tree opens and positions the menu. Must bubble (no
            // stop_propagation) so the trigger receives the event and opens.
            oncontextmenu: {
                let path = path.clone();
                move |_: Event<MouseData>| {
                    menu_target.set(Some((path.clone(), is_dir)));
                }
            },

            // Expand/collapse arrow (or spacer for files)
            span { class: "{arrow_class}",
                if is_loading {
                    "○" // Loading indicator
                } else if is_dir {
                    "▶"
                }
            }

            // File/folder icon
            span {
                class: if is_dir { "tree-icon folder" } else { "tree-icon file" },
                {get_icon(&path, is_dir)}
            }

            // Name or rename input
            if is_renaming {
                input {
                    class: "inline-input rename-input",
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
                            on_confirm_rename.call(());
                        } else if e.key() == Key::Escape {
                            on_cancel_rename.call(());
                        }
                    },
                    onblur: move |_| on_cancel_rename.call(()),
                    onclick: move |e: Event<MouseData>| e.stop_propagation(),
                }
            } else {
                span { class: "tree-name", "{node.name}" }
            }

            // Size (files only)
            if !is_dir && !is_renaming {
                span { class: "tree-size", {format_size(node.size)} }
            }
        }

        // Inline input for creating new file/folder (rendered inside expanded folder)
        if is_creating_here {
            CreateEntryInput {
                is_folder: is_folder_create,
                indent: child_indent,
                input_value,
                on_submit: on_create_submit,
                on_cancel: on_create_cancel,
            }
        }
    }
}

/// Explorer component - VSCode style expandable file tree
#[component]
pub fn AssetBrowser() -> Element {
    use dioxus_primitives::ContentSide;
    use dioxus_primitives::context_menu::{
        ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger,
    };
    use dioxus_primitives::tooltip::{Tooltip, TooltipContent, TooltipTrigger};

    let mut state = use_context::<AppStore>();

    // ---- Hooks (all called unconditionally so the hook count never changes
    // between a workspace-less and a workspace-loaded render) ----

    // Tree state - kept local to this component
    let mut nodes: Signal<Vec<TreeNode>> = use_signal(Vec::new);
    let mut expanded: Signal<HashSet<PathBuf>> = use_signal(HashSet::new);
    let mut loading: Signal<HashSet<PathBuf>> = use_signal(HashSet::new);

    // State for inline creation input
    let mut creating: Signal<Option<CreatingType>> = use_signal(|| None);
    let mut creating_parent: Signal<Option<PathBuf>> = use_signal(|| None); // Target folder for new file/folder
    let mut input_value: Signal<String> = use_signal(String::new);

    // Which node the context menu acts on: (path, is_dir). The dioxus-primitives
    // ContextMenu owns visibility/position; this just carries the target.
    let mut menu_target: Signal<Option<(PathBuf, bool)>> = use_signal(|| None);

    // Rename state
    let mut rename_state: Signal<RenameState> = use_signal(RenameState::default);

    // Pending delete awaiting confirmation: (path, is_dir)
    let mut pending_delete: Signal<Option<(PathBuf, bool)>> = use_signal(|| None);

    // Handle to the drag-drop eval, kept so `use_drop` can drop it (ending the
    // JS recv loop) alongside disconnecting the observer.
    let mut drag_drop_eval: Signal<Option<document::Eval>> = use_signal(|| None);

    // Load the root directory whenever the workspace folder changes. Reading the
    // workspace field *inside* the resource keys the reload on it, so switching
    // folders reloads the tree (the old once-only effect never did).
    let _root_resource = use_resource(move || async move {
        let workspace = state.workspace().read().clone();
        if let Some(dir) = workspace
            && let Ok(entries) = load_directory(&dir, 0).await
        {
            nodes.set(entries);
            // Clear expanded state when root changes
            expanded.write().clear();
        }
    });

    // JavaScript-based drag-drop setup (workaround for WebKitGTK bug on Linux).
    // Native ondragover/ondrop don't fire properly, so we use JS event listeners.
    use_effect(move || {
        // Set up JavaScript drag-drop handlers and communication channel
        let eval = document::eval(DRAG_DROP_SETUP_JS);
        // Keep a handle so use_drop can drop it (releasing the recv channel).
        drag_drop_eval.set(Some(eval));

        // Listen for drop events from JavaScript
        spawn(async move {
            let mut eval = eval;
            while let Ok(msg) = eval.recv::<serde_json::Value>().await {
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
                            if let Err(e) = tokio::fs::rename(&source_path, &new_path).await {
                                error!(
                                    "Failed to move {} to {}: {e}",
                                    source_path.display(),
                                    new_path.display()
                                );
                            } else if let Some(dir) = state.workspace().peek().clone() {
                                // Refresh tree while preserving expanded folders
                                reload_tree_preserving_expanded(dir, nodes, expanded).await;
                            }
                        }
                    }
                }
            }
        });
    });

    // Tear down the JS observer/listeners on unmount so they don't leak and
    // compound across remounts, and drop the eval handle to end the recv loop.
    use_drop(move || {
        let _ = document::eval(
            "if (window.__soyuzExplorerDnd) { window.__soyuzExplorerDnd.teardown(); }",
        );
        drag_drop_eval.set(None);
    });

    // ---- Non-hook derived values and handlers ----

    // Toggle expand/collapse for a directory
    let mut toggle_dir = move |path: PathBuf, depth: usize| {
        let is_expanded = expanded.peek().contains(&path);

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

    // Reload the root (used by both Refresh and Collapse All - identical behavior:
    // reload from disk and collapse every folder).
    let reload_root = move |_| {
        let Some(dir) = state.workspace().peek().clone() else {
            return;
        };
        spawn(async move {
            if let Ok(entries) = load_directory(&dir, 0).await {
                nodes.set(entries);
                expanded.write().clear();
            }
        });
    };

    // Start creating a new file (at workspace root)
    let start_new_file = move |_| {
        let Some(dir) = state.workspace().peek().clone() else {
            return;
        };
        creating.set(Some(CreatingType::File));
        creating_parent.set(Some(dir));
        input_value.set(String::new());
    };

    // Start creating a new folder (at workspace root)
    let start_new_folder = move |_| {
        let Some(dir) = state.workspace().peek().clone() else {
            return;
        };
        creating.set(Some(CreatingType::Folder));
        creating_parent.set(Some(dir));
        input_value.set(String::new());
    };

    // Context menu: Copy Path
    let ctx_copy_path = move |_| {
        if let Some((path, _)) = menu_target.peek().clone() {
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
                    match Clipboard::new() {
                        Ok(mut clipboard) => {
                            if let Err(e) = clipboard.set_text(&path_str) {
                                warn!("Failed to copy path to clipboard: {e}");
                            }
                        }
                        Err(e) => warn!("Failed to access clipboard: {e}"),
                    }
                }
            });
        }
    };

    // Context menu: Rename
    let ctx_rename = move |_| {
        if let Some((path, _)) = menu_target.peek().clone() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            rename_state.set(RenameState {
                active: true,
                path: Some(path),
                new_name: name,
            });
        }
    };

    // Context menu: Delete (asks for confirmation before deleting)
    let ctx_delete = move |_| {
        if let Some((path, is_dir)) = menu_target.peek().clone() {
            pending_delete.set(Some((path, is_dir)));
        }
    };

    // Perform the confirmed deletion (surfaces failures to the terminal log)
    let mut confirm_delete = move |()| {
        let Some((path, is_dir)) = pending_delete.peek().clone() else {
            return;
        };
        pending_delete.set(None);
        spawn(async move {
            let result = if is_dir {
                tokio::fs::remove_dir_all(&path).await
            } else {
                tokio::fs::remove_file(&path).await
            };

            if let Err(e) = result {
                error!("Failed to delete {}: {e}", path.display());
                return;
            }

            // Close any open tabs for the deleted file/directory
            state.write().close_tabs_for_deleted_path(&path, is_dir);

            // Refresh tree while preserving expanded folders
            if let Some(dir) = state.workspace().peek().clone() {
                reload_tree_preserving_expanded(dir, nodes, expanded).await;
            }
        });
    };

    // Context menu: New File (folders only)
    let ctx_new_file = move |_| {
        if let Some((parent_path, _)) = menu_target.peek().clone() {
            // Expand the folder if not already expanded
            if !expanded.peek().contains(&parent_path) {
                let depth = nodes
                    .peek()
                    .iter()
                    .find(|n| n.path == parent_path)
                    .map(|n| n.depth)
                    .unwrap_or(0);
                toggle_dir(parent_path.clone(), depth);
            }
            // Start inline creation at the folder location
            creating.set(Some(CreatingType::File));
            creating_parent.set(Some(parent_path));
            input_value.set(String::new());
        }
    };

    // Context menu: New Folder (folders only)
    let ctx_new_folder = move |_| {
        if let Some((parent_path, _)) = menu_target.peek().clone() {
            // Expand the folder if not already expanded
            if !expanded.peek().contains(&parent_path) {
                let depth = nodes
                    .peek()
                    .iter()
                    .find(|n| n.path == parent_path)
                    .map(|n| n.depth)
                    .unwrap_or(0);
                toggle_dir(parent_path.clone(), depth);
            }
            // Start inline creation at the folder location
            creating.set(Some(CreatingType::Folder));
            creating_parent.set(Some(parent_path));
            input_value.set(String::new());
        }
    };

    // Confirm rename
    let mut confirm_rename = move |()| {
        let rename = rename_state.peek().clone();
        if !rename.active || rename.new_name.is_empty() {
            rename_state.set(RenameState::default());
            return;
        }

        if let Some(old_path) = rename.path {
            let new_name = rename.new_name.clone();
            let new_path = old_path.parent().map(|p| p.join(&new_name));

            if let Some(new_path) = new_path {
                spawn(async move {
                    if let Err(e) = tokio::fs::rename(&old_path, &new_path).await {
                        error!(
                            "Failed to rename {} to {}: {e}",
                            old_path.display(),
                            new_path.display()
                        );
                    } else if let Some(dir) = state.workspace().peek().clone() {
                        // Refresh tree while preserving expanded folders
                        reload_tree_preserving_expanded(dir, nodes, expanded).await;
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

    // Whether an inline create input is currently active, and its kind.
    let is_creating = creating.read().is_some();
    let creating_type = *creating.read();

    // Currently open file, computed ONCE from fine-grained field selectors
    // (editor pane + focused pane) instead of re-reading the whole store for
    // every tree node inside the render loop.
    let current_file = {
        let focused = *state.focused_pane_id().read();
        let pane = state.editor_pane();
        let pane_ref = pane.read();
        pane_ref
            .find_pane(focused)
            .and_then(|p| p.active_tab())
            .and_then(|t| t.path.clone())
    };

    // Branch on the workspace via a field selector (not a whole-store read) and
    // ONLY in the returned markup, so the hooks above always run.
    let workspace_dir = state.workspace().read().clone();

    rsx! {
        if let Some(workspace_dir) = workspace_dir {
            {
                let root_name = workspace_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| workspace_dir.display().to_string());
                let workspace_root = workspace_dir.to_string_lossy().to_string();
                let is_creating_at_root = is_creating
                    && creating_parent.read().as_ref() == Some(&workspace_dir);

                rsx! {
                    div {
                        class: "explorer-container",
                        // A right-click's mousedown fires (and bubbles up here) before its
                        // contextmenu, so default the menu target to the workspace root; a
                        // tree row's own oncontextmenu then overrides it. Keeps "right-click
                        // empty space acts on the workspace root" working. Gated to the
                        // secondary button so a left-click that *selects* a menu item (whose
                        // mousedown also bubbles here) doesn't clobber the target first.
                        onmousedown: {
                            let workspace_dir = workspace_dir.clone();
                            move |e: Event<MouseData>| {
                                if e.trigger_button()
                                    == Some(dioxus_elements::input_data::MouseButton::Secondary)
                                {
                                    menu_target.set(Some((workspace_dir.clone(), true)));
                                }
                            }
                        },
                        // Header with title
                        div { class: "explorer-header",
                            span { class: "explorer-title", "Explorer" }
                        }

                        // Folder name row with action buttons
                        div { class: "folder-row",
                            span { class: "explorer-path", "{root_name}" }
                            div { class: "explorer-actions",
                                Tooltip { class: "action-tooltip",
                                    TooltipTrigger { class: "action-tooltip-trigger",
                                        button {
                                            class: "action-btn",
                                            r#type: "button",
                                            aria_label: "New File",
                                            onclick: start_new_file,
                                            "+"
                                        }
                                    }
                                    TooltipContent {
                                        class: "action-tooltip-content",
                                        side: ContentSide::Bottom,
                                        "New File"
                                    }
                                }
                                Tooltip { class: "action-tooltip",
                                    TooltipTrigger { class: "action-tooltip-trigger",
                                        button {
                                            class: "action-btn",
                                            r#type: "button",
                                            aria_label: "New Folder",
                                            onclick: start_new_folder,
                                            "+/"
                                        }
                                    }
                                    TooltipContent {
                                        class: "action-tooltip-content",
                                        side: ContentSide::Bottom,
                                        "New Folder"
                                    }
                                }
                                button {
                                    class: "action-btn",
                                    title: "Refresh Explorer",
                                    onclick: reload_root,
                                    "↻"
                                }
                                button {
                                    class: "action-btn",
                                    title: "Collapse All",
                                    onclick: reload_root,
                                    "⊟"
                                }
                            }
                        }

                        // Tree content wrapped in a dioxus-primitives ContextMenu. The
                        // trigger *is* the scrollable content area, so a right-click
                        // anywhere (row or empty space) opens the menu; the primitive owns
                        // positioning, focus, keyboard nav and outside/Escape dismiss.
                        // data-root lets the JS drag-drop move items to the workspace root.
                        ContextMenu { class: "explorer-menu-root",
                            ContextMenuTrigger {
                                class: "explorer-content",
                                "data-root": "{workspace_root}",

                                for node in nodes.read().iter() {
                                    {
                                        let path = node.path.clone();
                                        let is_dir = node.is_dir;
                                        let depth = node.depth;
                                        let node_data = node.clone();
                                        let is_expanded = expanded.read().contains(&path);
                                        let is_loading = loading.read().contains(&path);
                                        let is_selected = current_file.as_ref() == Some(&path);
                                        let is_creating_here = is_creating
                                            && is_dir
                                            && is_expanded
                                            && creating_parent.read().as_ref() == Some(&path);

                                        rsx! {
                                            TreeItem {
                                                key: "{path:?}",
                                                node: node_data,
                                                is_expanded,
                                                is_loading,
                                                is_selected,
                                                is_creating_here,
                                                is_folder_create: creating_type == Some(CreatingType::Folder),
                                                rename_state,
                                                menu_target,
                                                input_value,
                                                on_activate: {
                                                    let path = path.clone();
                                                    move |_| {
                                                        if is_dir {
                                                            toggle_dir(path.clone(), depth);
                                                        } else {
                                                            open_file(path.clone());
                                                        }
                                                    }
                                                },
                                                on_confirm_rename: move |_| confirm_rename(()),
                                                on_cancel_rename: move |_| cancel_rename(()),
                                                on_create_cancel: move |_| {
                                                    creating.set(None);
                                                    creating_parent.set(None);
                                                    input_value.set(String::new());
                                                },
                                                on_create_submit: {
                                                    let path = path.clone();
                                                    move |name: String| {
                                                        let target_path = path.join(&name);
                                                        let parent_for_refresh = path.clone();
                                                        let ct = *creating.peek();
                                                        spawn(async move {
                                                            if create_fs_entry(ct, &target_path).await
                                                                && let Ok(new_children) =
                                                                    load_directory(&parent_for_refresh, depth + 1).await
                                                            {
                                                                // Replace just this folder's children in place.
                                                                let mut nodes_write = nodes.write();
                                                                if let Some(parent_idx) = nodes_write
                                                                    .iter()
                                                                    .position(|n| n.path == parent_for_refresh)
                                                                {
                                                                    let mut remove_count = 0;
                                                                    for i in (parent_idx + 1)..nodes_write.len() {
                                                                        if nodes_write[i].depth > depth {
                                                                            remove_count += 1;
                                                                        } else {
                                                                            break;
                                                                        }
                                                                    }
                                                                    for _ in 0..remove_count {
                                                                        nodes_write.remove(parent_idx + 1);
                                                                    }
                                                                    for (i, child) in new_children.into_iter().enumerate() {
                                                                        nodes_write.insert(parent_idx + 1 + i, child);
                                                                    }
                                                                }
                                                            }
                                                            creating.set(None);
                                                            creating_parent.set(None);
                                                            input_value.set(String::new());
                                                        });
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }

                                // Inline input for creating at workspace root level
                                if is_creating_at_root {
                                    CreateEntryInput {
                                        is_folder: creating_type == Some(CreatingType::Folder),
                                        indent: 4,
                                        input_value,
                                        on_cancel: move |_| {
                                            creating.set(None);
                                            creating_parent.set(None);
                                            input_value.set(String::new());
                                        },
                                        on_submit: {
                                            let ws = workspace_dir.clone();
                                            move |name: String| {
                                                let target_path = ws.join(&name);
                                                let parent_for_refresh = ws.clone();
                                                let ct = *creating.peek();
                                                spawn(async move {
                                                    if create_fs_entry(ct, &target_path).await
                                                        && let Ok(new_entries) =
                                                            load_directory(&parent_for_refresh, 0).await
                                                    {
                                                        nodes.set(new_entries);
                                                        expanded.write().clear();
                                                    }
                                                    creating.set(None);
                                                    creating_parent.set(None);
                                                    input_value.set(String::new());
                                                });
                                            }
                                        },
                                    }
                                }

                                if nodes.read().is_empty() && !is_creating {
                                    div { class: "tree-empty", "Empty folder" }
                                }
                            }

                            // The menu itself: the primitive renders/positions it on
                            // right-click and closes it on select, Escape or outside click.
                            // Items are gated on the right-clicked target's is_dir; fixed
                            // indices drive keyboard nav (absent dir-only items just leave
                            // gaps, which the roving focus handles).
                            ContextMenuContent { class: "context-menu",
                                {
                                    let is_dir = menu_target.read().as_ref().is_some_and(|(_, d)| *d);
                                    rsx! {
                                        // Folder-specific actions
                                        if is_dir {
                                            ContextMenuItem {
                                                class: "context-menu-item",
                                                value: "new-file".to_string(),
                                                index: 0usize,
                                                on_select: ctx_new_file,
                                                span { class: "context-menu-icon", "+" }
                                                span { "New File" }
                                            }
                                            ContextMenuItem {
                                                class: "context-menu-item",
                                                value: "new-folder".to_string(),
                                                index: 1usize,
                                                on_select: ctx_new_folder,
                                                span { class: "context-menu-icon" }
                                                span { "New Folder" }
                                            }
                                            div { class: "context-menu-separator" }
                                        }

                                        // Common actions
                                        ContextMenuItem {
                                            class: "context-menu-item",
                                            value: "copy-path".to_string(),
                                            index: 2usize,
                                            on_select: ctx_copy_path,
                                            span { class: "context-menu-icon" }
                                            span { "Copy Path" }
                                        }

                                        div { class: "context-menu-separator" }

                                        ContextMenuItem {
                                            class: "context-menu-item",
                                            value: "rename".to_string(),
                                            index: 3usize,
                                            on_select: ctx_rename,
                                            span { class: "context-menu-icon" }
                                            span { "Rename" }
                                        }

                                        ContextMenuItem {
                                            class: "context-menu-item danger",
                                            value: "delete".to_string(),
                                            index: 4usize,
                                            on_select: ctx_delete,
                                            span { class: "context-menu-icon" }
                                            span { "Delete" }
                                        }
                                    }
                                }
                            }
                        }

                        // Delete confirmation dialog
                        if let Some((del_path, del_is_dir)) = pending_delete.read().clone() {
                            ConfirmDialog {
                                title: format!("Delete {}?", if del_is_dir { "folder" } else { "file" }),
                                message: format!(
                                    "Are you sure you want to delete \"{}\"? This cannot be undone.",
                                    del_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| del_path.display().to_string()),
                                ),
                                confirm_label: "Delete",
                                destructive: true,
                                on_confirm: move |_| confirm_delete(()),
                                on_cancel: move |_| pending_delete.set(None),
                            }
                        }
                    }
                }
            }
        } else {
            EmptyExplorerState {}
        }
    }
}
