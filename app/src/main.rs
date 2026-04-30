//! Soyuz Studio - Desktop application for procedural asset generation

mod app_commands;
mod assets;
mod browser;
mod command_palette;
mod docs_generated;
mod export;
mod js_interop;
mod markdown_panel;
mod pane;
mod preview;
mod preview_cli;
mod services;
mod session;
mod settings;
mod settings_panel;
mod state;
mod statusbar;
mod terminal;
mod toolbar;

use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use preview_cli::LaunchMode;
use services::AppServices;
use state::{AppState, TerminalBuffer};
use std::process::ExitCode;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Panel-level error fallback component
#[component]
fn PanelError(panel_name: String, error_msg: String) -> Element {
    rsx! {
        div { class: "error-panel",
            div { class: "error-panel-icon", "!" }
            h3 { "{panel_name} Error" }
            p { "An error occurred in this panel." }
            pre { "{error_msg}" }
        }
    }
}

#[component]
fn ActivityBar() -> Element {
    let mut state = use_context::<state::AppStore>();
    let mut palette = use_context::<Signal<command_palette::PaletteState>>();
    let services = use_context::<AppServices>();
    let (preview_active, export_active, terminal_visible) = {
        let state_ref = state.read();
        let active_tab = state_ref.active_tab();
        (
            active_tab.is_some_and(|tab| tab.is_preview()),
            active_tab.is_some_and(|tab| tab.is_export()),
            state_ref.terminal_visible,
        )
    };

    rsx! {
        div { class: "activity-bar",
            button {
                class: "activity-item active",
                aria_label: "Explorer",
                title: "Explorer",
                svg {
                    class: "activity-icon",
                    width: "22",
                    height: "22",
                    view_box: "0 0 24 24",
                    fill: "none",
                    path {
                        d: "M3.5 5.5A2.5 2.5 0 0 1 6 3h5l2 2h5A2.5 2.5 0 0 1 20.5 7.5v9A2.5 2.5 0 0 1 18 19H6a2.5 2.5 0 0 1-2.5-2.5v-11Z",
                        stroke: "currentColor",
                        stroke_width: "1.7",
                        stroke_linecap: "round",
                        stroke_linejoin: "round"
                    }
                }
            }
            button {
                class: "activity-item",
                aria_label: "Command Palette",
                title: "Command Palette",
                onclick: move |_| {
                    palette.write().visible = true;
                    palette.write().query.clear();
                },
                svg {
                    class: "activity-icon",
                    width: "22",
                    height: "22",
                    view_box: "0 0 24 24",
                    fill: "none",
                    path {
                        d: "m6 8 4 4-4 4",
                        stroke: "currentColor",
                        stroke_width: "1.9",
                        stroke_linecap: "round",
                        stroke_linejoin: "round"
                    }
                    path {
                        d: "M12 17h6",
                        stroke: "currentColor",
                        stroke_width: "1.9",
                        stroke_linecap: "round"
                    }
                }
            }
            button {
                class: if preview_active { "activity-item active" } else { "activity-item" },
                aria_label: "Preview",
                title: "Preview",
                onclick: {
                    let services = services.clone();
                    move |_| crate::preview::open_docked_preview(state, services.clone())
                },
                svg {
                    class: "activity-icon",
                    width: "22",
                    height: "22",
                    view_box: "0 0 24 24",
                    fill: "none",
                    path {
                        d: "M8 5v14l11-7L8 5Z",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round",
                        stroke_linejoin: "round"
                    }
                }
            }
            button {
                class: if export_active { "activity-item active" } else { "activity-item" },
                aria_label: "Export",
                title: "Export",
                onclick: move |_| crate::export::open_export_panel(state),
                svg {
                    class: "activity-icon",
                    width: "22",
                    height: "22",
                    view_box: "0 0 24 24",
                    fill: "none",
                    path {
                        d: "M12 3v11",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round"
                    }
                    path {
                        d: "m7 9 5 5 5-5",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round",
                        stroke_linejoin: "round"
                    }
                    path {
                        d: "M5 20h14",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round"
                    }
                }
            }
            button {
                class: if terminal_visible { "activity-item active" } else { "activity-item" },
                aria_label: "Terminal",
                title: "Terminal",
                onclick: move |_| state.write().toggle_terminal(),
                svg {
                    class: "activity-icon",
                    width: "22",
                    height: "22",
                    view_box: "0 0 24 24",
                    fill: "none",
                    path {
                        d: "m5 8 4 4-4 4",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round",
                        stroke_linejoin: "round"
                    }
                    path {
                        d: "M11.5 17h7",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round"
                    }
                    path {
                        d: "M3.5 5.5h17v13h-17v-13Z",
                        stroke: "currentColor",
                        stroke_width: "1.4",
                        stroke_linecap: "round",
                        stroke_linejoin: "round"
                    }
                }
            }
            div { class: "activity-spacer" }
            button {
                class: "activity-item",
                aria_label: "Settings",
                title: "Settings",
                onclick: move |_| state.write().open_settings(),
                dangerous_inner_html: include_str!("../assets/gear.svg")
            }
        }
    }
}

