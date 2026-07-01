//! Soyuz Studio - Desktop application for procedural asset generation

mod app_commands;
mod assets;
mod browser;
mod command_palette;
mod components;
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

use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use preview_cli::LaunchMode;
use services::AppServices;
use settings::SettingsStoreExt;
use state::{AppState, AppStateStoreExt, TerminalBuffer};
use std::process::ExitCode;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Panel-level error fallback component.
///
/// `pub(crate)` so panels dispatched outside this file (e.g. the docked
/// Preview/Export/Settings/Markdown panels rendered by `pane`) can share the
/// same fallback when they are wrapped in their own `ErrorBoundary`.
#[component]
pub(crate) fn PanelError(panel_name: String, error_msg: String) -> Element {
    rsx! {
        div { class: "error-panel",
            div { class: "error-panel-icon", "!" }
            h3 { "{panel_name} Error" }
            p { "An error occurred in this panel." }
            pre { "{error_msg}" }
        }
    }
}

/// SVG path data for the Settings gear icon, sourced from `assets/gear.svg` and
/// inlined so the icon renders as an `rsx!` `svg` element rather than via
/// `dangerous_inner_html`.
const GEAR_ICON_PATH: &str = "M29 11.756h-1.526c-0.109-0.295-0.229-0.584-0.361-0.87l1.087-1.076c0.441-0.389 0.717-0.956 0.717-1.587 0-0.545-0.206-1.042-0.545-1.417l0.002 0.002-3.178-3.178c-0.373-0.338-0.87-0.544-1.415-0.544-0.632 0-1.199 0.278-1.587 0.718l-0.002 0.002-1.081 1.080c-0.285-0.131-0.573-0.251-0.868-0.36l0.008-1.526c0.003-0.042 0.005-0.091 0.005-0.141 0-1.128-0.884-2.049-1.997-2.109l-0.005-0h-4.496c-1.119 0.059-2.004 0.981-2.004 2.11 0 0.049 0.002 0.098 0.005 0.147l-0-0.007v1.524c-0.295 0.109-0.584 0.229-0.87 0.361l-1.074-1.084c-0.389-0.443-0.957-0.722-1.589-0.722-0.545 0-1.042 0.206-1.416 0.545l0.002-0.002-3.179 3.179c-0.338 0.373-0.544 0.87-0.544 1.415 0 0.633 0.278 1.2 0.719 1.587l0.002 0.002 1.078 1.079c-0.132 0.287-0.252 0.576-0.362 0.872l-1.525-0.007c-0.042-0.003-0.091-0.005-0.14-0.005-1.128 0-2.050 0.885-2.11 1.998l-0 0.005v4.495c0.059 1.119 0.982 2.005 2.111 2.005 0.049 0 0.098-0.002 0.146-0.005l-0.007 0h1.525c0.109 0.295 0.229 0.584 0.361 0.87l-1.084 1.071c-0.443 0.39-0.721 0.958-0.721 1.592 0 0.545 0.206 1.043 0.545 1.418l-0.002-0.002 3.179 3.178c0.339 0.337 0.806 0.545 1.322 0.545 0.007 0 0.014-0 0.021-0h-0.001c0.653-0.013 1.24-0.287 1.662-0.722l0.001-0.001 1.079-1.079c0.287 0.132 0.577 0.252 0.873 0.362l-0.007 1.524c-0.003 0.042-0.005 0.091-0.005 0.14 0 1.128 0.885 2.050 1.998 2.11l0.005 0h4.496c1.118-0.060 2.003-0.981 2.003-2.109 0-0.050-0.002-0.099-0.005-0.147l0 0.007v-1.526c0.296-0.11 0.585-0.23 0.872-0.362l1.069 1.079c0.423 0.435 1.009 0.709 1.66 0.723l0.002 0h0.002c0.006 0 0.014 0 0.021 0 0.515 0 0.982-0.207 1.323-0.541l3.177-3.177c0.335-0.339 0.541-0.805 0.541-1.32 0-0.009-0-0.018-0-0.028l0 0.001c-0.013-0.651-0.285-1.236-0.718-1.658l-0.001-0-1.080-1.081c0.131-0.285 0.251-0.573 0.36-0.868l1.525 0.007c0.042 0.003 0.090 0.005 0.139 0.005 1.129 0 2.051-0.885 2.11-1.999l0-0.005v-4.495c-0.060-1.119-0.981-2.004-2.11-2.004-0.049 0-0.098 0.002-0.147 0.005l0.007-0zM28.75 17.749l-2.162-0.011c-0.026 0-0.048 0.013-0.074 0.015-0.093 0.009-0.179 0.026-0.261 0.053l0.008-0.002c-0.31 0.068-0.565 0.263-0.711 0.527l-0.003 0.005c-0.048 0.071-0.091 0.152-0.124 0.238l-0.003 0.008c-0.008 0.024-0.027 0.041-0.034 0.066-0.23 0.804-0.527 1.503-0.898 2.155l0.025-0.048c-0.014 0.025-0.013 0.054-0.026 0.080-0.029 0.063-0.053 0.138-0.071 0.215l-0.001 0.008c-0.023 0.072-0.040 0.156-0.048 0.242l-0 0.005c-0.002 0.027-0.004 0.058-0.004 0.089 0 0.209 0.061 0.404 0.166 0.568l-0.003-0.004c0.045 0.088 0.096 0.163 0.154 0.232l-0.001-0.002c0.017 0.019 0.022 0.043 0.040 0.061l1.529 1.531-2.469 2.467-1.516-1.529c-0.020-0.021-0.048-0.027-0.069-0.046-0.060-0.050-0.128-0.096-0.200-0.135l-0.006-0.003c-0.195-0.109-0.429-0.173-0.677-0.173-0.002 0-0.004 0-0.006 0h0c-0.076 0.008-0.145 0.022-0.211 0.040l0.009-0.002c-0.102 0.020-0.192 0.050-0.276 0.089l0.007-0.003c-0.022 0.011-0.047 0.010-0.069 0.022-0.606 0.346-1.307 0.644-2.043 0.859l-0.070 0.017c-0.027 0.008-0.045 0.027-0.071 0.037-0.084 0.033-0.157 0.071-0.224 0.116l0.004-0.003c-0.075 0.041-0.139 0.085-0.199 0.135l0.002-0.002c-0.053 0.052-0.102 0.110-0.145 0.171l-0.003 0.004c-0.103 0.113-0.176 0.254-0.206 0.411l-0.001 0.005c-0.024 0.074-0.043 0.160-0.051 0.249l-0 0.005c-0.002 0.026-0.015 0.048-0.015 0.075l-0.001 2.162h-3.491l0.011-2.156c0-0.028-0.014-0.052-0.016-0.079-0.008-0.092-0.026-0.177-0.052-0.258l0.002 0.008c-0.068-0.313-0.265-0.570-0.531-0.717l-0.006-0.003c-0.070-0.047-0.150-0.089-0.235-0.122l-0.008-0.003c-0.024-0.008-0.042-0.027-0.067-0.034-0.806-0.230-1.507-0.528-2.161-0.9l0.048 0.025c-0.023-0.013-0.050-0.012-0.073-0.023-0.072-0.033-0.156-0.061-0.244-0.079l-0.008-0.001c-0.092-0.029-0.198-0.045-0.308-0.045-0.221 0-0.426 0.066-0.597 0.18l0.004-0.002c-0.076 0.040-0.141 0.084-0.201 0.134l0.002-0.002c-0.021 0.019-0.048 0.025-0.068 0.045l-1.529 1.529-2.47-2.469 1.532-1.516c0.020-0.020 0.027-0.047 0.045-0.067 0.053-0.063 0.101-0.134 0.142-0.209l0.003-0.006c0.037-0.058 0.071-0.124 0.099-0.194l0.003-0.008c0.038-0.140 0.062-0.301 0.066-0.467l0-0.003c-0.008-0.083-0.023-0.158-0.044-0.231l0.002 0.009c-0.020-0.094-0.047-0.177-0.083-0.255l0.003 0.007c-0.012-0.025-0.011-0.052-0.025-0.076-0.347-0.605-0.645-1.305-0.858-2.041l-0.017-0.068c-0.007-0.026-0.027-0.045-0.036-0.070-0.034-0.086-0.072-0.160-0.118-0.228l0.003 0.005c-0.040-0.074-0.084-0.138-0.133-0.197l0.002 0.002c-0.052-0.053-0.109-0.101-0.169-0.144l-0.004-0.003c-0.060-0.051-0.128-0.097-0.200-0.136l-0.006-0.003c-0.057-0.026-0.126-0.049-0.196-0.066l-0.008-0.002c-0.077-0.026-0.167-0.045-0.259-0.053l-0.005-0c-0.026-0.002-0.047-0.015-0.073-0.015l-2.162-0.001v-3.492l2.162 0.011c0.160-0.002 0.311-0.035 0.450-0.092l-0.008 0.003c0.054-0.024 0.099-0.048 0.142-0.075l-0.005 0.003c0.090-0.047 0.168-0.100 0.239-0.160l-0.002 0.001c0.043-0.039 0.082-0.079 0.118-0.122l0.002-0.002c0.056-0.065 0.106-0.138 0.147-0.215l0.003-0.007c0.027-0.047 0.054-0.102 0.076-0.159l0.003-0.008c0.010-0.028 0.029-0.050 0.037-0.078 0.230-0.805 0.527-1.506 0.899-2.159l-0.025 0.048c0.014-0.024 0.013-0.052 0.025-0.076 0.031-0.067 0.057-0.147 0.075-0.229l0.001-0.008c0.020-0.086 0.032-0.185 0.032-0.287 0-0.317-0.113-0.607-0.300-0.834l0.002 0.002c-0.017-0.020-0.023-0.045-0.042-0.063l-1.527-1.529 2.469-2.469 1.518 1.531c0.055 0.045 0.116 0.087 0.180 0.122l0.006 0.003c0.042 0.033 0.089 0.065 0.138 0.094l0.006 0.003c0.160 0.088 0.350 0.142 0.551 0.148l0.002 0 0.005 0.001c0.012 0 0.023-0.009 0.034-0.009 0.186-0.008 0.359-0.056 0.513-0.135l-0.007 0.003c0.022-0.011 0.047-0.006 0.070-0.018 0.605-0.346 1.305-0.645 2.041-0.858l0.069-0.017c0.025-0.007 0.042-0.026 0.066-0.034 0.091-0.035 0.170-0.076 0.243-0.125l-0.004 0.003c0.069-0.038 0.128-0.079 0.183-0.124l-0.002 0.002c0.058-0.056 0.110-0.117 0.156-0.183l0.003-0.004c0.046-0.056 0.089-0.119 0.126-0.185l0.003-0.006c0.028-0.062 0.053-0.135 0.070-0.210l0.002-0.008c0.024-0.073 0.042-0.158 0.050-0.247l0-0.005c0.002-0.027 0.015-0.049 0.015-0.076l0.001-2.162h3.491l-0.011 2.156c-0 0.028 0.014 0.051 0.015 0.079 0.008 0.093 0.026 0.178 0.052 0.260l-0.002-0.008c0.019 0.084 0.044 0.157 0.075 0.227l-0.003-0.008c0.040 0.073 0.082 0.136 0.130 0.194l-0.002-0.002c0.048 0.070 0.101 0.131 0.158 0.187l0 0c0.053 0.044 0.112 0.084 0.174 0.120l0.006 0.003c0.068 0.046 0.147 0.087 0.230 0.119l0.008 0.003c0.025 0.009 0.043 0.028 0.069 0.035 0.804 0.229 1.503 0.527 2.155 0.899l-0.047-0.025c0.022 0.012 0.046 0.007 0.068 0.018 0.147 0.076 0.320 0.124 0.503 0.132l0.003 0c0.012 0 0.024 0.009 0.036 0.009l0.028-0.008c0.193-0.008 0.372-0.059 0.531-0.143l-0.007 0.003c0.059-0.033 0.109-0.066 0.156-0.104l-0.003 0.002c0.068-0.037 0.127-0.076 0.181-0.120l-0.002 0.002 1.531-1.528 2.469 2.47-1.531 1.516c-0.020 0.020-0.027 0.047-0.046 0.068-0.053 0.063-0.101 0.134-0.142 0.209l-0.003 0.006c-0.084 0.123-0.138 0.272-0.148 0.434l-0 0.002c-0.013 0.056-0.020 0.121-0.020 0.187 0 0.097 0.016 0.190 0.045 0.277l-0.002-0.006c0.020 0.094 0.047 0.176 0.083 0.254l-0.003-0.007c0.012 0.025 0.011 0.053 0.025 0.078 0.347 0.604 0.645 1.303 0.858 2.038l0.017 0.068c0.008 0.030 0.028 0.052 0.038 0.080 0.024 0.062 0.049 0.113 0.077 0.163l-0.003-0.006c0.211 0.397 0.619 0.665 1.090 0.674l0.001 0 2.162 0.001zM16 10.75c-2.899 0-5.25 2.351-5.25 5.25s2.351 5.25 5.25 5.25c2.899 0 5.25-2.351 5.25-5.25v0c-0.004-2.898-2.352-5.246-5.25-5.25h-0zM16 18.75c-1.519 0-2.75-1.231-2.75-2.75s1.231-2.75 2.75-2.75c1.519 0 2.75 1.231 2.75 2.75v0c-0.002 1.518-1.232 2.748-2.75 2.75h-0z";

