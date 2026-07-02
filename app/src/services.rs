//! Runtime services shared by Dioxus UI components.
//!
//! These handles are intentionally kept outside the reactive app store. They
//! represent process and thread-safe IO resources, not UI state.

use crate::state::{TerminalBuffer, TerminalEntry, TerminalLevel};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use tokio::sync::Notify;
use tracing::warn;

/// Non-reactive application services used by the desktop UI.
#[derive(Clone)]
pub struct AppServices {
    terminal_buffer: TerminalBuffer,
    preview_process: Arc<Mutex<Option<std::process::Child>>>,
    /// Monotonic counter identifying the current preview. Bumped on each spawn so
    /// a superseded wait-loop can detect it is stale and stop touching state (F53).
    preview_generation: Arc<AtomicU64>,
    /// Whether the current preview child is embedded (docked) in the main window
    /// rather than a pop-out. Host-rect repositioning only applies when docked.
    preview_docked: Arc<AtomicBool>,
    /// X11 window id of the embedded preview child (0 = unknown/none),
    /// discovered after spawn so the studio can restack it below DOM overlays.
    embedded_preview_xid: Arc<AtomicU32>,
    /// Whether a DOM overlay (command palette) is currently open; while true
    /// the embedded preview is parked offscreen so the overlay wins.
    overlay_open: Arc<AtomicBool>,
    /// Parent-relative position (physical px) where the docked preview child
    /// belongs, captured at each spawn/reposition. Overlay handling moves the
    /// window between this spot and an offscreen parking position.
    embedded_preview_dock_pos: Arc<Mutex<(i32, i32)>>,
    /// X11 window id of the studio toplevel the preview is embedded into
    /// (0 = unknown). Focus healing returns keyboard focus here when a click
    /// in the preview child has captured it.
    embedded_preview_parent: Arc<AtomicU32>,
    /// Wakes the positioning worker that owns the single X connection all
    /// park/dock moves go through (see [`overlay_worker`]). Send errors are
    /// ignored — on non-Linux the receiver doesn't exist.
    overlay_ping: std::sync::mpsc::Sender<()>,
}

impl PartialEq for AppServices {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Drop for AppServices {
    fn drop(&mut self) {
        // `AppServices` is cloned into many UI closures and `Drop` runs for every
        // clone, so only stop the preview when this is the last owner of the shared
        // process handle. Otherwise a transient clone drop would kill a running
        // preview. The explicit window-close path remains the primary cleanup.
        if Arc::strong_count(&self.preview_process) == 1 {
            self.stop_preview_process();
        }
    }
}

impl AppServices {
    /// Create services backed by the tracing terminal buffer.
    pub fn new(terminal_buffer: TerminalBuffer) -> Self {
        let overlay_open = Arc::new(AtomicBool::new(false));
        let embedded_preview_xid = Arc::new(AtomicU32::new(0));
        let embedded_preview_dock_pos = Arc::new(Mutex::new((0, 0)));

        #[cfg(target_os = "linux")]
        let overlay_ping = overlay_worker::spawn(overlay_worker::WorkerShared {
            overlay_open: overlay_open.clone(),
            xid: embedded_preview_xid.clone(),
            dock_pos: embedded_preview_dock_pos.clone(),
        });
        // Without a spawned receiver every send fails, which callers ignore.
        #[cfg(not(target_os = "linux"))]
        let overlay_ping = std::sync::mpsc::channel().0;

        Self {
            terminal_buffer,
            preview_process: Arc::new(Mutex::new(None)),
            preview_generation: Arc::new(AtomicU64::new(0)),
            preview_docked: Arc::new(AtomicBool::new(false)),
            embedded_preview_xid,
            overlay_open,
            embedded_preview_dock_pos,
            embedded_preview_parent: Arc::new(AtomicU32::new(0)),
            overlay_ping,
        }
    }

    /// Record where the docked preview child belongs (parent-relative physical
    /// px). Captured at each docked spawn and reposition so overlay parking
    /// knows where to restore the window.
    pub fn record_embedded_dock_pos(&self, x: i32, y: i32) {
        *self.embedded_preview_dock_pos.lock() = (x, y);
    }

    /// Record the studio toplevel the preview is embedded into, for focus
    /// healing.
    pub fn record_embedded_parent(&self, parent: u32) {
        self.embedded_preview_parent.store(parent, Ordering::SeqCst);
    }

    /// Shared preview process handle.
    pub fn preview_process(&self) -> Arc<Mutex<Option<std::process::Child>>> {
        self.preview_process.clone()
    }

