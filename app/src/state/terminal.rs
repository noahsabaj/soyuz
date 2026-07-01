//! Terminal output buffer state management
//!
//! Stores terminal entries with log levels, timestamps, and message content.
//! Provides thread-safe access for the tracing subscriber layer.

use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Notify;

/// Maximum number of terminal entries to retain
pub const MAX_TERMINAL_ENTRIES: usize = 1000;

/// Filter settings for terminal output levels
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalFilter {
    pub show_info: bool,
    pub show_warn: bool,
    pub show_error: bool,
}

impl Default for TerminalFilter {
    fn default() -> Self {
        Self {
            show_info: true,
            show_warn: true,
            show_error: true,
        }
    }
}

impl TerminalFilter {
    /// Check if a given level should be shown
    pub fn should_show(&self, level: TerminalLevel) -> bool {
        match level {
            TerminalLevel::Trace | TerminalLevel::Debug => false, // Never show trace/debug
            TerminalLevel::Info => self.show_info,
            TerminalLevel::Warn => self.show_warn,
            TerminalLevel::Error => self.show_error,
        }
    }

    /// Toggle a specific level's visibility
    pub fn toggle(&mut self, level: TerminalLevel) {
        match level {
            TerminalLevel::Info => self.show_info = !self.show_info,
            TerminalLevel::Warn => self.show_warn = !self.show_warn,
            TerminalLevel::Error => self.show_error = !self.show_error,
            _ => {}
        }
    }
}

/// Log level for terminal entries
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl TerminalLevel {
    /// Short prefix for display
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl From<tracing::Level> for TerminalLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE => Self::Trace,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::INFO => Self::Info,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::ERROR => Self::Error,
        }
    }
}

/// A single terminal output entry
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalEntry {
    /// Timestamp when the entry was created (seconds since UNIX epoch)
    pub timestamp_secs: u64,
    /// Log level
    pub level: TerminalLevel,
    /// Message content
    pub message: String,
    /// Optional target/module name
    pub target: Option<String>,
}

impl TerminalEntry {
    /// Create a new terminal entry with current timestamp
    pub fn new(level: TerminalLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp_secs: Self::current_timestamp(),
            level,
            message: message.into(),
            target: None,
        }
    }

    /// Create with target/module name
    pub fn with_target(level: TerminalLevel, message: impl Into<String>, target: String) -> Self {
        Self {
            timestamp_secs: Self::current_timestamp(),
            level,
            message: message.into(),
            target: Some(target),
        }
    }

    /// Get current timestamp as seconds since UNIX epoch
    fn current_timestamp() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Format timestamp for display with given timezone offset and format
    /// - `timezone_offset`: hours offset from UTC (-12 to +14)
    /// - `use_24h`: true for 24-hour format, false for 12-hour with AM/PM
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub fn format_timestamp(&self, timezone_offset: i8, use_24h: bool) -> String {
        // Apply timezone offset (convert hours to seconds)
        // These casts are safe: timestamp_secs won't overflow i64, and result is always positive
        // after adding timezone offset (max -12 hours = -43200 seconds, negligible vs UNIX timestamp)
        let offset_secs = i64::from(timezone_offset) * 3600;
        let adjusted_secs = (self.timestamp_secs as i64 + offset_secs) as u64;

        // Extract time components
        let hours_24 = (adjusted_secs / 3600) % 24;
        let minutes = (adjusted_secs / 60) % 60;
        let seconds = adjusted_secs % 60;

        if use_24h {
            format!("{:02}:{:02}:{:02}", hours_24, minutes, seconds)
        } else {
            let (hours_12, period) = if hours_24 == 0 {
                (12, "AM")
            } else if hours_24 < 12 {
                (hours_24, "AM")
            } else if hours_24 == 12 {
                (12, "PM")
            } else {
                (hours_24 - 12, "PM")
            };
            format!("{:02}:{:02}:{:02} {}", hours_12, minutes, seconds, period)
        }
    }
}

