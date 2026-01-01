//! Soyuz Studio - Desktop application for procedural asset generation

mod browser;
mod command_palette;
mod export;
mod js_interop;
mod pane;
mod preview;
mod session;
mod state;
mod statusbar;
mod toolbar;

use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use state::AppState;

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

/// Whether to start fresh (skip session restore)
static FRESH_START: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

fn main() {
    tracing_subscriber::fmt::init();

    // Check for --fresh flag (used by New Window)
    let args: Vec<String> = std::env::args().collect();
    let fresh = args.contains(&"--fresh".to_string());
    FRESH_START.set(fresh).ok();

    if fresh {
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
}

fn load_window_icon() -> Option<Icon> {
    let icon_bytes = include_bytes!("../../assets/icons/icon-256.png");
    let icon_image = image::load_from_memory(icon_bytes).ok()?.to_rgba8();
    let (width, height) = icon_image.dimensions();
    Icon::from_rgba(icon_image.into_raw(), width, height).ok()
}

#[component]
fn App() -> Element {
    // Global app state - load from session if available (unless --fresh flag)
    use_context_provider(|| {
        let mut state = AppState::new();

        // Only restore session if not starting fresh
        let fresh = FRESH_START.get().copied().unwrap_or(false);
        if !fresh
            && let Some(saved_session) = session::Session::load()
        {
            tracing::info!("Restoring session with {} tabs", saved_session.tabs.len());
            session::restore_session(&mut state, saved_session);
        }

        Signal::new(state)
    });

    // Command palette state
    use_context_provider(|| Signal::new(command_palette::PaletteState::default()));

    let state = use_context::<Signal<AppState>>();
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

        // Ctrl+P - Open file search
        if ctrl && !shift && key == Key::Character("p".to_string()) {
            e.prevent_default();
            palette.write().visible = true;
            palette.write().query.clear();
            palette.write().mode = command_palette::PaletteMode::Files;
        }
        // Ctrl+Shift+P - Open command palette
        else if ctrl && shift && key == Key::Character("P".to_string()) {
            e.prevent_default();
            palette.write().visible = true;
            palette.write().query = ">".to_string();
            palette.write().mode = command_palette::PaletteMode::Commands;
            palette.write().filtered_commands = command_palette::get_all_commands();
        }
        // Ctrl+G - Go to line
        else if ctrl && !shift && key == Key::Character("g".to_string()) {
            e.prevent_default();
            palette.write().visible = true;
            palette.write().query = ":".to_string();
            palette.write().mode = command_palette::PaletteMode::GoToLine;
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
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| tab.display_name());
            let dirty = if tab.is_dirty { " *" } else { "" };
            format!("{}{} - Soyuz Studio", name, dirty)
        } else {
            "Soyuz Studio".to_string()
        }
    };

    rsx! {
        document::Title { "{title}" }
        style { {include_str!("../assets/theme.css")} }
        style { {include_str!("../assets/style.css")} }

        // Native scroll sync - handles scroll without Rust async overhead
        script {
            dangerous_inner_html: "
                document.addEventListener('scroll', function(e) {{
                    if (!e.target.classList || !e.target.classList.contains('code-input')) return;

                    var editor = e.target;
                    var paneId = editor.id.replace('editor-', '');
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
                            if let Ok(exe) = std::env::current_exe() {
                                let _ = std::process::Command::new(exe).spawn();
                                std::process::exit(0);
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

                div { class: "main-content",
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

                // Status bar
                statusbar::StatusBar {}

                // Command palette overlay
                command_palette::CommandPalette {}
            }
        }
    }
}