#[component]
fn AboutDialog() -> Element {
    let mut state = use_context::<state::AppStore>();
    let visible = state.read().is_about_open();
    let version = env!("CARGO_PKG_VERSION");

    if !visible {
        return rsx! {};
    }

    rsx! {
        div {
            class: "about-backdrop",
            onclick: move |_| state.write().close_about(),
            div {
                class: "about-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "About Soyuz Studio",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "about-header",
                    div { class: "about-brand",
                        span { class: "about-mark", "S" }
                        div {
                            h2 { "Soyuz Studio" }
                            p { "v{version}" }
                        }
                    }
                    button {
                        class: "about-close",
                        aria_label: "Close About dialog",
                        onclick: move |_| state.write().close_about(),
                        "x"
                    }
                }
                p { class: "about-description",
                    "Procedural 3D workbench for Rhai SDF assets."
                }
                div { class: "about-meta",
                    div {
                        span { class: "about-label", "Runtime" }
                        span { "Desktop app built with Dioxus" }
                    }
                    div {
                        span { class: "about-label", "Theme" }
                        span { "Soyuz Graphite" }
                    }
                }
            }
        }
    }
}

/// Whether to start fresh (skip session restore)
static FRESH_START: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Global terminal buffer shared between tracing and runtime services
static TERMINAL_BUFFER: std::sync::OnceLock<TerminalBuffer> = std::sync::OnceLock::new();

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let launch_mode = match preview_cli::parse_launch_mode(&args) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let fresh_start = match launch_mode {
        LaunchMode::Studio { fresh_start } => fresh_start,
        LaunchMode::Preview {
            script_path,
            check_only,
        } => {
            return preview_cli::run_preview_command(&script_path, check_only);
        }
        LaunchMode::EmbeddedPreview {
            script_path,
            parent_handle,
            x,
            y,
            width,
            height,
            check_only,
        } => {
            return preview_cli::run_embedded_preview_command(
                &script_path,
                parent_handle,
                x,
                y,
                width,
                height,
                check_only,
            );
        }
        LaunchMode::ExportCheck {
            script_path,
            out_path,
            resolution,
        } => {
            return preview_cli::run_export_check_command(&script_path, &out_path, resolution);
        }
    };

    // Create a shared terminal buffer for capturing logs
    let terminal_buffer = TerminalBuffer::new();
    TERMINAL_BUFFER.set(terminal_buffer.clone()).ok();

    // Build layered tracing subscriber: terminal layer + filtered console output
    let terminal_layer = terminal::layer::TerminalLayer::new(terminal_buffer);

    // Console layer with filter: only show WARN+ from soyuz, INFO+ for others
    let fmt_layer = tracing_subscriber::fmt::layer();
    let filter = tracing_subscriber::EnvFilter::new(
        "warn,soyuz_studio=info,soyuz_engine=info,soyuz_script=info,soyuz_render=info",
    );

    tracing_subscriber::registry()
        .with(terminal_layer)
        .with(fmt_layer.with_filter(filter))
        .init();

    // Check for --fresh flag (used by New Window)
    FRESH_START.set(fresh_start).ok();

    if fresh_start {
        tracing::info!("Starting fresh session (--fresh flag)");
    }

    // Load window icon
    let icon = load_window_icon();

    // Remove native window decorations - we'll create our own title bar
    let window = WindowBuilder::new()
        .with_title("Soyuz Studio")
        .with_decorations(false)
        .with_window_icon(icon);

    let config = Config::new().with_window(window);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);

    ExitCode::SUCCESS
}

