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
use crate::state::AppStore;
use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    let state = use_context::<AppStore>();
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
                WindowTitle { state }
            }

            // Right side: Window controls
            div { class: "titlebar-right window-controls",
                button {
                    class: "titlebar-btn window-button",
                    title: "Minimize",
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
                button {
                    class: "titlebar-btn window-button",
                    title: "Maximize",
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
                button {
                    class: "titlebar-btn window-button close",
                    title: "Close",
                    onclick: move |_| window_close.close(),
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
        }
    }
}

/// VSCode-style top menu labels.
#[component]
fn MenuBar() -> Element {
    let state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let palette = use_context::<Signal<PaletteState>>();
    let mut active_menu = use_signal(|| None::<TopMenu>);

    let open_menu = *active_menu.read();
    let has_workspace = state.read().has_workspace();

    rsx! {
        nav {
            class: "menu-bar",
            aria_label: "Application menu",
            onmousedown: |e| e.stop_propagation(),

            if open_menu.is_some() {
                div {
                    class: "menu-backdrop",
                    onclick: move |_| active_menu.set(None),
                    onmousedown: |e| e.stop_propagation()
                }
            }

            for menu in TopMenu::ALL {
                {
                    let label = menu.label();
                    let item_class = if open_menu == Some(menu) {
                        "menu-item active"
                    } else {
                        "menu-item"
                    };

                    rsx! {
                        div {
                            key: "{label}",
                            class: "menu-root",
                            onmouseenter: move |_| {
                                if active_menu.read().is_some() {
                                    active_menu.set(Some(menu));
                                }
                            },

                            button {
                                class: "{item_class}",
                                onclick: move |_| {
                                    let next = if active_menu.read().as_ref() == Some(&menu) {
                                        None
                                    } else {
                                        Some(menu)
                                    };
                                    active_menu.set(next);
                                },
                                onmousedown: |e| {
                                    e.prevent_default();
                                    e.stop_propagation();
                                },
                                "{label}"
                            }

                            if open_menu == Some(menu) {
                                {menu_dropdown(
                                    menu,
                                    state,
                                    &services,
                                    palette,
                                    active_menu,
                                    has_workspace,
                                )}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn menu_dropdown(
    menu: TopMenu,
    state: AppStore,
    services: &AppServices,
    palette: Signal<PaletteState>,
    active_menu: Signal<Option<TopMenu>>,
    has_workspace: bool,
) -> Element {
    let services = AppServices::clone(services);

    rsx! {
        div {
            class: "menu-dropdown",
            onmousedown: |e| {
                e.prevent_default();
                e.stop_propagation();
            },
            match menu {
                TopMenu::File => rsx! {
                    {menu_action("New File", Some("Ctrl+N"), "file.new", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Open File", Some("Ctrl+O"), "file.open", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Open Folder", None, "file.openFolder", false, state, services.clone(), palette, active_menu)}
                    {menu_separator()}
                    {menu_action("Save", Some("Ctrl+S"), "file.save", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Save As", Some("Ctrl+Shift+S"), "file.saveAs", false, state, services.clone(), palette, active_menu)}
                    {menu_separator()}
                    {menu_action("New Window", None, "window.new", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Close Folder", None, "file.closeFolder", !has_workspace, state, services.clone(), palette, active_menu)}
                },
                TopMenu::Edit => rsx! {
                    {menu_action("Undo", Some("Ctrl+Z"), "edit.undo", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Redo", Some("Ctrl+Shift+Z"), "edit.redo", false, state, services.clone(), palette, active_menu)}
                    {menu_separator()}
                    {menu_action("Cut", Some("Ctrl+X"), "edit.cut", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Copy", Some("Ctrl+C"), "edit.copy", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Paste", Some("Ctrl+V"), "edit.paste", false, state, services.clone(), palette, active_menu)}
                },
                TopMenu::Selection => rsx! {
                    {menu_action("Select All", Some("Ctrl+A"), "edit.selectAll", false, state, services.clone(), palette, active_menu)}
                },
                TopMenu::View => rsx! {
                    {menu_action("Command Palette", Some("Ctrl+Shift+P"), "view.commandPalette", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Settings", None, "view.settings", false, state, services.clone(), palette, active_menu)}
                },
                TopMenu::Go => rsx! {
                    {menu_action("Go to File", Some("Ctrl+P"), "view.goToFile", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Go to Line", Some("Ctrl+G"), "view.goToLine", false, state, services.clone(), palette, active_menu)}
                },
                TopMenu::Preview => rsx! {
                    {menu_action("Open/Refresh Preview", Some("F5"), "preview.run", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Pop Out Preview", None, "preview.popOut", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Stop Preview", Some("Shift+F5"), "preview.stop", false, state, services.clone(), palette, active_menu)}
                },
                TopMenu::Terminal => rsx! {
                    {menu_action("Toggle Terminal", Some("Ctrl+`"), "terminal.toggle", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Clear Terminal", None, "terminal.clear", false, state, services.clone(), palette, active_menu)}
                },
                TopMenu::Help => rsx! {
                    {menu_action("Open Cookbook", None, "help.cookbook", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Open README", None, "help.readme", false, state, services.clone(), palette, active_menu)}
                    {menu_action("Open Documentation", Some("F1"), "help.documentation", false, state, services.clone(), palette, active_menu)}
                    {menu_separator()}
                    {menu_action("About Soyuz Studio", None, "help.about", false, state, services.clone(), palette, active_menu)}
                },
            }
        }
    }
}

fn menu_action(
    label: &'static str,
    shortcut: Option<&'static str>,
    command_id: &'static str,
    disabled: bool,
    state: AppStore,
    services: AppServices,
    palette: Signal<PaletteState>,
    mut active_menu: Signal<Option<TopMenu>>,
) -> Element {
    let class = if disabled {
        "menu-action disabled"
    } else {
        "menu-action"
    };

    rsx! {
        button {
            class: "{class}",
            disabled,
            onclick: move |_| {
                if disabled {
                    return;
                }
                app_commands::execute_app_command(command_id, state, services.clone(), palette);
                active_menu.set(None);
            },
            onmousedown: |e| {
                e.prevent_default();
                e.stop_propagation();
            },

            span { class: "menu-label", "{label}" }
            if let Some(shortcut) = shortcut {
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
fn WindowTitle(state: AppStore) -> Element {
    let palette = use_context::<Signal<PaletteState>>();

    // Get workspace name for display
    let workspace_name = state
        .read()
        .workspace
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
