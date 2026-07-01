//! Top toolbar component with file operations and window controls

// Separate if statements are clearer for async dialog handling
#![allow(clippy::collapsible_if)]
// map_or_else is less readable for UI state
#![allow(clippy::map_unwrap_or)]
// Borrowed format strings are valid
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::app_commands;
use crate::assets::APP_ICON_32;
use crate::command_palette::PaletteState;
use crate::services::AppServices;
use crate::state::{AppStateStoreExt, AppStore};
use dioxus::prelude::*;

#[derive(Clone, Copy)]
enum TopMenu {
    File,
    Edit,
    Selection,
    View,
    Go,
    Preview,
    Terminal,
    Help,
}

impl TopMenu {
    const ALL: [Self; 8] = [
        Self::File,
        Self::Edit,
        Self::Selection,
        Self::View,
        Self::Go,
        Self::Preview,
        Self::Terminal,
        Self::Help,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Selection => "Selection",
            Self::View => "View",
            Self::Go => "Go",
            Self::Preview => "Preview",
            Self::Terminal => "Terminal",
            Self::Help => "Help",
        }
    }
}

/// Application logo in the toolbar
#[component]
fn AppLogo() -> Element {
    rsx! {
        div {
            class: "app-logo",
            onmousedown: |e| e.stop_propagation(),
            img {
                src: APP_ICON_32,
                alt: "Soyuz Studio",
                width: "20",
                height: "20"
            }
        }
    }
}

/// Top toolbar with file operations and window controls
#[component]
pub fn Toolbar() -> Element {
    use dioxus_primitives::ContentSide;
    use dioxus_primitives::tooltip::{Tooltip, TooltipContent, TooltipTrigger};

    let state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let window = dioxus::desktop::use_window();

    // Clone window for each closure that needs it
    let window_drag = window.clone();
    let window_dblclick = window.clone();
    let window_min = window.clone();
    let window_max = window.clone();
    let window_close = window.clone();

    rsx! {
        div {
            class: "titlebar",
            onmousedown: move |_| { window_drag.drag(); },
            ondoubleclick: move |_| { window_dblclick.set_maximized(!window_dblclick.is_maximized()); },

            // Left side: Logo, file operations and preview controls
            div { class: "titlebar-left",
                AppLogo {}
                MenuBar {}
            }

            // Center: Search bar (fills available space, centers content)
            div { class: "titlebar-center",
                WindowTitle {}
            }

            // Right side: Window controls
            div { class: "titlebar-right window-controls",
                Tooltip { class: "titlebar-tooltip",
                    TooltipTrigger { class: "titlebar-tooltip-trigger",
                        button {
                            class: "titlebar-btn window-button",
                            r#type: "button",
                            aria_label: "Minimize",
                            onclick: move |_| window_min.set_minimized(true),
                            onmousedown: |e| e.stop_propagation(),
                            // Minimize icon: horizontal line
                            svg {
                                width: "10",
                                height: "10",
                                view_box: "0 0 10 10",
                                path {
                                    d: "M0 5L10 5",
                                    stroke: "currentColor",
                                    stroke_width: "1.2"
                                }
                            }
                        }
                    }
                    TooltipContent {
                        class: "titlebar-tooltip-content",
                        side: ContentSide::Bottom,
                        "Minimize"
                    }
                }
                Tooltip { class: "titlebar-tooltip",
                    TooltipTrigger { class: "titlebar-tooltip-trigger",
                        button {
                            class: "titlebar-btn window-button",
                            r#type: "button",
                            aria_label: "Maximize",
                            onclick: {
                                let window_max = window_max.clone();
                                move |_| window_max.set_maximized(!window_max.is_maximized())
                            },
                            onmousedown: |e| e.stop_propagation(),
                            // Maximize icon: square outline
                            svg {
                                width: "10",
                                height: "10",
                                view_box: "0 0 10 10",
                                rect {
                                    x: "0.5",
                                    y: "0.5",
                                    width: "9",
                                    height: "9",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1.2"
                                }
                            }
                        }
                    }
                    TooltipContent {
                        class: "titlebar-tooltip-content",
                        side: ContentSide::Bottom,
                        "Maximize"
                    }
                }
                Tooltip { class: "titlebar-tooltip",
                    TooltipTrigger { class: "titlebar-tooltip-trigger",
                        button {
                            class: "titlebar-btn window-button close",
                            r#type: "button",
                            aria_label: "Close",
                            onclick: {
                                let services = services.clone();
                                move |_| {
                                    // F60: persist the session before tearing the window down,
                                    // so edits made within the 30s auto-save window are not lost.
                                    let session = crate::session::state_to_session(&state.read());
                                    if let Err(e) = session.save() {
                                        tracing::warn!("Failed to save session on exit: {e}");
                                    }
                                    // Persist the window geometry so the next launch restores
                                    // the same size / position / maximized state (best-effort).
                                    let size = window_close.inner_size();
                                    let (x, y) = window_close
                                        .outer_position()
                                        .map(|p| (p.x, p.y))
                                        .unwrap_or((0, 0));
                                    crate::save_window_geometry(
                                        size.width,
                                        size.height,
                                        x,
                                        y,
                                        window_close.is_maximized(),
                                    );
                                    // Kill the preview child before tearing down the window;
                                    // std::process::Child is not killed on drop.
                                    services.stop_preview_process();
                                    window_close.close();
                                }
                            },
                            onmousedown: |e| e.stop_propagation(),
                            // Close icon: X shape
                            svg {
                                width: "10",
                                height: "10",
                                view_box: "0 0 10 10",
                                path {
                                    d: "M0 0L10 10M10 0L0 10",
                                    stroke: "currentColor",
                                    stroke_width: "1.2"
                                }
                            }
                        }
                    }
                    TooltipContent {
                        class: "titlebar-tooltip-content",
                        side: ContentSide::Bottom,
                        "Close"
                    }
                }
            }
        }
    }
}

