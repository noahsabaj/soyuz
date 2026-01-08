//! Command Palette - Unified fuzzy search for files and commands
//!
//! Provides a single search interface that searches both files and commands
//! simultaneously, with fuzzy matching for typo tolerance.

#![allow(clippy::map_unwrap_or)]
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::state::AppState;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};
use strsim::jaro_winkler;

/// Search mode - only for special utility prefixes
#[derive(Clone, Copy, PartialEq, Default)]
pub enum PaletteMode {
    #[default]
    Unified,    // Search files + commands together (default)
    GoToLine,   // : prefix - go to line number
    TextSearch, // % or # prefix - search text in files
    Symbols,    // @ prefix - search symbols in current file
}

/// A command that can be executed from the palette
#[derive(Clone)]
pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub category: &'static str,
}

/// A file result from search
#[derive(Clone)]
pub struct FileResult {
    pub path: PathBuf,
    pub name: String,
    pub relative_path: String,
    pub score: i32,
}

/// Unified search result - can be either a file or a command
#[derive(Clone)]
pub enum SearchResult {
    File(FileResult),
    Command { cmd: Command, score: i32 },
}

impl SearchResult {
    /// Get the relevance score for sorting
    pub fn score(&self) -> i32 {
        match self {
            SearchResult::File(f) => f.score,
            SearchResult::Command { score, .. } => *score,
        }
    }
}

/// Command palette state - shared via context
#[derive(Clone, Default)]
pub struct PaletteState {
    pub visible: bool,
    pub query: String,
    pub mode: PaletteMode,
    pub selected_index: usize,
    pub unified_results: Vec<SearchResult>,
}

/// All available commands in Soyuz Studio
pub fn get_all_commands() -> Vec<Command> {
    vec![
        // File operations
        Command { id: "file.new", label: "New File", shortcut: Some("Ctrl+N"), category: "File" },
        Command { id: "file.open", label: "Open File", shortcut: Some("Ctrl+O"), category: "File" },
        Command { id: "file.save", label: "Save", shortcut: Some("Ctrl+S"), category: "File" },
        Command { id: "file.saveAs", label: "Save As", shortcut: Some("Ctrl+Shift+S"), category: "File" },
        Command { id: "file.openFolder", label: "Open Folder", shortcut: None, category: "File" },
        Command { id: "file.closeFolder", label: "Close Folder", shortcut: None, category: "File" },

        // Edit operations
        Command { id: "edit.undo", label: "Undo", shortcut: Some("Ctrl+Z"), category: "Edit" },
        Command { id: "edit.redo", label: "Redo", shortcut: Some("Ctrl+Shift+Z"), category: "Edit" },
        Command { id: "edit.cut", label: "Cut", shortcut: Some("Ctrl+X"), category: "Edit" },
        Command { id: "edit.copy", label: "Copy", shortcut: Some("Ctrl+C"), category: "Edit" },
        Command { id: "edit.paste", label: "Paste", shortcut: Some("Ctrl+V"), category: "Edit" },
        Command { id: "edit.selectAll", label: "Select All", shortcut: Some("Ctrl+A"), category: "Edit" },

        // View operations
        Command { id: "view.commandPalette", label: "Command Palette", shortcut: Some("Ctrl+Shift+P"), category: "View" },
        Command { id: "view.goToFile", label: "Go to File", shortcut: Some("Ctrl+P"), category: "View" },
        Command { id: "view.goToLine", label: "Go to Line", shortcut: Some("Ctrl+G"), category: "View" },

        // Preview operations
        Command { id: "preview.run", label: "Run Preview", shortcut: Some("F5"), category: "Preview" },
        Command { id: "preview.stop", label: "Stop Preview", shortcut: Some("Shift+F5"), category: "Preview" },

        // Export operations
        Command { id: "export.obj", label: "Export as OBJ", shortcut: None, category: "Export" },
        Command { id: "export.stl", label: "Export as STL", shortcut: None, category: "Export" },
        Command { id: "export.gltf", label: "Export as GLTF", shortcut: None, category: "Export" },

        // Window operations
        Command { id: "window.new", label: "New Window", shortcut: None, category: "Window" },
        Command { id: "window.minimize", label: "Minimize", shortcut: None, category: "Window" },
        Command { id: "window.maximize", label: "Toggle Maximize", shortcut: None, category: "Window" },
        Command { id: "window.close", label: "Close Window", shortcut: Some("Alt+F4"), category: "Window" },

        // Help
        Command { id: "help.cookbook", label: "Open Cookbook", shortcut: None, category: "Help" },
        Command { id: "help.documentation", label: "Open Documentation", shortcut: Some("F1"), category: "Help" },
        Command { id: "help.about", label: "About Soyuz Studio", shortcut: None, category: "Help" },
    ]
}