    /// Bump the preview generation and return the new value. Each preview spawn
    /// calls this so a previous `wait_for_process_exit` loop can recognise it was
    /// superseded and exit without clobbering the newer preview's state (F53).
    pub fn bump_preview_generation(&self) -> u64 {
        self.preview_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The current preview generation.
    pub fn preview_generation(&self) -> u64 {
        self.preview_generation.load(Ordering::SeqCst)
    }

    /// Replace the preview process handle. A live child here means two spawn
    /// paths raced (every legitimate spawn calls `stop_preview_process`
    /// first): kill the one being replaced, or it leaks as an orphan window
    /// fighting the new child for the same host rect.
    pub fn set_preview_process(&self, process: std::process::Child) {
        eprintln!(
            "[soyuz-studio @{}] preview child spawned: pid {}",
            debug_timestamp(),
            process.id()
        );
        let mut guard = self.preview_process.lock();
        if let Some(ref mut old) = *guard {
            eprintln!(
                "[soyuz-studio @{}] WARNING: replacing live preview child pid {}; killing it",
                debug_timestamp(),
                old.id()
            );
            let _ = old.kill();
        }
        *guard = Some(process);
    }

    /// Record whether the current preview child is docked (embedded).
    pub fn set_preview_docked(&self, docked: bool) {
        self.preview_docked.store(docked, Ordering::SeqCst);
    }

    /// Whether the current preview child is docked (embedded).
    pub fn preview_is_docked(&self) -> bool {
        self.preview_docked.load(Ordering::SeqCst)
    }

    /// Record the discovered X11 window id of the embedded preview child. If
    /// an overlay is already open, park the child offscreen immediately so it
    /// cannot cover the overlay.
    pub fn record_embedded_preview_xid(&self, xid: u32) {
        eprintln!(
            "[soyuz-studio @{}] embedded preview xid discovered: {xid:#x}",
            debug_timestamp()
        );
        self.embedded_preview_xid.store(xid, Ordering::SeqCst);
        let _ = self.overlay_ping.send(());
    }

    /// Track whether a DOM overlay (e.g. the command palette) is open, parking
    /// the embedded preview child offscreen while it is. An X11 child window
    /// always occludes content its parent paints — GTK renders the webview
    /// client-side into the toplevel, so there are no siblings to restack
    /// against. Hiding by *position* rather than unmapping is deliberate:
    /// something in the WebKitGTK/XWayland stack intermittently re-maps an
    /// unmapped child (observed as the palette flashing behind the preview),
    /// while nothing else ever repositions one. The child keeps rendering
    /// while parked and returns with its camera intact. No-op when the state
    /// hasn't changed, so reactive callers can invoke it freely.
    pub fn set_overlay_open(&self, open: bool) {
        if self.overlay_open.swap(open, Ordering::SeqCst) == open {
            return;
        }
        let xid = self.embedded_preview_xid.load(Ordering::SeqCst);
        eprintln!(
            "[soyuz-studio @{}] overlay {} (preview xid {xid:#x})",
            debug_timestamp(),
            if open { "opened" } else { "closed" }
        );
        let _ = self.overlay_ping.send(());
    }

    /// Converge the embedded child's actual X11 position onto the intended
    /// one (docked unless an overlay is open, parked offscreen otherwise). A
    /// single request can get lost in the WebKitGTK/X11 shuffle; the preview
    /// wait-loop calls this periodically so any divergence heals within a
    /// tick instead of sticking until the next manual Refresh. The worker
    /// recomputes the desired state itself, so a stale ping is harmless.
    pub fn reassert_embedded_visibility(&self) {
        if !self.preview_is_docked() {
            return;
        }
        if self.embedded_preview_xid.load(Ordering::SeqCst) == 0 {
            return;
        }
        let _ = self.overlay_ping.send(());
    }

    /// Reclaim X11 keyboard focus lost to the embedded preview interaction.
    ///
    /// Clicking or dragging in the preview child can leave X input focus in a
    /// degenerate state — on the child, on `PointerRoot` (keys follow the
    /// mouse, sinking into whatever the pointer hovers), on `None` (keys
    /// dropped), or on a WM-internal window. In every one of those states the
    /// studio's keyboard shortcuts go dead until the user clicks the webview.
    /// This runs on the preview wait-loop tick: when the WM reports the
    /// studio as the *active* window but X focus is not on the studio
    /// toplevel (or a descendant other than the preview child), focus is
    /// broken and gets pulled back. The active-window gate means focus is
    /// never stolen from another application.
    pub fn reassert_embedded_focus(&self) {
        if !self.preview_is_docked() {
            return;
        }
        let xid = self.embedded_preview_xid.load(Ordering::SeqCst);
        let parent = self.embedded_preview_parent.load(Ordering::SeqCst);
        if xid == 0 || parent == 0 {
            return;
        }
        reassert_embedded_focus_impl(xid, parent);
    }

    /// Stop the preview process if one is running.
    pub fn stop_preview_process(&self) {
        self.preview_docked.store(false, Ordering::SeqCst);
        self.embedded_preview_xid.store(0, Ordering::SeqCst);
        // Let the positioning worker forget the window it was managing.
        let _ = self.overlay_ping.send(());
        let mut guard = self.preview_process.lock();
        if let Some(ref mut process) = *guard {
            eprintln!(
                "[soyuz-studio @{}] stopping preview child pid {}",
                debug_timestamp(),
                process.id()
            );
            if let Err(e) = process.kill() {
                warn!("Failed to kill preview process: {e}");
            }
        }
        *guard = None;
        drop(guard);

        // Best-effort cleanup of the per-process temp script file.
        let _ = std::fs::remove_file(crate::preview::preview_temp_path());
    }

    /// Add a message to the terminal output.
    pub fn terminal_log(&self, level: TerminalLevel, message: impl Into<String>) {
        self.terminal_buffer
            .push(TerminalEntry::new(level, message));
    }

    /// Clear the terminal output.
    pub fn terminal_clear(&self) {
        self.terminal_buffer.clear();
    }

    /// Snapshot terminal output with stable per-entry ids for rendering (F72).
    pub fn terminal_snapshot_with_ids(&self) -> Vec<(u64, TerminalEntry)> {
        self.terminal_buffer.snapshot_with_ids()
    }

    /// Handle to await the next terminal output change (F72).
    pub fn terminal_notifier(&self) -> Arc<Notify> {
        self.terminal_buffer.notifier()
    }
}

/// Wall-clock UTC `HH:MM:SS.mmm` for stderr diagnostics. Matches the preview
/// child's breadcrumb timestamps (also UTC) so the studio's spawn/stop/kill
/// decisions and each child's startup sequence line up on one timeline.
pub(crate) fn debug_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    format!(
        "{:02}:{:02}:{:02}.{:03}Z",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        now.subsec_millis()
    )
}

/// Parking position (parent-relative) for the embedded preview child while a
/// DOM overlay is open. Far enough to be fully outside any plausible window,
/// within X11's i16 coordinate range.
#[cfg(target_os = "linux")]
const OFFSCREEN_POS: i32 = -30_000;

/// The preview positioning worker: one thread, one persistent X connection,
/// through which every park/dock move goes.
///
/// Requests on a single X connection are processed by the server strictly in
/// order. The earlier design issued moves from short-lived connections in
/// several places (overlay transitions, a delayed-restore thread, the
/// periodic reassert), and the server may interleave requests from different
/// connections in arrival order — under rapid palette toggling a stale
/// "dock" could land after a newer "park", flashing the palette behind the
/// preview. The worker eliminates that class of race by construction: pings
/// just say "state changed"; the worker recomputes the desired position from
/// the shared atomics immediately before each move, so late or coalesced
/// pings are harmless.
#[cfg(target_os = "linux")]
mod overlay_worker {
    use parking_lot::Mutex;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::mpsc::{Receiver, Sender, channel};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt};
    use x11rb::rust_connection::RustConnection;