/// VSCode-style top menu bar, built on the primitives' [`Menubar`] so it gets
/// ARIA roles, roving keyboard navigation, and outside-click dismissal for free
/// (replacing the former hand-rolled `active_menu` context + backdrop).
#[component]
fn MenuBar() -> Element {
    use dioxus_primitives::menubar::{Menubar, MenubarContent, MenubarMenu, MenubarTrigger};

    let state = use_context::<AppStore>();
    // F73: subscribe only to the workspace field, not the whole store.
    let has_workspace = state.workspace().read().is_some();

    rsx! {
        // The `Menubar` primitive extends only global attributes (no event
        // listeners), so the drag-stop lives on this wrapping landmark: menu
        // clicks must not start a window drag (the titlebar drags on mousedown).
        nav {
            class: "menu-bar",
            aria_label: "Application menu",
            onmousedown: |e| e.stop_propagation(),

            Menubar { class: "menu-bar-list",
                for (menu_index, menu) in TopMenu::ALL.into_iter().enumerate() {
                    MenubarMenu {
                        key: "{menu.label()}",
                        class: "menu-root",
                        index: menu_index,

                        MenubarTrigger { class: "menu-item", "{menu.label()}" }
                        MenubarContent { class: "menu-dropdown",
                            {menu_dropdown(menu, has_workspace)}
                        }
                    }
                }
            }
        }
    }
}

