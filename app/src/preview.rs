//! Preview management - docked preview tab plus pop-out fallback.

use crate::js_interop::{self, DomRect};
use crate::services::{AppServices, debug_timestamp};
use crate::state::{AppStore, TerminalLevel};
use dioxus::core::spawn_forever;
use dioxus::desktop::wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use dioxus::prelude::*;
use soyuz_engine::Engine;
use std::path::Path;
use std::process::{Child, Command};

const PREVIEW_HOST_ID: &str = "soyuz-preview-host";
const PREVIEW_HOST_RETRY_ATTEMPTS: usize = 20;
const PREVIEW_HOST_RETRY_DELAY_MS: u64 = 50;

/// Per-process temporary path for the preview script.
///
/// Keyed by the studio process id so the writer here and the cleanup in
/// `AppServices::stop_preview_process` agree on the same path, and concurrent
/// Soyuz instances do not clobber each other's preview script.
pub(crate) fn preview_temp_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("soyuz_preview_{}.rhai", std::process::id()))
}

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

#[allow(clippy::too_many_lines)] // One linear spawn sequence with diagnostics
fn spawn_preview_with_code(
    mut state: AppStore,
    services: AppServices,
    code: &str,
    placement: PreviewPlacement,
) {
    services.stop_preview_process();
    // F53: claim a new preview generation. Any wait-loop from a previous preview
    // will see the generation has moved on and stop mutating shared state, so it
    // can no longer flip `is_previewing` off under this newer preview.
    let generation = services.bump_preview_generation();
    eprintln!(
        "[soyuz-studio @{}] preview spawn requested ({}, generation {generation})",
        debug_timestamp(),
        match placement {
            PreviewPlacement::Docked => "docked",
            PreviewPlacement::PopOut => "pop-out",
        }
    );
    services.terminal_log(TerminalLevel::Info, "Starting preview...");

    let temp_path = preview_temp_path();
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

    // spawn() ties the task to the *calling component's* scope — and this is
    // called from e.g. the editor's Ctrl+Enter handler, where opening the
    // Preview tab immediately unmounts the editor and silently cancels its
    // tasks before the preview ever spawns. The task also owns the child
    // process bookkeeping for the preview's whole lifetime, so it must not
    // die with whichever component happened to start it.
    spawn_forever(async move {
        eprintln!(
            "[soyuz-studio @{}] preview spawn task started (generation {generation}, parent {:?})",
            debug_timestamp(),
            parent_handle
        );
        let rect = if matches!(placement, PreviewPlacement::Docked) {
            wait_for_preview_host_rect().await
        } else {
            None
        };

        match placement {
            PreviewPlacement::Docked => {
                let Some(parent) = parent_handle else {
                    // No X11 parent handle (e.g. a Wayland session): the preview
                    // can't be embedded into the main window. Rather than leave a
                    // dead pane, open a pop-out window automatically — it works on
                    // any session.
                    services.terminal_log(
                        TerminalLevel::Info,
                        "Docked preview needs an X11 parent window, which this session (e.g. Wayland) doesn't expose \u{2014} opening a pop-out preview window instead. Tip: launch with GDK_BACKEND=x11 for an embedded docked preview.",
                    );
                    run_popout_preview(state, services, &temp_path, generation).await;
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

                services.terminal_log(
                    TerminalLevel::Info,
                    format!(
                        "Preview docked at {}x{}+{}+{} (physical px)",
                        rect.width, rect.height, rect.x, rect.y
                    ),
                );
                services.record_embedded_dock_pos(rect.x, rect.y);
                services.record_embedded_parent(parent);
                services.set_preview_process(child);
                services.set_preview_docked(true);
                discover_embedded_preview_xid(services.clone(), parent, generation);

                let process_handle_wait = services.preview_process();
                wait_for_process_exit(state, services, process_handle_wait, generation).await;
            }
            PreviewPlacement::PopOut => {
                run_popout_preview(state, services, &temp_path, generation).await;
            }
        }
    });
}

/// Re-dock the running embedded preview at the current host rect (pane splits,
/// window resizes, and panel toggles all move it). Reuses the already-written
/// preview script so the rendered content is unchanged; validation, dirty and
/// error state are deliberately untouched — this repositions, it doesn't re-run.
fn reposition_docked_preview(mut state: AppStore, services: AppServices) {
    if !services.preview_is_docked() || !state.read().is_previewing {
        return;
    }
    let Some(parent) = preview_parent_handle() else {
        return;
    };

    // stop_preview_process removes the temp script as part of its cleanup, so
    // capture the script first and restore it for the respawn.
    let temp_path = preview_temp_path();
    let Ok(script) = std::fs::read_to_string(&temp_path) else {
        return;
    };
    eprintln!(
        "[soyuz-studio @{}] repositioning docked preview (host rect changed)",
        debug_timestamp()
    );
    services.stop_preview_process();
    let generation = services.bump_preview_generation();
    if let Err(e) = std::fs::write(&temp_path, script) {
        let error_msg = format!("Failed to restore preview script: {e}");
        services.terminal_log(TerminalLevel::Error, &error_msg);
        let mut s = state.write();
        s.error_message = Some(error_msg);
        s.is_previewing = false;
        return;
    }

    // spawn_forever: this task kills the old child and spawns the new one; a
    // scope-tied task cancelled between those two steps would leave the
    // preview dead (see the matching comment in `spawn_preview_with_code`).
    spawn_forever(async move {
        let rect = wait_for_preview_host_rect().await;
        let Some(rect) = rect.filter(|r| r.width >= 1 && r.height >= 1) else {
            report_docked_preview_unavailable(
                state,
                &services,
                "Docked preview host disappeared while repositioning. Click Refresh to restart the preview.",
            );
            return;
        };

        match spawn_embedded_preview_process(&temp_path, parent, rect) {
            Ok(child) => {
                services.record_embedded_dock_pos(rect.x, rect.y);
                services.record_embedded_parent(parent);
                services.set_preview_process(child);
                services.set_preview_docked(true);
                discover_embedded_preview_xid(services.clone(), parent, generation);

                let process_handle_wait = services.preview_process();
                wait_for_process_exit(state, services, process_handle_wait, generation).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to reposition docked preview: {e}");
                services.terminal_log(TerminalLevel::Error, &error_msg);
                tracing::error!("{error_msg}");
                let mut s = state.write();
                s.error_message = Some(error_msg);
                s.is_previewing = false;
            }
        }
    });
}

/// Discover the X11 window id of a freshly-spawned embedded preview child.
///
/// The child creates its window a few hundred milliseconds after spawn, as a
/// child of the studio's X11 window carrying winit's `_XEMBED` property. Poll
/// the studio window's children for it off the UI thread, then hand the id to
/// [`AppServices`] so the studio can raise it (settling WebKitGTK's subwindow
/// shuffle) and keep it below DOM overlays like the command palette.
#[cfg(target_os = "linux")]
fn discover_embedded_preview_xid(services: AppServices, parent: u32, generation: u64) {
    std::thread::spawn(move || {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

        let Ok((conn, _)) = x11rb::connect(None) else {
            return;
        };
        let Ok(cookie) = conn.intern_atom(false, b"_XEMBED") else {
            return;
        };
        let Ok(reply) = cookie.reply() else {
            return;
        };
        let xembed = reply.atom;

        for _ in 0..30 {
            if services.preview_generation() != generation {
                return;
            }
            let found = conn
                .query_tree(parent)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .and_then(|tree| {
                    // Children are returned bottom-to-top; prefer the topmost.
                    tree.children.iter().rev().copied().find(|&child| {
                        conn.get_property(false, child, xembed, AtomEnum::ANY, 0, 2)
                            .ok()
                            .and_then(|cookie| cookie.reply().ok())
                            .is_some_and(|prop| prop.value_len > 0)
                    })
                });
            if let Some(xid) = found {
                services.record_embedded_preview_xid(xid);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // Without the xid the studio can't park the preview under overlays or
        // heal its position/focus; say so instead of failing silently.
        eprintln!(
            "[soyuz-studio @{}] embedded preview xid discovery FAILED after 3s (parent {parent:#x}, generation {generation})",
            crate::services::debug_timestamp()
        );
    });
}

#[cfg(not(target_os = "linux"))]
fn discover_embedded_preview_xid(_services: AppServices, _parent: u32, _generation: u64) {}

async fn wait_for_preview_host_rect() -> Option<DomRect> {
    let mut probe_timeouts = 0usize;
    for attempt in 1..=PREVIEW_HOST_RETRY_ATTEMPTS {
        // A document::eval issued in the same tick the Preview tab mounts can
        // hang forever (the webview is still applying the DOM update), which
        // used to wedge the whole spawn task on the first Ctrl+Enter. Bound
        // each probe and retry: by the next attempt the webview has settled.
        let rect = match tokio::time::timeout(
            tokio::time::Duration::from_millis(200),
            js_interop::get_element_rect(PREVIEW_HOST_ID),
        )
        .await
        {
            Ok(rect) => {
                if rect.is_none() {
                    eprintln!(
                        "[soyuz-studio @{}] host rect probe {attempt}: host element not in DOM yet",
                        debug_timestamp()
                    );
                }
                rect
            }
            Err(_elapsed) => {
                probe_timeouts += 1;
                eprintln!(
                    "[soyuz-studio @{}] host rect probe {attempt}: eval timed out (webview busy)",
                    debug_timestamp()
                );
                None
            }
        };

        if let Some(rect) = rect
            && rect.width > 0
            && rect.height > 0
        {
            eprintln!(
                "[soyuz-studio @{}] preview host rect {}x{}+{}+{} (attempt {attempt}, {probe_timeouts} probe timeouts)",
                debug_timestamp(),
                rect.width,
                rect.height,
                rect.x,
                rect.y
            );
            return Some(rect);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(
            PREVIEW_HOST_RETRY_DELAY_MS,
        ))
        .await;
    }

    eprintln!(
        "[soyuz-studio @{}] preview host rect NOT found after {PREVIEW_HOST_RETRY_ATTEMPTS} attempts ({probe_timeouts} probe timeouts)",
        debug_timestamp()
    );
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

/// Spawn a standalone pop-out preview window and track it to completion. Used
/// both for an explicit Pop Out and as the fallback when a docked preview can't
/// obtain an X11 parent handle (e.g. on a Wayland session).
async fn run_popout_preview(
    mut state: AppStore,
    services: AppServices,
    script_path: &Path,
    generation: u64,
) {
    match spawn_preview_process(script_path) {
        Ok(child) => {
            services.terminal_log(TerminalLevel::Info, "Preview window opened");
            services.set_preview_process(child);
            services.set_preview_docked(false);

            let process_handle_wait = services.preview_process();
            wait_for_process_exit(state, services, process_handle_wait, generation).await;
        }
        Err(e) => {
            let error_msg = format!("Failed to spawn preview: {e}");
            services.terminal_log(TerminalLevel::Error, &error_msg);
            tracing::error!("{error_msg}");
            let mut s = state.write();
            s.error_message = Some(error_msg);
            s.is_previewing = false;
        }
    }
}

async fn wait_for_process_exit(
    state: AppStore,
    services: AppServices,
    process_handle: std::sync::Arc<parking_lot::Mutex<Option<Child>>>,
    generation: u64,
) {
    let mut tick: u32 = 0;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // F53: a newer preview superseded this one. Stop polling and leave
        // `is_previewing` for the current generation's wait-loop to manage.
        if services.preview_generation() != generation {
            return;
        }

        // Every tick, reclaim keyboard focus if preview interaction broke it
        // (shortcut deadness is user-visible immediately, so heal fast).
        // Twice a second, heal any divergence between the embedded child's
        // actual position and the intended one (docked, or parked offscreen
        // while an overlay is open).
        services.reassert_embedded_focus();
        tick = tick.wrapping_add(1);
        if tick.is_multiple_of(5) {
            services.reassert_embedded_visibility();
        }

        let mut guard = process_handle.lock();
        if let Some(ref mut process) = *guard {
            match process.try_wait() {
                Ok(Some(status)) => {
                    *guard = None;
                    drop(guard);
                    eprintln!(
                        "[soyuz-studio @{}] preview child exited: {status} (generation {generation})",
                        debug_timestamp()
                    );
                    if status.success() {
                        services.terminal_log(TerminalLevel::Info, "Preview closed");
                    } else {
                        services.terminal_log(
                            TerminalLevel::Warn,
                            format!("Preview exited with status: {status}"),
                        );
                    }
                    set_idle_if_current(state, &services, generation);
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
                    set_idle_if_current(state, &services, generation);
                    break;
                }
            }
        } else {
            drop(guard);
            services.terminal_log(TerminalLevel::Info, "Preview stopped");
            set_idle_if_current(state, &services, generation);
            break;
        }
    }
}

/// Clear `is_previewing`, but only while this wait-loop's preview is still the
/// current generation, so a stale loop can never flip the flag off after a newer
/// preview has already set it true (F53).
fn set_idle_if_current(mut state: AppStore, services: &AppServices, generation: u64) {
    if services.preview_generation() == generation {
        state.write().is_previewing = false;
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

/// Debounced ResizeObserver on the preview host div. Pushes an event through
/// `dioxus.send` after the host rect settles so the embedded preview child can
/// be re-docked at the new geometry. The initial observe callback is skipped —
/// mounting the panel must not respawn a healthy preview.
const PREVIEW_RESIZE_OBSERVER_JS: &str = r#"
    (function attach(tries) {
        const host = document.getElementById('soyuz-preview-host');
        if (!host) {
            // The eval can run before the freshly-mounted Preview tab reaches
            // the real DOM; retry briefly instead of silently not observing.
            if (tries > 0) { setTimeout(() => attach(tries - 1), 50); }
            return;
        }
        if (window.__soyuzPreviewResize) { window.__soyuzPreviewResize.teardown(); }
        let timer = null;
        let first = true;
        const observer = new ResizeObserver(() => {
            if (first) { first = false; return; }
            if (timer) { clearTimeout(timer); }
            timer = setTimeout(() => { dioxus.send({}); }, 250);
        });
        observer.observe(host);
        window.__soyuzPreviewResize = {
            teardown: () => {
                observer.disconnect();
                if (timer) { clearTimeout(timer); }
                delete window.__soyuzPreviewResize;
            },
        };
    })(20);
"#;

/// Docked preview tab body.
#[component]
pub fn PreviewPanel() -> Element {
    let state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let is_previewing = state.read().is_previewing;
    let is_dirty = state.read().preview_dirty;
    let error = state.read().error_message.clone();

    // Keep the embedded preview child following the host rect: pane splits,
    // window resizes, and panel toggles all change it. The F53 generation
    // counter makes the respawn race-safe.
    let mut resize_eval = use_signal(|| None);
    use_effect({
        let services = services.clone();
        move || {
            let eval = document::eval(PREVIEW_RESIZE_OBSERVER_JS);
            // Keep a handle so use_drop can drop it (releasing the recv channel).
            resize_eval.set(Some(eval));

            let services = services.clone();
            spawn(async move {
                let mut eval = eval;
                while eval.recv::<serde_json::Value>().await.is_ok() {
                    reposition_docked_preview(state, services.clone());
                }
            });
        }
    });
    use_drop({
        let services = services.clone();
        move || {
            // An embedded child has no life without its host pane: when the
            // Preview tab is closed or another tab takes its place, stop the
            // docked preview instead of leaving it floating over the editor.
            // Pop-out previews are unaffected.
            if services.preview_is_docked() {
                services.terminal_log(
                    TerminalLevel::Info,
                    "Preview stopped: the Preview tab was closed or hidden.",
                );
                stop_preview(state, &services);
            }
            let _ = document::eval(
                "if (window.__soyuzPreviewResize) { window.__soyuzPreviewResize.teardown(); }",
            );
            resize_eval.set(None);
        }
    });

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
