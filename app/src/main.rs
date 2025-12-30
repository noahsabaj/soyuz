//! Soyuz Studio - Desktop application for procedural asset generation

mod browser;
mod export;
mod js_interop;
mod pane;
mod preview;
mod session;
mod state;
mod statusbar;
mod toolbar;
mod viewport;

use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use state::AppState;

fn main() {
    tracing_subscriber::fmt::init();

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
    // Global app state - load from session if available
    use_context_provider(|| {
        let mut state = AppState::new();

        // Try to restore session
        if let Some(saved_session) = session::Session::load() {
            tracing::info!("Restoring session with {} tabs", saved_session.tabs.len());
            session::restore_session(&mut state, saved_session);
        }

        Signal::new(state)
    });

    let state = use_context::<Signal<AppState>>();

    // Auto-save session every 30 seconds
    use_effect(move || {
        spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                let session_data = session::state_to_session(&state.read());
                if let Err(e) = session_data.save() {
                    tracing::warn!("Failed to save session: {}", e);
                }
            }
        });
    });

    rsx! {
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

        div { class: "app-container",
            // Top toolbar
            toolbar::Toolbar {}

            div { class: "main-content",
                // Left sidebar: Explorer (file browser)
                div { class: "panel explorer-panel",
                    browser::AssetBrowser {}
                }

                // Center: Code editor with tabs and splits
                div { class: "panel editor-panel",
                    pane::PaneTree {}
                }

                // Right sidebar: Reference and Export
                div { class: "right-sidebar",
                    div { class: "panel reference-panel",
                        viewport::ViewportPanel {}
                    }
                    div { class: "panel export-panel",
                        export::ExportPanel {}
                    }
                }
            }

            // Status bar
            statusbar::StatusBar {}
        }
    }
}