/// Renders the items of `menu` as [`MenuAction`]s interleaved with separators.
/// Item `index` values (used by the primitive for roving keyboard focus) count
/// only actions, skipping the visual separators.
fn menu_dropdown(menu: TopMenu, has_workspace: bool) -> Element {
    match menu {
        TopMenu::File => rsx! {
            MenuAction { index: 0, label: "New File", shortcut: "Ctrl+N", command_id: "file.new" }
            MenuAction { index: 1, label: "Open File", shortcut: "Ctrl+O", command_id: "file.open" }
            MenuAction { index: 2, label: "Open Folder", command_id: "file.openFolder" }
            {menu_separator()}
            MenuAction { index: 3, label: "Save", shortcut: "Ctrl+S", command_id: "file.save" }
            MenuAction { index: 4, label: "Save As", shortcut: "Ctrl+Shift+S", command_id: "file.saveAs" }
            {menu_separator()}
            MenuAction { index: 5, label: "New Window", command_id: "window.new" }
            MenuAction { index: 6, label: "Close Folder", command_id: "file.closeFolder", disabled: !has_workspace }
        },
        TopMenu::Edit => rsx! {
            MenuAction { index: 0, label: "Undo", shortcut: "Ctrl+Z", command_id: "edit.undo" }
            MenuAction { index: 1, label: "Redo", shortcut: "Ctrl+Shift+Z", command_id: "edit.redo" }
            {menu_separator()}
            MenuAction { index: 2, label: "Cut", shortcut: "Ctrl+X", command_id: "edit.cut" }
            MenuAction { index: 3, label: "Copy", shortcut: "Ctrl+C", command_id: "edit.copy" }
            MenuAction { index: 4, label: "Paste", shortcut: "Ctrl+V", command_id: "edit.paste" }
        },
        TopMenu::Selection => rsx! {
            MenuAction { index: 0, label: "Select All", shortcut: "Ctrl+A", command_id: "edit.selectAll" }
        },
        TopMenu::View => rsx! {
            MenuAction { index: 0, label: "Command Palette", shortcut: "Ctrl+Shift+P", command_id: "view.commandPalette" }
            MenuAction { index: 1, label: "Settings", command_id: "view.settings" }
        },
        TopMenu::Go => rsx! {
            MenuAction { index: 0, label: "Go to File", shortcut: "Ctrl+P", command_id: "view.goToFile" }
            MenuAction { index: 1, label: "Go to Line", shortcut: "Ctrl+G", command_id: "view.goToLine" }
        },
        TopMenu::Preview => rsx! {
            MenuAction { index: 0, label: "Open/Refresh Preview", shortcut: "F5", command_id: "preview.run" }
            MenuAction { index: 1, label: "Pop Out Preview", command_id: "preview.popOut" }
            MenuAction { index: 2, label: "Stop Preview", shortcut: "Shift+F5", command_id: "preview.stop" }
        },
        TopMenu::Terminal => rsx! {
            MenuAction { index: 0, label: "Toggle Terminal", shortcut: "Ctrl+`", command_id: "terminal.toggle" }
            MenuAction { index: 1, label: "Clear Terminal", command_id: "terminal.clear" }
        },
        TopMenu::Help => rsx! {
            MenuAction { index: 0, label: "Open Cookbook", command_id: "help.cookbook" }
            MenuAction { index: 1, label: "Open README", command_id: "help.readme" }
            MenuAction { index: 2, label: "Open Documentation", shortcut: "F1", command_id: "help.documentation" }
            {menu_separator()}
            MenuAction { index: 3, label: "About Soyuz Studio", command_id: "help.about" }
        },
    }
}

/// A single dropdown menu item, built on the primitives' [`MenubarItem`] so it
/// inherits the menu ARIA role, roving focus, and automatic dismissal on select.
/// Pulls `state` / `services` / `palette` from context instead of taking them as
/// positional arguments; `command_id` doubles as the item's `value`.
#[component]
fn MenuAction(
    index: usize,
    #[props(into)] label: String,
    shortcut: Option<String>,
    #[props(into)] command_id: String,
    #[props(default)] disabled: bool,
) -> Element {
    use dioxus_primitives::menubar::MenubarItem;

    let state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let palette = use_context::<Signal<PaletteState>>();

    rsx! {
        MenubarItem {
            class: "menu-action",
            index,
            value: command_id.clone(),
            disabled,
            // The primitive already skips disabled items and closes the menu on select.
            on_select: move |_| {
                app_commands::execute_app_command(&command_id, state, services.clone(), palette);
            },

            span { class: "menu-label", "{label}" }
            if let Some(shortcut) = shortcut.as_ref() {
                span { class: "menu-shortcut", "{shortcut}" }
            }
        }
    }
}

fn menu_separator() -> Element {
    rsx! {
        div { class: "menu-separator" }
    }
}

/// Search bar in toolbar - opens command palette when clicked
#[component]
fn WindowTitle() -> Element {
    let state = use_context::<AppStore>();
    let palette = use_context::<Signal<PaletteState>>();

    // Get workspace name for display (F73: subscribe only to the workspace field).
    let workspace_name = state
        .workspace()
        .read()
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Soyuz Studio".to_string());

    let open_palette = move |_| {
        app_commands::open_unified_palette(palette, state, "");
    };

    rsx! {
        div {
            class: "search-bar",
            onclick: open_palette,
            onmousedown: |e| e.stop_propagation(), // Don't drag window

            span { class: "search-icon", "" }
            span { class: "search-placeholder", "{workspace_name}" }
        }
    }
}
