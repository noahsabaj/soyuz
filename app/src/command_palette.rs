//! Command Palette - VS Code style quick search and command runner

// map_or_else is less readable for Option/Result chains
#![allow(clippy::map_unwrap_or)]
// Borrowed format strings are valid
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::state::AppState;
use std::path::Path;
use dioxus::prelude::*;
use std::path::PathBuf;

/// Search mode based on input prefix
#[derive(Clone, Copy, PartialEq, Default)]
pub enum PaletteMode {
    #[default]
    Files,      // Default - search files
    Commands,   // > prefix - run commands
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
    pub score: i32, // For sorting by relevance
}

/// Command palette state - shared via context
#[derive(Clone, Default)]
pub struct PaletteState {
    pub visible: bool,
    pub query: String,
    pub mode: PaletteMode,
    pub selected_index: usize,
    pub file_results: Vec<FileResult>,
    pub filtered_commands: Vec<Command>,
}

/// All available commands in Soyuz Studio
pub fn get_all_commands() -> Vec<Command> {
    vec![
        // File operations
        Command { id: "file.new", label: "New File", shortcut: Some("Ctrl+N"), category: "File" },
        Command { id: "file.open", label: "Open File...", shortcut: Some("Ctrl+O"), category: "File" },
        Command { id: "file.save", label: "Save", shortcut: Some("Ctrl+S"), category: "File" },
        Command { id: "file.saveAs", label: "Save As...", shortcut: Some("Ctrl+Shift+S"), category: "File" },
        Command { id: "file.openFolder", label: "Open Folder...", shortcut: None, category: "File" },
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
        Command { id: "view.goToLine", label: "Go to Line...", shortcut: Some("Ctrl+G"), category: "View" },

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
        Command { id: "help.documentation", label: "Open Documentation", shortcut: Some("F1"), category: "Help" },
        Command { id: "help.about", label: "About Soyuz Studio", shortcut: None, category: "Help" },
    ]
}

/// Filter commands based on query
pub fn filter_commands(query: &str) -> Vec<Command> {
    let query_lower = query.to_lowercase();
    let all_commands = get_all_commands();

    if query_lower.is_empty() {
        return all_commands;
    }

    all_commands
        .into_iter()
        .filter(|cmd| {
            cmd.label.to_lowercase().contains(&query_lower)
                || cmd.category.to_lowercase().contains(&query_lower)
                || cmd.id.to_lowercase().contains(&query_lower)
        })
        .collect()
}

/// Fuzzy match score for file search
fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();

    if query_lower.is_empty() {
        return Some(0);
    }

    // Simple substring match for now
    if target_lower.contains(&query_lower) {
        // Exact match at start scores highest
        if target_lower.starts_with(&query_lower) {
            return Some(100);
        }
        // Contains match
        return Some(50);
    }

    // Check if all characters appear in order (fuzzy)
    let mut query_chars = query_lower.chars().peekable();
    let mut score = 0;
    let mut consecutive = 0;

    for c in target_lower.chars() {
        if query_chars.peek() == Some(&c) {
            query_chars.next();
            consecutive += 1;
            score += consecutive * 10; // Bonus for consecutive matches
        } else {
            consecutive = 0;
        }
    }

    if query_chars.peek().is_none() {
        Some(score)
    } else {
        None // Not all characters matched
    }
}