/// Thread-safe terminal buffer for collecting log output
#[derive(Clone)]
pub struct TerminalBuffer {
    entries: Arc<RwLock<Vec<TerminalEntry>>>,
    /// Stable id of `entries[0]`. Increases as the front is trimmed so every
    /// entry keeps a unique, monotonic id usable as a stable list key (F72).
    first_id: Arc<AtomicU64>,
    /// Wakes the terminal panel when output changes so it re-renders live.
    /// Signalled via `notify_one`, which retains a single wake permit when the
    /// panel is not currently parked, so a change is never lost (F72).
    notify: Arc<Notify>,
}

impl Default for TerminalBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalBuffer {
    /// Create a new empty terminal buffer
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::with_capacity(MAX_TERMINAL_ENTRIES))),
            first_id: Arc::new(AtomicU64::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Add an entry to the buffer
    pub fn push(&self, entry: TerminalEntry) {
        {
            let mut entries = self.entries.write();
            entries.push(entry);

            // Trim oldest entries if over limit, advancing the stable id base so
            // surviving entries keep their ids.
            if entries.len() > MAX_TERMINAL_ENTRIES {
                let excess = entries.len() - MAX_TERMINAL_ENTRIES;
                entries.drain(0..excess);
                self.first_id.fetch_add(excess as u64, Ordering::Relaxed);
            }
        }
        self.mark_changed();
    }

    /// Clear all entries
    pub fn clear(&self) {
        {
            let mut entries = self.entries.write();
            // Advance the id base past the cleared entries so a refilled buffer
            // never reuses a key.
            self.first_id
                .fetch_add(entries.len() as u64, Ordering::Relaxed);
            entries.clear();
        }
        self.mark_changed();
    }

    /// Wake the observer waiting on the next change to the buffer.
    ///
    /// Uses `notify_one` rather than `notify_waiters`: `notify_one` stores a
    /// single wake permit when no waiter is currently parked, so a push that
    /// lands between the panel's snapshot and its `notified().await` is still
    /// delivered on the next await. `notify_waiters` only woke already-parked
    /// waiters, dropping that wakeup — the lost wakeup the old 250ms UI poll
    /// used to mask (F72).
    fn mark_changed(&self) {
        self.notify.notify_one();
    }

    /// Handle used to await the next change to the buffer. Consumers should
    /// register interest (e.g. `Notified::enable`) before taking their snapshot
    /// so the permit stored by `notify_one` is observed even when the change
    /// races the snapshot (register-before-snapshot, F72).
    pub fn notifier(&self) -> Arc<Notify> {
        self.notify.clone()
    }

    /// Snapshot paired with each entry's stable id, for use as list keys (F72).
    pub fn snapshot_with_ids(&self) -> Vec<(u64, TerminalEntry)> {
        let base = self.first_id.load(Ordering::Relaxed);
        self.entries
            .read()
            .iter()
            .enumerate()
            .map(|(i, entry)| (base + i as u64, entry.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_level_prefix() {
        assert_eq!(TerminalLevel::Error.prefix(), "ERROR");
        assert_eq!(TerminalLevel::Info.prefix(), "INFO");
    }

    #[test]
    fn test_terminal_buffer_push_and_snapshot() {
        let buffer = TerminalBuffer::new();
        buffer.push(TerminalEntry::new(TerminalLevel::Info, "Test message"));

        let snapshot = buffer.snapshot_with_ids();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].1.message, "Test message");
        assert_eq!(snapshot[0].1.level, TerminalLevel::Info);
        // First entry has stable id 0
        assert_eq!(snapshot[0].0, 0);
        // Verify timestamp is set (non-zero)
        assert!(snapshot[0].1.timestamp_secs > 0);
    }

    #[test]
    fn test_terminal_buffer_clear() {
        let buffer = TerminalBuffer::new();
        buffer.push(TerminalEntry::new(TerminalLevel::Info, "Message 1"));
        buffer.push(TerminalEntry::new(TerminalLevel::Error, "Message 2"));

        assert_eq!(buffer.snapshot_with_ids().len(), 2);
        buffer.clear();
        assert!(buffer.snapshot_with_ids().is_empty());

        // Ids keep advancing after a clear so keys are never reused.
        buffer.push(TerminalEntry::new(TerminalLevel::Info, "Message 3"));
        assert_eq!(buffer.snapshot_with_ids()[0].0, 2);
    }

    #[test]
    fn test_terminal_buffer_max_entries() {
        let buffer = TerminalBuffer::new();

        // Push more than MAX_TERMINAL_ENTRIES
        for i in 0..(MAX_TERMINAL_ENTRIES + 100) {
            buffer.push(TerminalEntry::new(
                TerminalLevel::Debug,
                format!("Message {}", i),
            ));
        }

        let snapshot = buffer.snapshot_with_ids();
        assert_eq!(snapshot.len(), MAX_TERMINAL_ENTRIES);

        // Verify oldest entries were trimmed
        assert!(snapshot[0].1.message.contains("100")); // First remaining is message 100
        // The first surviving entry's stable id reflects the 100 trimmed entries.
        assert_eq!(snapshot[0].0, 100);
    }

    #[test]
    fn test_terminal_filter_default() {
        let filter = TerminalFilter::default();
        assert!(filter.show_info);
        assert!(filter.show_warn);
        assert!(filter.show_error);
    }

    #[test]
    fn test_terminal_filter_toggle() {
        let mut filter = TerminalFilter::default();

        // Toggle info off
        filter.toggle(TerminalLevel::Info);
        assert!(!filter.show_info);
        assert!(filter.show_warn);
        assert!(filter.show_error);

        // Toggle info back on
        filter.toggle(TerminalLevel::Info);
        assert!(filter.show_info);
    }

    #[test]
    fn test_terminal_filter_should_show() {
        let mut filter = TerminalFilter::default();

        // All should show by default
        assert!(filter.should_show(TerminalLevel::Info));
        assert!(filter.should_show(TerminalLevel::Warn));
        assert!(filter.should_show(TerminalLevel::Error));

        // Debug and trace never show
        assert!(!filter.should_show(TerminalLevel::Debug));
        assert!(!filter.should_show(TerminalLevel::Trace));

        // Turn off warnings
        filter.show_warn = false;
        assert!(filter.should_show(TerminalLevel::Info));
        assert!(!filter.should_show(TerminalLevel::Warn));
        assert!(filter.should_show(TerminalLevel::Error));
    }

    #[test]
    fn test_format_timestamp_24h() {
        let entry = TerminalEntry {
            timestamp_secs: 50400, // 14:00:00 UTC
            level: TerminalLevel::Info,
            message: "test".to_string(),
            target: None,
        };

        // UTC (offset 0), 24-hour format
        assert_eq!(entry.format_timestamp(0, true), "14:00:00");

        // UTC-5 (Eastern), 24-hour format -> 09:00:00
        assert_eq!(entry.format_timestamp(-5, true), "09:00:00");

        // UTC+8 (China), 24-hour format -> 22:00:00
        assert_eq!(entry.format_timestamp(8, true), "22:00:00");
    }

    #[test]
    fn test_format_timestamp_12h() {
        let entry = TerminalEntry {
            timestamp_secs: 50400, // 14:00:00 UTC
            level: TerminalLevel::Info,
            message: "test".to_string(),
            target: None,
        };

        // UTC, 12-hour format -> 02:00:00 PM
        assert_eq!(entry.format_timestamp(0, false), "02:00:00 PM");

        // Test midnight
        let midnight_entry = TerminalEntry {
            timestamp_secs: 0,
            level: TerminalLevel::Info,
            message: "test".to_string(),
            target: None,
        };
        assert_eq!(midnight_entry.format_timestamp(0, false), "12:00:00 AM");

        // Test noon
        let noon_entry = TerminalEntry {
            timestamp_secs: 43200, // 12:00:00 UTC
            level: TerminalLevel::Info,
            message: "test".to_string(),
            target: None,
        };
        assert_eq!(noon_entry.format_timestamp(0, false), "12:00:00 PM");
    }
}
