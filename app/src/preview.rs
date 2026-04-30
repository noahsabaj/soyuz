//! Preview management - docked preview tab plus pop-out fallback.

use crate::js_interop::{self, DomRect};
use crate::services::AppServices;
use crate::state::{AppStore, TerminalLevel};
use dioxus::desktop::wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use dioxus::prelude::*;
use soyuz_engine::Engine;
use std::path::Path;
use std::process::{Child, Command};

const PREVIEW_HOST_ID: &str = "soyuz-preview-host";
const PREVIEW_HOST_RETRY_ATTEMPTS: usize = 20;
const PREVIEW_HOST_RETRY_DELAY_MS: u64 = 50;

/// Open the docked preview tab and render the current script there.
pub fn open_docked_preview(mut state: AppStore, services: AppServices) {
    let code = state.read().source_code();
    state.write().open_preview_tab();
    spawn_preview_with_code(state, services, &code, PreviewPlacement::Docked);
}

/// Refresh the docked preview using the script captured by the preview tab.
pub fn refresh_docked_preview(mut state: AppStore, services: AppServices) {
    let code = state.read().source_code();
    state.write().open_preview_tab();
    spawn_preview_with_code(state, services, &code, PreviewPlacement::Docked);
}

/// Spawn the preview in a separate OS window.
pub fn pop_out_preview(state: AppStore, services: AppServices) {
    let code = state.read().source_code();
    spawn_preview_with_code(state, services, &code, PreviewPlacement::PopOut);
}

/// Stop the current preview process.
pub fn stop_preview(mut state: AppStore, services: &AppServices) {
    state.write().stop_preview(services);
}

#[derive(Clone, Copy)]
enum PreviewPlacement {
    Docked,
    PopOut,
}

fn spawn_preview_with_code(
    mut state: AppStore,
    services: AppServices,
    code: &str,
    placement: PreviewPlacement,
) {
    services.stop_preview_process();
    services.terminal_log(TerminalLevel::Info, "Starting preview...");

    let temp_path = std::env::temp_dir().join("soyuz_preview.rhai");
    if let Err(e) = std::fs::write(&temp_path, code) {
        let error_msg = format!("Failed to write temp script: {e}");
        services.terminal_log(TerminalLevel::Error, &error_msg);
        tracing::error!("{error_msg}");
        return;
    }

    {
        let mut s = state.write();
        s.is_previewing = true;
        s.preview_dirty = false;
        s.error_message = None;
    }
    services.publish_preview_script(code.to_string());

    let engine = Engine::new();
    if let Err(e) = engine.compile(code) {
        let error_msg = format!("Script validation error: {e}");
        services.terminal_log(TerminalLevel::Error, &error_msg);
        state.write().error_message = Some(e.to_string());
    }

    let parent_handle = if matches!(placement, PreviewPlacement::Docked) {
        preview_parent_handle()
    } else {
        None
    };

    spawn(async move {
        let rect = if matches!(placement, PreviewPlacement::Docked) {
            wait_for_preview_host_rect().await
        } else {
            None
        };

        match placement {
            PreviewPlacement::Docked => {
                let Some(parent) = parent_handle else {
                    report_docked_preview_unavailable(
                        state,
                        &services,
                        "Docked preview requires an X11 parent window handle. This desktop session did not expose one, so Soyuz did not open a pop-out window automatically. Use Pop Out for a separate preview window.",
                    );
                    return;
                };

                let Some(rect) = rect else {
                    report_docked_preview_unavailable(
                        state,
                        &services,
                        "Docked preview host is not ready yet. Click Refresh to try the tab preview again, or use Pop Out for a separate preview window.",
                    );
                    return;
                };

                if rect.width < 1 || rect.height < 1 {
                    report_docked_preview_unavailable(
                        state,
                        &services,
                        "Docked preview host has no visible size yet. Click Refresh after the Preview tab is visible, or use Pop Out for a separate preview window.",
                    );
                    return;
                }

                let child = match spawn_embedded_preview_process(&temp_path, parent, rect) {
                    Ok(child) => child,
                    Err(e) => {
                        let error_msg = format!("Failed to spawn docked preview: {e}");
                        services.terminal_log(TerminalLevel::Error, &error_msg);
                        tracing::error!("{error_msg}");
                        let mut s = state.write();
                        s.error_message = Some(error_msg);
                        s.is_previewing = false;
                        return;
                    }
                };

                services.terminal_log(TerminalLevel::Info, "Preview docked");
                services.set_preview_process(child);

                let process_handle_wait = services.preview_process();
                wait_for_process_exit(state, services, process_handle_wait).await;
            }
            PreviewPlacement::PopOut => match spawn_preview_process(&temp_path) {
                Ok(child) => {
                    services.terminal_log(TerminalLevel::Info, "Preview window opened");
                    services.set_preview_process(child);

                    let process_handle_wait = services.preview_process();
                    wait_for_process_exit(state, services, process_handle_wait).await;
                }
                Err(e) => {
                    let error_msg = format!("Failed to spawn preview: {e}");
                    services.terminal_log(TerminalLevel::Error, &error_msg);
                    tracing::error!("{error_msg}");
                    let mut s = state.write();
                    s.error_message = Some(format!("Failed to spawn preview: {e}"));
                    s.is_previewing = false;
                }
            },
        }
    });
}

async fn wait_for_preview_host_rect() -> Option<DomRect> {
    for _ in 0..PREVIEW_HOST_RETRY_ATTEMPTS {
        if let Some(rect) = js_interop::get_element_rect(PREVIEW_HOST_ID).await
            && rect.width > 0
            && rect.height > 0
        {
            return Some(rect);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(
            PREVIEW_HOST_RETRY_DELAY_MS,
        ))
        .await;
    }

    None
}