fn load_window_icon() -> Option<Icon> {
    let icon_bytes = include_bytes!("../../assets/icons/icon-256.png");
    let icon_image = image::load_from_memory(icon_bytes).ok()?.to_rgba8();
    let (width, height) = icon_image.dimensions();
    Icon::from_rgba(icon_image.into_raw(), width, height).ok()
}

#[component]
fn App() -> Element {
    // Non-reactive runtime services: process handles, preview IPC state, log buffer.
    use_context_provider(|| {
        let terminal_buffer = TERMINAL_BUFFER
            .get()
            .cloned()
            .unwrap_or_else(TerminalBuffer::new);
        AppServices::new(terminal_buffer)
    });

    // Global app state - load settings and session (unless --fresh flag)
    let app_state = dioxus_stores::use_store(|| {
        // Load settings from config file
        let loaded_settings = settings::load_settings();
        let mut state = AppState::with_settings(loaded_settings);

        // Only restore session if not starting fresh
        let fresh = FRESH_START.get().copied().unwrap_or(false);
        if !fresh && let Some(saved_session) = session::Session::load() {
            tracing::info!("Restoring session with {} tabs", saved_session.tabs.len());
            session::restore_session(&mut state, saved_session);
        }

        state
    });
    use_context_provider(|| app_state);

    // Command palette state
    use_context_provider(|| Signal::new(command_palette::PaletteState::default()));

    let mut state = use_context::<state::AppStore>();
    let mut palette = use_context::<Signal<command_palette::PaletteState>>();

    // Auto-save session every 30 seconds using use_future for background tasks
    use_future(move || async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            let session_data = session::state_to_session(&state.read());
            if let Err(e) = session_data.save() {
                tracing::warn!("Failed to save session: {}", e);
            }
        }
    });

    // Global keyboard shortcuts
    let on_keydown = move |e: Event<KeyboardData>| {
        let key = e.key();
        let ctrl = e.modifiers().ctrl();
        let shift = e.modifiers().shift();

        if key == Key::Escape && state.read().is_about_open() {
            e.prevent_default();
            state.write().close_about();
            return;
        }

        // Ctrl+P or Ctrl+Shift+P - Open unified search
        if ctrl
            && (key == Key::Character("p".to_string()) || key == Key::Character("P".to_string()))
        {
            e.prevent_default();
            palette.write().visible = true;
            palette.write().query.clear();
            palette.write().mode = command_palette::PaletteMode::Unified;
            // Trigger initial search to show all commands
            let workspace = state.read().workspace.clone();
            spawn(async move {
                let results = command_palette::unified_search(workspace.as_deref(), "").await;
                palette.write().unified_results = results;
            });
        }
        // Ctrl+G - Go to line
        else if ctrl && !shift && key == Key::Character("g".to_string()) {
            e.prevent_default();
            palette.write().visible = true;
            palette.write().query = ":".to_string();
            palette.write().mode = command_palette::PaletteMode::GoToLine;
        }
        // Ctrl+` - Toggle terminal
        else if ctrl && !shift && key == Key::Character("`".to_string()) {
            e.prevent_default();
            state.write().toggle_terminal();
        }
    };

    // Dynamic window title based on current file
    let title = {
        let s = state.read();
        if let Some(tab) = s.active_tab() {
            let name = tab
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map_or_else(|| tab.display_name(), |n| n.to_string_lossy().to_string());
            let dirty = if tab.is_dirty { " *" } else { "" };
            format!("{}{} - Soyuz Studio", name, dirty)
        } else {
            "Soyuz Studio".to_string()
        }
    };
    rsx! {
        document::Title { "{title}" }

        document::Stylesheet { href: assets::THEME_CSS }
        document::Stylesheet { href: assets::BASE_CSS }
        document::Stylesheet { href: assets::APP_CSS }
        document::Stylesheet { href: assets::TOOLBAR_CSS }
        document::Stylesheet { href: assets::BROWSER_CSS }
        document::Stylesheet { href: assets::PANE_CSS }
        document::Stylesheet { href: assets::TERMINAL_CSS }
        document::Stylesheet { href: assets::STATUSBAR_CSS }
        document::Stylesheet { href: assets::PALETTE_CSS }
        document::Stylesheet { href: assets::SETTINGS_CSS }
        document::Stylesheet { href: assets::MARKDOWN_CSS }
        document::Stylesheet { href: assets::EXPORT_CSS }

        // Native scroll sync - handles scroll without Rust async overhead
        // Uses data-editor-pane attribute since CSS module class names are hashed
        script {
            dangerous_inner_html: "
                document.addEventListener('scroll', function(e) {{
                    var paneId = e.target.dataset && e.target.dataset.editorPane;
                    if (!paneId) return;

                    var editor = e.target;
                    var lineNumbers = document.getElementById('line-numbers-' + paneId);
                    var syntax = document.getElementById('syntax-' + paneId);

                    if (lineNumbers) {{
                        lineNumbers.scrollTop = editor.scrollTop;
                    }}
                    if (syntax) {{
                        syntax.style.transform = 'translateY(-' + editor.scrollTop + 'px)';
                    }}
                }}, true);
            "
        }

        // Root-level error boundary - catches catastrophic errors
        ErrorBoundary {
            handle_error: |error| rsx! {
                div { class: "error-screen",
                    h2 { "Soyuz Studio encountered an error" }
                    pre { "{error:?}" }
                    button {
                        onclick: move |_| {
                            // Reload the application
                            match std::env::current_exe() {
                                Ok(exe) => {
                                    if let Err(e) = std::process::Command::new(exe).spawn() {
                                        tracing::error!("Failed to restart application: {e}");
                                    } else {
                                        std::process::exit(0);
                                    }
                                }
                                Err(e) => tracing::error!("Failed to get current executable: {e}"),
                            }
                        },
                        "Restart Application"
                    }
                }
            },

            div {
                class: "app-container",
                tabindex: "0",
                onkeydown: on_keydown,

                // Top toolbar
                toolbar::Toolbar {}

                // Content area with terminal
                div {
                    class: "content-with-terminal",
                    div { class: "main-content",
                        ActivityBar {}

                        // Left sidebar: Explorer (file browser) with error boundary
                        div { class: "panel explorer-panel",
                            ErrorBoundary {
                                handle_error: |error| rsx! {
                                    PanelError {
                                        panel_name: "File Explorer".to_string(),
                                        error_msg: format!("{error:?}")
                                    }
                                },
                                browser::AssetBrowser {}
                            }
                        }

                        // Center: Code editor with tabs and splits
                        div { class: "panel editor-panel",
                            ErrorBoundary {
                                handle_error: |error| rsx! {
                                    PanelError {
                                        panel_name: "Editor".to_string(),
                                        error_msg: format!("{error:?}")
                                    }
                                },
                                pane::PaneTree {}
                            }
                        }
                    }

                    // Terminal panel (bottom-docked, collapsible)
                    terminal::TerminalPanel {}
                }

                // Status bar
                statusbar::StatusBar {}

                // Command palette overlay
                command_palette::CommandPalette {}

                // About dialog overlay
                AboutDialog {}
            }
        }
    }
}