/// Compute fuzzy match score between query and target
/// Returns 0-100 where higher is better match
fn fuzzy_match_score(query: &str, target: &str) -> i32 {
    if query.is_empty() {
        return 50; // Base score for empty query
    }

    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();

    // Exact match = highest score
    if target_lower == query_lower {
        return 100;
    }

    // Starts with = very high score
    if target_lower.starts_with(&query_lower) {
        return 95;
    }

    // Contains = high score
    if target_lower.contains(&query_lower) {
        return 80;
    }

    // Jaro-Winkler similarity for typo tolerance
    let similarity = jaro_winkler(&query_lower, &target_lower);
    (similarity * 100.0) as i32
}

/// Search commands with fuzzy matching
fn search_commands(query: &str) -> Vec<(Command, i32)> {
    let all_commands = get_all_commands();
    let threshold = if query.is_empty() { 50 } else { 55 };

    all_commands
        .into_iter()
        .map(|cmd| {
            // Score against label, category, and id
            let label_score = fuzzy_match_score(query, cmd.label);
            let category_score = fuzzy_match_score(query, cmd.category);
            let id_score = fuzzy_match_score(query, cmd.id);
            let best_score = label_score.max(category_score).max(id_score);
            (cmd, best_score)
        })
        .filter(|(_, score)| *score >= threshold)
        .collect()
}

/// Search files in workspace with fuzzy matching
pub async fn search_files(workspace: &Path, query: &str) -> Vec<FileResult> {
    let mut results = Vec::new();
    let threshold = if query.is_empty() { 50 } else { 55 };

    // Recursively walk directory
    if let Ok(entries) = walk_directory(workspace, workspace).await {
        for (path, relative) in entries {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Score against filename and path
            let name_score = fuzzy_match_score(query, &name);
            let path_score = fuzzy_match_score(query, &relative);
            let best_score = name_score.max(path_score);

            if best_score >= threshold {
                results.push(FileResult {
                    path,
                    name,
                    relative_path: relative,
                    score: best_score,
                });
            }
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.cmp(&a.score));

    // Limit results
    results.truncate(30);

    results
}

/// Recursively walk a directory and return all file paths
async fn walk_directory(root: &Path, base: &Path) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut results = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files and common ignore patterns
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }

                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path
                        .strip_prefix(base)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string());
                    results.push((path, relative));
                }
            }
        }
    }

    Ok(results)
}

/// Unified search - searches both files and commands, returns sorted results
pub async fn unified_search(workspace: Option<&Path>, query: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Search commands (sync, fast)
    let cmd_results = search_commands(query);
    for (cmd, score) in cmd_results {
        results.push(SearchResult::Command { cmd, score });
    }

    // Search files (async)
    if let Some(ws) = workspace {
        let file_results = search_files(ws, query).await;
        for file_result in file_results {
            results.push(SearchResult::File(file_result));
        }
    }

    // Sort by score descending
    results.sort_by_key(|r| std::cmp::Reverse(r.score()));

    // Limit total results
    results.truncate(50);

    results
}