fn report_docked_preview_unavailable(
    mut state: AppStore,
    services: &AppServices,
    message: &'static str,
) {
    services.terminal_log(TerminalLevel::Warn, message);
    let mut s = state.write();
    s.error_message = Some(message.to_string());
    s.is_previewing = false;
}

async fn wait_for_process_exit(
    mut state: AppStore,
    services: AppServices,
    process_handle: std::sync::Arc<parking_lot::Mutex<Option<Child>>>,
) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut guard = process_handle.lock();
        if let Some(ref mut process) = *guard {
            match process.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    drop(guard);
                    if status.success() {
                        services.terminal_log(TerminalLevel::Info, "Preview closed");
                    } else {
                        services.terminal_log(
                            TerminalLevel::Warn,
                            format!("Preview exited with status: {status}"),
                        );
                    }
                    state.write().is_previewing = false;
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    *guard = None;
                    drop(guard);
                    services.terminal_log(
                        TerminalLevel::Error,
                        format!("Error checking preview status: {e}"),
                    );
                    state.write().is_previewing = false;
                    break;
                }
            }
        } else {
            drop(guard);
            services.terminal_log(TerminalLevel::Info, "Preview stopped");
            state.write().is_previewing = false;
            break;
        }
    }
}

fn spawn_preview_process(script_path: &Path) -> Result<Child, std::io::Error> {
    preview_process_command(&std::env::current_exe()?, script_path).spawn()
}

fn preview_process_command(exe_path: &Path, script_path: &Path) -> Command {
    let mut command = Command::new(exe_path);
    command.arg("--preview").arg("--script").arg(script_path);
    command
}

fn spawn_embedded_preview_process(
    script_path: &Path,
    parent_handle: u32,
    rect: DomRect,
) -> Result<Child, std::io::Error> {
    embedded_preview_process_command(&std::env::current_exe()?, script_path, parent_handle, rect)
        .spawn()
}

fn embedded_preview_process_command(
    exe_path: &Path,
    script_path: &Path,
    parent_handle: u32,
    rect: DomRect,
) -> Command {
    let mut command = Command::new(exe_path);
    command
        .arg("--embedded-preview")
        .arg("--script")
        .arg(script_path)
        .arg("--parent")
        .arg(parent_handle.to_string())
        .arg("--x")
        .arg(rect.x.to_string())
        .arg("--y")
        .arg(rect.y.to_string())
        .arg("--width")
        .arg(rect.width.to_string())
        .arg("--height")
        .arg(rect.height.to_string());
    command
}

fn preview_parent_handle() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let window = dioxus::desktop::window();
        match window.window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
            _ => None,
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Docked preview tab body.
#[component]
pub fn PreviewPanel() -> Element {
    let state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let is_previewing = state.read().is_previewing;
    let is_dirty = state.read().preview_dirty;
    let error = state.read().error_message.clone();

    rsx! {
        div { class: "preview-panel",
            div { class: "preview-toolbar",
                div { class: "preview-toolbar-title",
                    span { class: "preview-title-dot" }
                    span { "Preview" }
                    if is_dirty {
                        span { class: "preview-status stale", "Stale" }
                    } else if is_previewing {
                        span { class: "preview-status running", "Running" }
                    } else {
                        span { class: "preview-status", "Ready" }
                    }
                }
                div { class: "preview-actions",
                    button {
                        class: "preview-action primary",
                        title: "Refresh Preview",
                        onclick: {
                            let services = services.clone();
                            move |_| refresh_docked_preview(state, services.clone())
                        },
                        "Refresh"
                    }
                    button {
                        class: "preview-action",
                        title: "Open Preview in a separate window",
                        onclick: {
                            let services = services.clone();
                            move |_| pop_out_preview(state, services.clone())
                        },
                        "Pop Out"
                    }
                    button {
                        class: "preview-action",
                        title: "Stop Preview",
                        onclick: {
                            let services = services.clone();
                            move |_| stop_preview(state, &services)
                        },
                        "Stop"
                    }
                }
            }

            if let Some(message) = error {
                div { class: "preview-error", "{message}" }
            }

            div {
                id: PREVIEW_HOST_ID,
                class: "preview-host",
                if !is_previewing {
                    div { class: "preview-placeholder",
                        div { class: "preview-placeholder-title", "Preview is idle" }
                        div { class: "preview-placeholder-subtitle", "Run or refresh preview to render the active script." }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_process_uses_current_studio_artifact() {
        let command =
            preview_process_command(Path::new("/tmp/soyuz-studio"), Path::new("/tmp/smoke.rhai"));

        assert_eq!(command.get_program(), Path::new("/tmp/soyuz-studio"));

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(args, ["--preview", "--script", "/tmp/smoke.rhai"]);
        assert!(!args.iter().any(|arg| arg == "cargo"));
        assert!(!args.iter().any(|arg| arg.contains("soyuz-preview")));
    }

    #[test]
    fn embedded_preview_process_uses_current_studio_artifact() {
        let command = embedded_preview_process_command(
            Path::new("/tmp/soyuz-studio"),
            Path::new("/tmp/smoke.rhai"),
            42,
            DomRect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
        );

        assert_eq!(command.get_program(), Path::new("/tmp/soyuz-studio"));

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "--embedded-preview",
                "--script",
                "/tmp/smoke.rhai",
                "--parent",
                "42",
                "--x",
                "10",
                "--y",
                "20",
                "--width",
                "640",
                "--height",
                "480"
            ]
        );
    }
}