    /// Shared state the worker reads its desired position from. Deliberately
    /// not the whole `AppServices`: holding only these Arcs keeps the worker
    /// out of the `Drop` last-owner bookkeeping on the process handle.
    pub(super) struct WorkerShared {
        pub overlay_open: Arc<AtomicBool>,
        pub xid: Arc<AtomicU32>,
        pub dock_pos: Arc<Mutex<(i32, i32)>>,
    }

    pub(super) fn spawn(shared: WorkerShared) -> Sender<()> {
        let (tx, rx) = channel();
        std::thread::spawn(move || run(&shared, &rx));
        tx
    }

    fn desired(shared: &WorkerShared) -> (u32, bool) {
        (
            shared.xid.load(Ordering::SeqCst),
            !shared.overlay_open.load(Ordering::SeqCst),
        )
    }

    fn run(shared: &WorkerShared, rx: &Receiver<()>) {
        let mut conn: Option<RustConnection> = None;
        // Last (xid, visible) actually applied; used to detect transitions
        // (for the restore delay) and to keep steady-state pings quiet.
        let mut last_applied: Option<(u32, bool)> = None;

        // Exits when the last AppServices clone drops the sender.
        while rx.recv().is_ok() {
            while rx.try_recv().is_ok() {} // coalesce queued pings

            let (xid, mut visible) = desired(shared);
            if xid == 0 {
                last_applied = None;
                continue;
            }

            // Restoring after an overlay close waits for the webview to
            // paint the overlay away: an X move is instant while the DOM
            // removal takes a frame or three, and an immediate restore puts
            // the preview back over the still-painted overlay. Re-read the
            // desired state after the wait so a rapid re-open wins.
            if visible && last_applied == Some((xid, false)) {
                std::thread::sleep(std::time::Duration::from_millis(90));
                while rx.try_recv().is_ok() {}
                let (new_xid, new_visible) = desired(shared);
                if new_xid != xid {
                    last_applied = None;
                    continue;
                }
                visible = new_visible;
            }

            if conn.is_none() {
                conn = x11rb::connect(None).ok().map(|(c, _)| c);
            }
            let Some(c) = conn.as_ref() else {
                eprintln!(
                    "[soyuz-studio @{}] positioning worker: X connection FAILED",
                    super::debug_timestamp()
                );
                continue;
            };

            let (dock_x, dock_y) = *shared.dock_pos.lock();
            let (x, y) = if visible {
                (dock_x, dock_y)
            } else {
                (super::OFFSCREEN_POS, super::OFFSCREEN_POS)
            };

            // Cheap drift diagnostic on the same ordered connection: worth a
            // log line when the window is somewhere it shouldn't be even
            // though no transition happened.
            if last_applied == Some((xid, visible))
                && let Ok(cookie) = c.get_geometry(xid)
                && let Ok(geom) = cookie.reply()
                && (i32::from(geom.x), i32::from(geom.y)) != (x, y)
            {
                eprintln!(
                    "[soyuz-studio @{}] position drift: preview {xid:#x} at ({}, {}), moving to ({x}, {y})",
                    super::debug_timestamp(),
                    geom.x,
                    geom.y
                );
            }

            let sent = c
                .configure_window(xid, &ConfigureWindowAux::new().x(x).y(y))
                .is_ok();
            let flushed = c.flush().is_ok();
            if !(sent && flushed) {
                // Connection went bad; reconnect on the next ping.
                conn = None;
                last_applied = None;
                eprintln!(
                    "[soyuz-studio @{}] positioning worker: move failed (sent: {sent}, flushed: {flushed}); will reconnect",
                    super::debug_timestamp()
                );
                continue;
            }
            if last_applied != Some((xid, visible)) {
                eprintln!(
                    "[soyuz-studio @{}] {} preview {xid:#x} at ({x}, {y})",
                    super::debug_timestamp(),
                    if visible { "docking" } else { "parking" }
                );
            }
            last_applied = Some((xid, visible));
        }
    }
}