/// Search files in workspace
pub async fn search_files(workspace: &Path, query: &str) -> Vec<FileResult> {
    let mut results = Vec::new();
    let query = query.to_lowercase();

    // Recursively walk directory
    if let Ok(entries) = walk_directory(workspace, workspace).await {
        for (path, relative) in entries {
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Score against filename and path
            let name_score = fuzzy_score(&query, &name);
            let path_score = fuzzy_score(&query, &relative);

            if name_score.or(path_score).is_some() {
                results.push(FileResult {
                    path,
                    name,
                    relative_path: relative,
                    score: name_score.unwrap_or(0).max(path_score.unwrap_or(0)),
                });
            }
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.cmp(&a.score));

    // Limit results
    results.truncate(50);

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
                    let relative = path.strip_prefix(base)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string());
                    results.push((path, relative));
                }
            }
        }
    }

    Ok(results)
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

        // Trigger search based on mode
        match mode {
            PaletteMode::Commands => {
                palette.write().filtered_commands = filter_commands(search_term);
            }
            PaletteMode::Files => {
                // File search is async, handled separately
                let workspace = state.read().workspace.clone();
                if let Some(ws) = workspace {
                    let search_term = search_term.to_string();
                    spawn(async move {
                        let results = search_files(&ws, &search_term).await;
                        palette.write().file_results = results;
                    });
                }
            }
            _ => {}
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
                    palette.write().selected_index = if current == 0 { max_items - 1 } else { current - 1 };
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
        PaletteMode::Files => "Search files by name (prefix: > commands, : line, % text, @ symbols)",
        PaletteMode::Commands => "Type command name...",
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
                    // Use onmounted for reliable focus when palette opens
                    onmounted: move |evt| {
                        spawn(async move {
                            let _ = evt.set_focus(true).await;
                        });
                    },
                    oninput: on_input,
                    onkeydown: on_keydown
                }
            }

            // Quick actions (only in file mode with empty query)
            if effective_mode == PaletteMode::Files && search_query.is_empty() {
                div { class: "palette-quick-actions",
                    QuickAction { label: "Go to File", shortcut: "Ctrl+P", selected: selected_index == 0 }
                    QuickAction { label: "Show and Run Commands", shortcut: "Ctrl+Shift+P", prefix: ">", selected: selected_index == 1 }
                    QuickAction { label: "Go to Line", shortcut: "Ctrl+G", prefix: ":", selected: selected_index == 2 }
                    QuickAction { label: "Search for Text", prefix: "%", selected: selected_index == 3 }
                    QuickAction { label: "Go to Symbol in Editor", shortcut: "Ctrl+Shift+O", prefix: "@", selected: selected_index == 4 }
                }
            }

            // Results list
            div { class: "palette-results",
                match effective_mode {
                    PaletteMode::Files if !search_query.is_empty() => rsx! {
                        FileResults { selected_index }
                    },
                    PaletteMode::Commands => rsx! {
                        CommandResults { selected_index }
                    },
                    PaletteMode::GoToLine => rsx! {
                        GoToLineHint { query: search_query.to_string() }
                    },
                    PaletteMode::TextSearch => rsx! {
                        TextSearchResults { query: search_query.to_string(), selected_index }
                    },
                    _ => rsx! {
                        RecentFiles { selected_index, offset: 5 }
                    }
                }
            }
        }
    }
}

/// Parse query to determine mode and extract search term
fn parse_query(query: &str) -> (PaletteMode, &str) {
    if let Some(rest) = query.strip_prefix('>') {
        (PaletteMode::Commands, rest.trim())
    } else if let Some(rest) = query.strip_prefix(':') {
        (PaletteMode::GoToLine, rest.trim())
    } else if let Some(rest) = query.strip_prefix('%') {
        (PaletteMode::TextSearch, rest.trim())
    } else if let Some(rest) = query.strip_prefix('#') {
        (PaletteMode::TextSearch, rest.trim())
    } else if let Some(rest) = query.strip_prefix('@') {
        (PaletteMode::Symbols, rest.trim())
    } else {
        (PaletteMode::Files, query.trim())
    }
}

/// Get total result count for keyboard navigation
fn get_result_count(palette: &PaletteState) -> usize {
    match palette.mode {
        PaletteMode::Files if palette.query.is_empty() => 5, // Quick actions
        PaletteMode::Files => palette.file_results.len(),
        PaletteMode::Commands => palette.filtered_commands.len(),
        _ => 0,
    }
}