/// Command Palette component
#[component]
pub fn CommandPalette() -> Element {
    let state = use_context::<Signal<AppState>>();
    let mut palette = use_context::<Signal<PaletteState>>();

    let visible = palette.read().visible;

    if !visible {
        return rsx! {};
    }

    let query = palette.read().query.clone();
    let mode = palette.read().mode;
    let selected_index = palette.read().selected_index;

    // Determine mode from query prefix
    let (effective_mode, search_query) = parse_query(&query);

    // Update mode if changed
    if effective_mode != mode {
        palette.write().mode = effective_mode;
    }

    // Close palette
    let close_palette = move |_| {
        palette.write().visible = false;
        palette.write().query.clear();
        palette.write().selected_index = 0;
    };

    // Handle input change
    let on_input = move |e: Event<FormData>| {
        let new_query = e.value().clone();
        palette.write().query.clone_from(&new_query);
        palette.write().selected_index = 0;

        let (mode, search_term) = parse_query(&new_query);
        palette.write().mode = mode;

        // Trigger search in unified mode
        if mode == PaletteMode::Unified {
            let workspace = state.read().workspace.clone();
            let search_term = search_term.to_string();
            spawn(async move {
                let results = unified_search(workspace.as_deref(), &search_term).await;
                palette.write().unified_results = results;
            });
        }
    };

    // Handle keyboard navigation
    let on_keydown = move |e: Event<KeyboardData>| {
        match e.key() {
            Key::Escape => {
                palette.write().visible = false;
                palette.write().query.clear();
            }
            Key::ArrowDown => {
                e.prevent_default();
                let max_items = get_result_count(&palette.read());
                if max_items > 0 {
                    let current = palette.read().selected_index;
                    palette.write().selected_index = (current + 1) % max_items;
                }
            }
            Key::ArrowUp => {
                e.prevent_default();
                let max_items = get_result_count(&palette.read());
                if max_items > 0 {
                    let current = palette.read().selected_index;
                    palette.write().selected_index = if current == 0 {
                        max_items - 1
                    } else {
                        current - 1
                    };
                }
            }
            Key::Enter => {
                e.prevent_default();
                execute_selected(&palette.read(), state);
                palette.write().visible = false;
                palette.write().query.clear();
                palette.write().selected_index = 0;
            }
            _ => {}
        }
    };

    // Get placeholder text based on mode
    let placeholder = match effective_mode {
        PaletteMode::Unified => "Search files and commands...",
        PaletteMode::GoToLine => "Enter line number...",
        PaletteMode::TextSearch => "Search text in files...",
        PaletteMode::Symbols => "Search symbols in current file...",
    };

    rsx! {
        // Backdrop
        div {
            class: "palette-backdrop",
            onclick: close_palette
        }

        // Palette container
        div {
            class: "palette-container",
            onclick: |e| e.stop_propagation(),

            // Search input
            div { class: "palette-input-row",
                input {
                    class: "palette-input",
                    r#type: "text",
                    placeholder: "{placeholder}",
                    value: "{query}",
                    autofocus: true,
                    onmounted: move |evt| {
                        spawn(async move {
                            let _ = evt.set_focus(true).await;
                        });
                    },
                    oninput: on_input,
                    onkeydown: on_keydown
                }
            }

            // Results list
            div { class: "palette-results",
                match effective_mode {
                    PaletteMode::Unified => rsx! {
                        UnifiedResults { selected_index }
                    },
                    PaletteMode::GoToLine => rsx! {
                        GoToLineHint { query: search_query.to_string() }
                    },
                    PaletteMode::TextSearch => rsx! {
                        TextSearchResults { query: search_query.to_string() }
                    },
                    PaletteMode::Symbols => rsx! {
                        div { class: "palette-hint", "Symbol search coming soon..." }
                    },
                }
            }
        }
    }
}

/// Parse query to determine mode and extract search term
fn parse_query(query: &str) -> (PaletteMode, &str) {
    if let Some(rest) = query.strip_prefix(':') {
        (PaletteMode::GoToLine, rest.trim())
    } else if let Some(rest) = query.strip_prefix('%') {
        (PaletteMode::TextSearch, rest.trim())
    } else if let Some(rest) = query.strip_prefix('#') {
        (PaletteMode::TextSearch, rest.trim())
    } else if let Some(rest) = query.strip_prefix('@') {
        (PaletteMode::Symbols, rest.trim())
    } else {
        (PaletteMode::Unified, query.trim())
    }
}