/// See [`AppServices::reassert_embedded_focus`]. `xid` is the preview child,
/// `parent` the studio toplevel it is embedded into.
#[cfg(target_os = "linux")]
fn reassert_embedded_focus_impl(xid: u32, parent: u32) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, InputFocus};

    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return;
    };
    let Ok(focus_cookie) = conn.get_input_focus() else {
        return;
    };
    let Ok(focus_reply) = focus_cookie.reply() else {
        return;
    };
    let focus = focus_reply.focus;
    // Fast path: focus is where it belongs.
    if focus == parent {
        return;
    }

    // Only intervene while the WM says the studio is the active window;
    // otherwise the user has genuinely switched apps and focus is not ours.
    let root = conn.setup().roots[screen_num].root;
    let Ok(atom_cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") else {
        return;
    };
    let Ok(atom_reply) = atom_cookie.reply() else {
        return;
    };
    let active = conn
        .get_property(false, root, atom_reply.atom, AtomEnum::WINDOW, 0, 1)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .and_then(|prop| prop.value32().and_then(|mut values| values.next()));
    if active != Some(parent) {
        return;
    }

    // The studio is active but X focus is elsewhere. A descendant of the
    // toplevel holding focus is legitimate (GTK focus sub-windows) — except
    // the preview child itself, which must never hold keyboard focus. Walk a
    // bounded ancestor chain; anything that never reaches the toplevel
    // (PointerRoot=1, None=0, the child, WM-internal windows) is the broken
    // state this heals.
    if focus != xid && focus > 1 {
        let mut window = focus;
        for _ in 0..16 {
            if window == parent {
                return; // descendant of the toplevel — legitimate focus
            }
            let Ok(cookie) = conn.query_tree(window) else {
                break;
            };
            let Ok(tree) = cookie.reply() else {
                break;
            };
            if window == tree.root || tree.parent == 0 {
                break; // reached the root without passing the toplevel
            }
            window = tree.parent;
        }
    }

    eprintln!(
        "[soyuz-studio @{}] keyboard focus lost to {focus:#x} while the studio is active - reclaiming",
        debug_timestamp()
    );
    let _ = conn.set_input_focus(InputFocus::PARENT, parent, x11rb::CURRENT_TIME);
    let _ = conn.flush();
}

#[cfg(not(target_os = "linux"))]
fn reassert_embedded_focus_impl(_xid: u32, _parent: u32) {}