#[component]
fn ActivityBar() -> Element {
    let mut state = use_context::<state::AppStore>();
    let mut palette = use_context::<Signal<command_palette::PaletteState>>();
    let services = use_context::<AppServices>();

    // F73: subscribe only to the focused pane's active-tab kind and the terminal
    // visibility via field selectors, instead of reading the whole store.
    let focused = *state.focused_pane_id().read();
    let editor_pane = state.editor_pane();
    let pane_ref = editor_pane.read();
    let active_tab = pane_ref.find_pane(focused).and_then(|p| p.active_tab());
    let preview_active = active_tab.is_some_and(|tab| tab.is_preview());
    let export_active = active_tab.is_some_and(|tab| tab.is_export());
    drop(pane_ref);
    let terminal_visible = *state.terminal_visible().read();

    rsx! {
        div { class: "activity-bar",
            components::ActivityButton {
                active: true,
                label: "Explorer",
                title: "Explorer",
                on_click: move |()| {},
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
            components::ActivityButton {
                active: false,
                label: "Command Palette",
                title: "Command Palette",
                on_click: move |()| {
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
            components::ActivityButton {
                active: preview_active,
                label: "Preview",
                title: "Preview",
                on_click: {
                    let services = services.clone();
                    move |()| crate::preview::open_docked_preview(state, services.clone())
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
            components::ActivityButton {
                active: export_active,
                label: "Export",
                title: "Export",
                on_click: move |()| crate::export::open_export_panel(state),
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
            components::ActivityButton {
                active: terminal_visible,
                label: "Terminal",
                title: "Terminal",
                on_click: move |()| { state.terminal_visible().toggle(); },
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
            components::ActivityButton {
                active: false,
                label: "Settings",
                title: "Settings",
                on_click: move |()| { state.write().open_settings(); },
                svg {
                    width: "16",
                    height: "16",
                    view_box: "0 0 32 32",
                    path { d: GEAR_ICON_PATH }
                }
            }
        }
    }
}

#[component]
fn AboutDialog() -> Element {
    use dioxus_primitives::dialog::{DialogContent, DialogDescription, DialogRoot, DialogTitle};

    let state = use_context::<state::AppStore>();
    // F73: subscribe only to the dialog field, not the whole store.
    let visible = matches!(*state.active_dialog().read(), Some(state::AppDialog::About));
    let version = env!("CARGO_PKG_VERSION");

    rsx! {
        DialogRoot {
            class: "about-backdrop",
            open: visible,
            on_open_change: move |o: bool| {
                if !o {
                    state.active_dialog().set(None);
                }
            },
            DialogContent {
                // The primitive provides role="dialog", aria-modal, focus trap,
                // and Escape-to-close; DialogTitle/Description supply the labels.
                class: "about-dialog".to_string(),
                div { class: "about-header",
                    div { class: "about-brand",
                        span { class: "about-mark", "S" }
                        div {
                            DialogTitle { "Soyuz Studio" }
                            p { "v{version}" }
                        }
                    }
                    button {
                        class: "about-close",
                        r#type: "button",
                        aria_label: "Close About dialog",
                        onclick: move |_| { state.active_dialog().set(None); },
                        "×"
                    }
                }
                DialogDescription { class: "about-description",
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
    let mut window = WindowBuilder::new()
        .with_title("Soyuz Studio")
        .with_decorations(false)
        .with_window_icon(icon)
        // Default size + a hard floor so a first run (no saved geometry) or a bad
        // record can never produce a cramped window where the custom title-bar
        // controls collide with the menu bar. The restore below overrides the size
        // when a saved record exists.
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .with_min_inner_size(LogicalSize::new(800.0, 600.0));

    // Restore the previous window geometry when a valid record exists. Robust by
    // design: missing/invalid data just leaves the defaults above in place.
    if let Some(geometry) = load_window_geometry() {
        if geometry.width > 0 && geometry.height > 0 {
            window = window
                .with_inner_size(PhysicalSize::new(geometry.width, geometry.height))
                .with_position(PhysicalPosition::new(geometry.x, geometry.y));
        }
        window = window.with_maximized(geometry.maximized);
    }

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

/// Persisted window geometry (physical pixels), stored beside the session file.
///
/// Kept in its own tiny `window.json` rather than the session struct so it stays
/// self-contained and robust: any read/parse failure simply falls back to the
/// platform's default window placement.
#[derive(serde::Serialize, serde::Deserialize)]
struct WindowGeometry {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    maximized: bool,
}

/// Path to the window-geometry file (`<config_dir>/soyuz/window.json`).
fn window_geometry_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|dir| dir.join("soyuz").join("window.json"))
}

/// Load persisted window geometry, if present and parseable.
fn load_window_geometry() -> Option<WindowGeometry> {
    let path = window_geometry_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Persist the current window geometry (best-effort; all errors are ignored so a
/// failure to save never blocks window teardown).
pub(crate) fn save_window_geometry(width: u32, height: u32, x: i32, y: i32, maximized: bool) {
    let Some(path) = window_geometry_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let geometry = WindowGeometry {
        width,
        height,
        x,
        y,
        maximized,
    };
    if let Ok(content) = serde_json::to_string_pretty(&geometry) {
        let _ = std::fs::write(&path, content);
    }
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

        // Only restore session if not starting fresh and the setting is enabled
        let fresh = FRESH_START.get().copied().unwrap_or(false);
        if !fresh
            && state.settings.restore_session
            && let Some(saved_session) = session::Session::load()
        {
            tracing::info!("Restoring session with {} tabs", saved_session.tabs.len());
            session::restore_session(&mut state, saved_session);
        }

        state
    });
    use_context_provider(|| app_state);

    // Command palette state
    use_context_provider(|| Signal::new(command_palette::PaletteState::default()));

    // Pending tab-close awaiting unsaved-changes confirmation (F71).
    let pending_close = use_context_provider(|| Signal::new(None::<pane::PendingCloseTab>));

    let mut state = use_context::<state::AppStore>();
    let palette = use_context::<Signal<command_palette::PaletteState>>();
    let services = use_context::<AppServices>();

    // Periodically checkpoint the session for crash recovery. The cadence follows
    // the `auto_save` setting when set, falling back to 30s (auto-save Off still
    // keeps crash recovery — a clean shutdown separately runs the synchronous
    // `save()`, which also persists pane/tab layout). We skip the write when no tab
    // has unsaved edits so an idle editor doesn't rewrite the file each cycle, and
    // use `save_async` so the atomic write+rename happens off the UI executor.
    use_future(move || async move {
        const DEFAULT_AUTOSAVE_SECS: u64 = 30;
        loop {
            let secs = state
                .settings()
                .auto_save()
                .cloned()
                .interval_secs()
                .map_or(DEFAULT_AUTOSAVE_SECS, u64::from);
            tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;

            if !state.peek().has_unsaved_changes() {
                continue;
            }
            let session_data = session::state_to_session(&state.peek());
            if let Err(e) = session_data.save_async().await {
                tracing::warn!("Failed to save session: {}", e);
            }
        }
    });

    // Global keyboard shortcuts. These work regardless of focus; the editor textarea
    // calls `stop_propagation()` on the shortcuts it handles so they are not run twice.
    let on_keydown = move |e: Event<KeyboardData>| {
        let key = e.key();
        // Accept Cmd (meta) in addition to Ctrl so shortcuts also work on macOS.
        let ctrl = e.modifiers().ctrl() || e.modifiers().meta();
        let shift = e.modifiers().shift();
        // Lowercase character keys so Caps Lock / Shift do not break matching.
        let key_char = match &key {
            Key::Character(s) => Some(s.to_lowercase()),
            _ => None,
        };
        let is_char = |c: &str| key_char.as_deref() == Some(c);

        // Escape closes the About dialog.
        if key == Key::Escape && state.read().is_about_open() {
            e.prevent_default();
            state.write().close_about();
            return;
        }

        // Function-key shortcuts (no modifier required).
        if key == Key::F5 {
            e.prevent_default();
            let id = if shift { "preview.stop" } else { "preview.run" };
            app_commands::execute_app_command(id, state, services.clone(), palette);
            return;
        }
        if key == Key::F1 {
            e.prevent_default();
            app_commands::execute_app_command(
                "help.documentation",
                state,
                services.clone(),
                palette,
            );
            return;
        }

        // Ctrl+Enter - run preview.
        if ctrl && key == Key::Enter {
            e.prevent_default();
            app_commands::execute_app_command("preview.run", state, services.clone(), palette);
            return;
        }

        if !ctrl {
            return;
        }

        // Ctrl (+Shift) + character shortcuts.
        if is_char("p") {
            // Ctrl+P or Ctrl+Shift+P - open unified search.
            e.prevent_default();
            app_commands::open_unified_palette(palette, state, "");
        } else if is_char("g") && !shift {
            // Ctrl+G - go to line.
            e.prevent_default();
            app_commands::open_go_to_line_palette(palette);
        } else if is_char("`") && !shift {
            // Ctrl+` - toggle terminal. Route through the field selector so only
            // terminal_visible subscribers re-render, not the whole store.
            e.prevent_default();
            state.terminal_visible().toggle();
        } else if is_char("\\") {
            // Ctrl+\ - split vertical; Ctrl+Shift+\ - split horizontal.
            e.prevent_default();
            let pane_id = state.read().focused_pane_id;
            let direction = if shift {
                state::SplitDirection::Horizontal
            } else {
                state::SplitDirection::Vertical
            };
            state.write().split_pane(pane_id, direction);
        } else if is_char("s") {
            // Ctrl+S - save; Ctrl+Shift+S - save as.
            e.prevent_default();
            let id = if shift { "file.saveAs" } else { "file.save" };
            app_commands::execute_app_command(id, state, services.clone(), palette);
        } else if is_char("o") && !shift {
            // Ctrl+O - open file.
            e.prevent_default();
            app_commands::execute_app_command("file.open", state, services.clone(), palette);
        } else if is_char("n") && !shift {
            // Ctrl+N - new tab.
            e.prevent_default();
            app_commands::execute_app_command("file.new", state, services.clone(), palette);
        } else if is_char("z") {
            // Ctrl+Z - undo; Ctrl+Shift+Z - redo.
            e.prevent_default();
            let id = if shift { "edit.redo" } else { "edit.undo" };
            app_commands::execute_app_command(id, state, services.clone(), palette);
        } else if is_char("w") && !shift {
            // Ctrl+W - close active tab. request_close_tab stops the preview if it is
            // the Preview tab and confirms before discarding unsaved edits (F71).
            e.prevent_default();
            let (pane_id, tab_id) = {
                let s = state.read();
                (s.focused_pane_id, s.active_tab().map(|t| t.id))
            };
            if let Some(tab_id) = tab_id {
                pane::request_close_tab(state, &services, pending_close, pane_id, tab_id);
            }
        }
    };

    // Dynamic window title based on the current file. Memoized against just the
    // pane tree + focused pane (F73) so the App re-renders only when the title
    // text actually changes, rather than subscribing to the whole store.
    let title = use_memo(move || {
        let focused = *state.focused_pane_id().read();
        let editor_pane = state.editor_pane();
        let pane_ref = editor_pane.read();
        let Some(tab) = pane_ref.find_pane(focused).and_then(|p| p.active_tab()) else {
            return "Soyuz Studio".to_string();
        };
        let name = tab
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| tab.display_name(), |n| n.to_string_lossy().to_string());
        let dirty = if tab.is_dirty { " *" } else { "" };
        format!("{}{} - Soyuz Studio", name, dirty)
    });
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
        document::Stylesheet { href: assets::COMPONENTS_CSS }

        // Native scroll sync - externalized to assets/scroll-sync.js and loaded
        // as a deduplicated `<script src>` instead of a large inline
        // dangerous_inner_html block.
        document::Script { src: asset!("/assets/scroll-sync.js") }

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

                    // Terminal panel (bottom-docked, collapsible), isolated so a
                    // panic in the terminal cannot tear down the editor.
                    ErrorBoundary {
                        handle_error: |error| rsx! {
                            PanelError {
                                panel_name: "Terminal".to_string(),
                                error_msg: format!("{error:?}")
                            }
                        },
                        terminal::TerminalPanel {}
                    }
                }

                // Status bar
                statusbar::StatusBar {}

                // Command palette overlay, isolated behind its own error boundary.
                ErrorBoundary {
                    handle_error: |error| rsx! {
                        PanelError {
                            panel_name: "Command Palette".to_string(),
                            error_msg: format!("{error:?}")
                        }
                    },
                    command_palette::CommandPalette {}
                }

                // About dialog overlay
                AboutDialog {}

                // Unsaved-changes close confirmation overlay (F71)
                pane::CloseTabConfirmDialog {}
            }
        }
    }
}