/// Get total result count for keyboard navigation
fn get_result_count(palette: &PaletteState) -> usize {
    match palette.mode {
        PaletteMode::Unified => palette.unified_results.len(),
        _ => 0,
    }
}

/// Execute the selected item
fn execute_selected(palette: &PaletteState, mut state: Signal<AppState>) {
    match palette.mode {
        PaletteMode::Unified => {
            if let Some(result) = palette.unified_results.get(palette.selected_index) {
                match result {
                    SearchResult::File(file) => {
                        let path = file.path.clone();
                        spawn(async move {
                            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                                state.write().open_file(path, content);
                            }
                        });
                    }
                    SearchResult::Command { cmd, .. } => {
                        execute_command(cmd.id, &mut state.write());
                    }
                }
            }
        }
        PaletteMode::GoToLine => {
            if let Ok(line) = palette.query.trim_start_matches(':').trim().parse::<usize>() {
                tracing::info!("Go to line: {}", line);
            }
        }
        _ => {}
    }
}

/// Execute a command by ID
fn execute_command(id: &str, state: &mut AppState) {
    match id {
        "file.new" => state.new_tab(),
        "file.closeFolder" => state.close_folder(),
        "help.cookbook" => state.open_cookbook(),
        _ => tracing::info!("Command not implemented: {}", id),
    }
}

/// Unified search results - displays both files and commands
#[component]
fn UnifiedResults(selected_index: usize) -> Element {
    let mut palette = use_context::<Signal<PaletteState>>();
    let mut state = use_context::<Signal<AppState>>();
    let results = palette.read().unified_results.clone();

    if results.is_empty() {
        return rsx! {
            div { class: "palette-empty", "No results found" }
        };
    }

    rsx! {
        for (idx, result) in results.iter().enumerate() {
            {
                let class = if idx == selected_index {
                    "palette-item selected"
                } else {
                    "palette-item"
                };

                match result {
                    SearchResult::File(file) => {
                        let path = file.path.clone();
                        let name = file.name.clone();
                        let relative = file.relative_path.clone();

                        rsx! {
                            div {
                                key: "{path:?}",
                                class: "{class}",
                                onclick: move |_| {
                                    let p = path.clone();
                                    spawn(async move {
                                        if let Ok(content) = tokio::fs::read_to_string(&p).await {
                                            state.write().open_file(p, content);
                                        }
                                    });
                                    palette.write().visible = false;
                                },

                                span { class: "palette-file-icon", "" }
                                span { class: "palette-item-label", "{name}" }
                                span { class: "palette-item-path", "{relative}" }
                            }
                        }
                    }
                    SearchResult::Command { cmd, .. } => {
                        let cmd_id = cmd.id;
                        let label = cmd.label;
                        let category = cmd.category;
                        let shortcut = cmd.shortcut;

                        rsx! {
                            div {
                                key: "{cmd_id}",
                                class: "{class}",
                                onclick: move |_| {
                                    execute_command(cmd_id, &mut state.write());
                                    palette.write().visible = false;
                                },

                                span { class: "palette-cmd-icon", ">" }
                                span { class: "palette-item-label", "{label}" }
                                span { class: "palette-item-category", "{category}" }
                                if let Some(s) = shortcut {
                                    span { class: "palette-shortcut", "{s}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Go to line hint
#[component]
fn GoToLineHint(query: String) -> Element {
    let line_num: Result<usize, _> = query.parse();

    rsx! {
        div { class: "palette-hint",
            if let Ok(n) = line_num {
                "Press Enter to go to line {n}"
            } else if query.is_empty() {
                "Type a line number..."
            } else {
                "Invalid line number"
            }
        }
    }
}

/// Text search results (placeholder)
#[component]
fn TextSearchResults(query: String) -> Element {
    rsx! {
        div { class: "palette-hint",
            if query.is_empty() {
                "Type to search text in files..."
            } else {
                "Searching for \"{query}\"..."
            }
        }
    }
}
