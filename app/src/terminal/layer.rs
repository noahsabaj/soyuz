//! Custom tracing subscriber layer for capturing logs into the terminal buffer
//!
//! This layer intercepts tracing events and forwards them to the in-app
//! terminal panel. It filters out debug/trace levels and Dioxus internals.

use crate::state::{TerminalBuffer, TerminalEntry, TerminalLevel};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Prefixes of targets to ignore (Dioxus internals, etc.)
const IGNORED_TARGET_PREFIXES: &[&str] = &[
    "dioxus_core",
    "dioxus_desktop",
    "dioxus_html",
    "dioxus_signals",
    "dioxus_hooks",
    "tao::",
    "wry::",
    "mio::",
    "tokio",
    "hyper",
    "tracing",
];

/// A tracing layer that captures events into a TerminalBuffer
pub struct TerminalLayer {
    buffer: TerminalBuffer,
}

impl TerminalLayer {
    /// Create a new terminal layer with the given buffer
    pub fn new(buffer: TerminalBuffer) -> Self {
        Self { buffer }
    }

    /// Check if a target should be ignored
    fn should_ignore_target(target: &str) -> bool {
        IGNORED_TARGET_PREFIXES
            .iter()
            .any(|prefix| target.starts_with(prefix))
    }
}

impl<S: Subscriber> Layer<S> for TerminalLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        let target = event.metadata().target();

        // Only capture INFO, WARN, ERROR (skip DEBUG and TRACE)
        if level > tracing::Level::INFO {
            return;
        }

        // Skip Dioxus/framework internals
        if Self::should_ignore_target(target) {
            return;
        }

        // Extract message from event
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let terminal_level = TerminalLevel::from(level);
        let entry = TerminalEntry::with_target(terminal_level, visitor.message, target.to_string());
        self.buffer.push(entry);
    }
}

/// Visitor to extract the message from a tracing event
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
            // Remove surrounding quotes from debug output
            if self.message.starts_with('"') && self.message.ends_with('"') {
                self.message = self.message[1..self.message.len() - 1].to_string();
            }
        } else if self.message.is_empty() {
            // Fallback: use any field as message
            self.message = format!("{}: {:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if self.message.is_empty() {
            self.message = format!("{}: {}", field.name(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_layer_creation() {
        let buffer = TerminalBuffer::new();
        let _layer = TerminalLayer::new(buffer);
    }
}
