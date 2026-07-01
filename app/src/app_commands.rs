//! Shared application command execution.
//!
//! The command palette and top menu both route through this module so behavior
//! does not drift between entry points.

use crate::command_palette::{PaletteMode, PaletteState, unified_search};
use crate::services::AppServices;
use crate::state::{AppStore, TerminalLevel};
use crate::{export, js_interop, preview};
use dioxus::prelude::*;
use std::path::Path;

/// Open the unified command palette, optionally prefilled with a query.
pub fn open_unified_palette(
    mut palette: Signal<PaletteState>,
    state: AppStore,
    query: impl Into<String>,
) {
    let query = query.into();
    // F74: claim a fresh search generation so a slower in-flight keystroke search
    // cannot land after and overwrite the results for the palette opened here.
    let generation = {
        let mut palette_state = palette.write();
        palette_state.visible = true;
        palette_state.query.clone_from(&query);
        palette_state.mode = PaletteMode::Unified;
        palette_state.selected_index = 0;
        palette_state.search_generation = palette_state.search_generation.wrapping_add(1);
        palette_state.search_generation
    };

    let workspace = state.peek().workspace.clone();
    spawn(async move {
        let results = unified_search(workspace.as_deref(), &query).await;
        if palette.read().search_generation == generation {
            palette.write().unified_results = results;
        }
    });
}

/// Open the go-to-line command palette mode.
pub fn open_go_to_line_palette(mut palette: Signal<PaletteState>) {
    let mut palette_state = palette.write();
    palette_state.visible = true;
    palette_state.query = ":".to_string();
    palette_state.mode = PaletteMode::GoToLine;
    palette_state.selected_index = 0;
}

/// Execute an application command by stable command id.
#[allow(clippy::too_many_lines)]
pub fn execute_app_command(
    id: &str,
    mut state: AppStore,
    services: AppServices,
    palette: Signal<PaletteState>,
) {
    match id {
        "file.new" => state.write().new_tab(),
        "file.open" => open_file_dialog(state, services),
        "file.openFolder" => open_folder_dialog(state, services),
        "file.save" => save_current_file(state, services, false),
        "file.saveAs" => save_current_file(state, services, true),
        "file.closeFolder" => state.write().close_folder(),
        "window.new" => spawn_new_window(&services),

        "edit.undo" => undo(state),
        "edit.redo" => redo(state),
        "edit.cut" => exec_document_command("cut"),
        "edit.copy" => exec_document_command("copy"),
        "edit.paste" => exec_document_command("paste"),
        "edit.selectAll" => exec_document_command("selectAll"),

        "view.commandPalette" | "view.goToFile" => open_unified_palette(palette, state, ""),
        "view.goToLine" => open_go_to_line_palette(palette),
        "view.settings" => state.write().open_settings(),

        "preview.run" => preview::open_docked_preview(state, services),
        "preview.popOut" => preview::pop_out_preview(state, services),
        "preview.stop" => preview::stop_preview(state, &services),

        "export.obj" | "export.stl" | "export.gltf" | "export.open" => {
            export::open_export_panel(state);
        }

        "terminal.toggle" => state.write().toggle_terminal(),
        "terminal.clear" => services.terminal_clear(),

        "help.documentation" => state.write().open_documentation(),
        "help.cookbook" => state.write().open_cookbook(),
        "help.readme" => state.write().open_readme(),
        "help.about" => state.write().open_about(),

        _ => services.terminal_log(
            TerminalLevel::Warn,
            format!("Command is not implemented yet: {id}"),
        ),
    }
}

fn open_file_dialog(mut state: AppStore, services: AppServices) {
    spawn(async move {
        let Some(path) = rfd::AsyncFileDialog::new()
            .add_filter("Rhai Scripts", &["rhai"])
            .pick_file()
            .await
        else {
            return;
        };

        match tokio::fs::read_to_string(path.path()).await {
            Ok(content) => state.write().open_file(path.path().to_path_buf(), content),
            Err(error) => services.terminal_log(
                TerminalLevel::Error,
                format!("Failed to open {}: {error}", path.path().display()),
            ),
        }
    });
}

fn open_folder_dialog(mut state: AppStore, services: AppServices) {
    spawn(async move {
        let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
            return;
        };

        let path = folder.path().to_path_buf();
        services.terminal_log(
            TerminalLevel::Info,
            format!("Opened folder {}", path.display()),
        );
        state.write().open_folder(path);
    });
}

fn save_current_file(state: AppStore, services: AppServices, force_dialog: bool) {
    if !force_dialog && let Some(path) = state.peek().current_file() {
        let code = state.peek().code();
        write_file(state, services, path, code);
        return;
    }

    spawn(async move {
        let Some(path) = rfd::AsyncFileDialog::new()
            .add_filter("Rhai Scripts", &["rhai"])
            .set_file_name("untitled.rhai")
            .save_file()
            .await
        else {
            return;
        };

        let code = state.peek().code();
        write_file(state, services, path.path(), code);
    });
}

fn write_file(mut state: AppStore, services: AppServices, path: impl AsRef<Path>, code: String) {
    let path = path.as_ref().to_path_buf();

    // F56: a split clones the active tab into an independent buffer that keeps the
    // same path, so saving one pane can silently clobber unsaved edits made in the
    // other. A shared-buffer refactor is too invasive here, so at least warn when
    // another open tab targets the same file instead of overwriting silently.
    let duplicate_open = {
        let s = state.peek();
        let active_id = s.active_tab().map(|t| t.id);
        s.all_tabs()
            .iter()
            .any(|t| t.path.as_deref() == Some(path.as_path()) && Some(t.id) != active_id)
    };
    if duplicate_open {
        services.terminal_log(
            TerminalLevel::Warn,
            format!(
                "{} is open in more than one pane; this save may overwrite unsaved changes in the other copy.",
                path.display()
            ),
        );
    }

    spawn(async move {
        match tokio::fs::write(&path, &code).await {
            Ok(()) => {
                state.write().mark_saved(Some(path.clone()));
                services.terminal_log(TerminalLevel::Info, format!("Saved {}", path.display()));
            }
            Err(error) => services.terminal_log(
                TerminalLevel::Error,
                format!("Failed to save {}: {error}", path.display()),
            ),
        }
    });
}

fn undo(mut state: AppStore) {
    let pane_id = state.peek().focused_pane_id;
    if let Some((new_content, cursor_pos)) = state.write().undo() {
        spawn(async move {
            js_interop::set_editor_content(pane_id, &new_content, cursor_pos).await;
        });
    }
}

fn redo(mut state: AppStore) {
    let pane_id = state.peek().focused_pane_id;
    if let Some((new_content, cursor_pos)) = state.write().redo() {
        spawn(async move {
            js_interop::set_editor_content(pane_id, &new_content, cursor_pos).await;
        });
    }
}

fn exec_document_command(command: &'static str) {
    spawn(async move {
        js_interop::exec_document_command(command).await;
    });
}

fn spawn_new_window(services: &AppServices) {
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(error) = std::process::Command::new(exe).arg("--fresh").spawn() {
                services.terminal_log(
                    TerminalLevel::Error,
                    format!("Failed to open new window: {error}"),
                );
            }
        }
        Err(error) => services.terminal_log(
            TerminalLevel::Error,
            format!("Failed to locate current executable: {error}"),
        ),
    }
}
