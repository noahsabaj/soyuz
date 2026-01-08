//! Soyuz Studio - Desktop application for procedural asset generation

mod browser;
mod command_palette;
mod export;
mod markdown_panel;
mod js_interop;
mod pane;
mod preview;
mod session;
mod settings;
mod settings_panel;
mod state;
mod statusbar;
mod terminal;
mod terminal_layer;
mod toolbar;

use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use state::{AppState, TerminalBuffer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

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

/// Global terminal buffer shared between tracing and AppState
static TERMINAL_BUFFER: std::sync::OnceLock<TerminalBuffer> = std::sync::OnceLock::new();

fn main() {
    // Create a shared terminal buffer for capturing logs
    let terminal_buffer = TerminalBuffer::new();
    TERMINAL_BUFFER.set(terminal_buffer.clone()).ok();

    // Build layered tracing subscriber: terminal layer + filtered console output
    let terminal_layer = terminal_layer::TerminalLayer::new(terminal_buffer);

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
    // Global app state - load settings and session (unless --fresh flag)
    use_context_provider(|| {
        // Load settings from config file
        let loaded_settings = settings::load_settings();
        let mut state = AppState::with_settings(loaded_settings);

        // Use the shared terminal buffer from the tracing subscriber
        if let Some(buffer) = TERMINAL_BUFFER.get() {
            state.terminal_buffer = buffer.clone();
        }

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

    let mut state = use_context::<Signal<AppState>>();
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

        // Ctrl+P or Ctrl+Shift+P - Open unified search
        if ctrl && (key == Key::Character("p".to_string()) || key == Key::Character("P".to_string())) {
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
                div { class: "content-with-terminal",
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

                    // Terminal panel (bottom-docked, collapsible)
                    terminal::TerminalPanel {}
                }

                // Status bar
                statusbar::StatusBar {}

                // Command palette overlay
                command_palette::CommandPalette {}
            }
        }
    }
}