/// Execute the selected item
fn execute_selected(palette: &PaletteState, mut state: Signal<AppState>) {
    match palette.mode {
        PaletteMode::Files if palette.query.is_empty() => {
            // Quick action selected - this is handled by updating the query
        }
        PaletteMode::Files => {
            if let Some(file) = palette.file_results.get(palette.selected_index) {
                let path = file.path.clone();
                spawn(async move {
                    if let Ok(content) = tokio::fs::read_to_string(&path).await {
                        state.write().open_file(path, content);
                    }
                });
            }
        }
        PaletteMode::Commands => {
            if let Some(cmd) = palette.filtered_commands.get(palette.selected_index) {
                execute_command(cmd.id, &mut state.write());
            }
        }
        PaletteMode::GoToLine => {
            if let Ok(line) = palette.query.trim_start_matches(':').trim().parse::<usize>() {
                // Go to line - this would need editor integration
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
        // Add more command implementations as needed
        _ => tracing::info!("Command not implemented: {}", id),
    }
}

/// Quick action item
#[component]
fn QuickAction(label: String, shortcut: Option<String>, prefix: Option<String>, selected: bool) -> Element {
    let class = if selected { "palette-item selected" } else { "palette-item" };

    rsx! {
        div { class: "{class}",
            span { class: "palette-item-label",
                if let Some(p) = prefix {
                    span { class: "palette-prefix", "{p} " }
                }
                "{label}"
            }
            if let Some(s) = shortcut {
                span { class: "palette-shortcut", "{s}" }
            }
        }
    }
}

/// File search results
#[component]
fn FileResults(selected_index: usize) -> Element {
    let mut palette = use_context::<Signal<PaletteState>>();
    let mut state = use_context::<Signal<AppState>>();
    let results = palette.read().file_results.clone();

    if results.is_empty() {
        return rsx! {
            div { class: "palette-empty", "No files found" }
        };
    }

    rsx! {
        for (idx, file) in results.iter().enumerate() {
            {
                let class = if idx == selected_index { "palette-item selected" } else { "palette-item" };
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
        }
    }
}

/// Command search results
#[component]
fn CommandResults(selected_index: usize) -> Element {
    let mut palette = use_context::<Signal<PaletteState>>();
    let mut state = use_context::<Signal<AppState>>();
    let commands = palette.read().filtered_commands.clone();

    if commands.is_empty() {
        return rsx! {
            div { class: "palette-empty", "No commands found" }
        };
    }

    rsx! {
        for (idx, cmd) in commands.iter().enumerate() {
            {
                let class = if idx == selected_index { "palette-item selected" } else { "palette-item" };
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

/// Recent files list
#[component]
fn RecentFiles(selected_index: usize, offset: usize) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut palette = use_context::<Signal<PaletteState>>();

    let recent = state.read().recent_files.clone();

    if recent.is_empty() {
        return rsx! {
            div { class: "palette-section-header", "recently opened" }
            div { class: "palette-empty", "No recent files" }
        };
    }

    rsx! {
        div { class: "palette-section-header", "recently opened" }
        for (idx, path) in recent.iter().enumerate() {
            {
                let adjusted_idx = idx + offset;
                let class = if adjusted_idx == selected_index { "palette-item selected" } else { "palette-item" };
                let p = path.clone();
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                let display_path = path.to_string_lossy().to_string();

                rsx! {
                    div {
                        key: "{display_path}",
                        class: "{class}",
                        onclick: move |_| {
                            let path = p.clone();
                            spawn(async move {
                                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                                    state.write().open_file(path, content);
                                }
                            });
                            palette.write().visible = false;
                        },

                        span { class: "palette-file-icon", "" }
                        span { class: "palette-item-label", "{name}" }
                        span { class: "palette-item-path", "{display_path}" }
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
fn TextSearchResults(query: String, selected_index: usize) -> Element {
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
