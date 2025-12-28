//! Preview window management - spawning and controlling the preview process

use crate::state::{AppState, PreviewProcess};
use dioxus::prelude::*;
use std::path::Path;
use std::process::{Child, Command};

/// Spawn the preview window with the current script
pub fn spawn_preview(mut state: Signal<AppState>) {
    let code = state.read().code();

    // Write script to temp file
    let temp_path = std::env::temp_dir().join("soyuz_preview.rhai");
    if let Err(e) = std::fs::write(&temp_path, &code) {
        tracing::error!("Failed to write temp script: {}", e);
        return;
    }

    // Mark as previewing and clear errors
    {
        let mut s = state.write();
        s.is_previewing = true;
        s.has_error = false;
        s.error_message = None;
    }

    // Validate script first
    let engine = soyuz_script::ScriptEngine::new();
    if let Err(e) = engine.compile(&code) {
        let mut s = state.write();
        s.has_error = true;
        s.error_message = Some(e.to_string());
    }

    // Spawn preview process
    match spawn_preview_process(&temp_path) {
        Ok(child) => {
            // Store the process handle
            let process_handle = state.read().preview_process.clone();
            *process_handle.lock() = Some(PreviewProcess::new(child));

            // Watch for process exit
            let process_handle_wait = state.read().preview_process.clone();
            spawn(async move {
                wait_for_process_exit(state, process_handle_wait).await;
            });
        }
        Err(e) => {
            tracing::error!("Failed to spawn preview: {}", e);
            let mut s = state.write();
            s.has_error = true;
            s.error_message = Some(format!("Failed to spawn preview: {}", e));
            s.is_previewing = false;
        }
    }
}

/// Wait for the preview process to exit and update state
async fn wait_for_process_exit(
    mut state: Signal<AppState>,
    process_handle: std::sync::Arc<parking_lot::Mutex<Option<PreviewProcess>>>,
) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut guard = process_handle.lock();
        if let Some(ref mut process) = *guard {
            match process.child.try_wait() {
                Ok(Some(_status)) => {
                    // Process exited
                    *guard = None;
                    drop(guard);
                    state.write().is_previewing = false;
                    break;
                }
                Ok(None) => {
                    // Still running
                }
                Err(_) => {
                    // Error checking status
                    *guard = None;
                    drop(guard);
                    state.write().is_previewing = false;
                    break;
                }
            }
        } else {
            // Process was killed externally
            drop(guard);
            state.write().is_previewing = false;
            break;
        }
    }
}

/// Spawn the preview as a separate window process
fn spawn_preview_process(script_path: &Path) -> Result<Child, std::io::Error> {
    // Try to find the soyuz CLI binary
    let exe_path = std::env::current_exe().ok();
    let cli_path = exe_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join("soyuz"))
        .filter(|p| p.exists());

    if let Some(cli) = cli_path {
        Command::new(&cli)
            .arg("preview")
            .arg("--script")
            .arg(script_path)
            .spawn()
    } else {
        // Fallback: try cargo run
        Command::new("cargo")
            .arg("run")
            .arg("-p")
            .arg("soyuz-cli")
            .arg("--release")
            .arg("--")
            .arg("preview")
            .arg("--script")
            .arg(script_path)
            .spawn()
    }
}
