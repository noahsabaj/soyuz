//! Preview window management - spawning and controlling the preview process

use crate::state::{AppState, TerminalLevel};
use dioxus::prelude::*;
use soyuz_engine::Engine;
use std::path::Path;
use std::process::{Child, Command};

/// Spawn the preview window with the current script
pub fn spawn_preview(mut state: Signal<AppState>) {
    let code = state.read().code();

    // Log to terminal
    state.read().terminal_log(TerminalLevel::Info, "Starting preview...");

    // Write script to temp file
    let temp_path = std::env::temp_dir().join("soyuz_preview.rhai");
    if let Err(e) = std::fs::write(&temp_path, &code) {
        let error_msg = format!("Failed to write temp script: {}", e);
        state.read().terminal_log(TerminalLevel::Error, &error_msg);
        tracing::error!("{}", error_msg);
        return;
    }

    // Mark as previewing and clear errors
    {
        let mut s = state.write();
        s.is_previewing = true;
        s.error_message = None;
    }

    // Validate script first using Engine
    let engine = Engine::new();
    if let Err(e) = engine.compile(&code) {
        let error_msg = format!("Script validation error: {}", e);
        state.read().terminal_log(TerminalLevel::Error, &error_msg);
        state.write().error_message = Some(e.to_string());
    }

    // Spawn preview process
    match spawn_preview_process(&temp_path) {
        Ok(child) => {
            state.read().terminal_log(TerminalLevel::Info, "Preview window opened");

            // Store the process handle
            let process_handle = state.read().preview_process.clone();
            *process_handle.lock() = Some(child);

            // Watch for process exit
            let process_handle_wait = state.read().preview_process.clone();
            spawn(async move {
                wait_for_process_exit(state, process_handle_wait).await;
            });
        }
        Err(e) => {
            let error_msg = format!("Failed to spawn preview: {}", e);
            state.read().terminal_log(TerminalLevel::Error, &error_msg);
            tracing::error!("{}", error_msg);
            let mut s = state.write();
            s.error_message = Some(format!("Failed to spawn preview: {}", e));
            s.is_previewing = false;
        }
    }
}

/// Wait for the preview process to exit and update state
async fn wait_for_process_exit(
    mut state: Signal<AppState>,
    process_handle: std::sync::Arc<parking_lot::Mutex<Option<Child>>>,
) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut guard = process_handle.lock();
        if let Some(ref mut process) = *guard {
            match process.try_wait() {
                Ok(Some(status)) => {
                    // Process exited
                    *guard = None;
                    drop(guard);
                    if status.success() {
                        state.read().terminal_log(TerminalLevel::Info, "Preview closed");
                    } else {
                        state.read().terminal_log(
                            TerminalLevel::Warn,
                            format!("Preview exited with status: {}", status),
                        );
                    }
                    state.write().is_previewing = false;
                    break;
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    // Error checking status
                    *guard = None;
                    drop(guard);
                    state.read().terminal_log(
                        TerminalLevel::Error,
                        format!("Error checking preview status: {}", e),
                    );
                    state.write().is_previewing = false;
                    break;
                }
            }
        } else {
            // Process was killed externally
            drop(guard);
            state.read().terminal_log(TerminalLevel::Info, "Preview stopped");
            state.write().is_previewing = false;
            break;
        }
    }
}

/// Spawn the preview as a separate window process
fn spawn_preview_process(script_path: &Path) -> Result<Child, std::io::Error> {
    // Try to find the soyuz-preview binary next to the main executable
    let exe_path = std::env::current_exe().ok();
    let preview_path = exe_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join("soyuz-preview"))
        .filter(|p| p.exists());

    if let Some(preview_bin) = preview_path {
        Command::new(&preview_bin)
            .arg("--script")
            .arg(script_path)
            .spawn()
    } else {
        // Fallback: try cargo run (for development)
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("soyuz-app")
            .arg("--bin")
            .arg("soyuz-preview")
            .arg("--release")
            .arg("--")
            .arg("--script")
            .arg(script_path)
            .spawn()
    }
}
