//! JavaScript interop utilities for DOM manipulation
//!
//! Consolidates all document::eval calls into reusable async functions,
//! reducing code duplication across components.

// map_or is less readable for position calculation
#![allow(clippy::map_unwrap_or)]

use crate::state::PaneId;
use dioxus::prelude::document;
use tracing::debug;

/// DOM rectangle in viewport coordinates.
#[derive(Clone, Copy, Debug)]
pub struct DomRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Get an element's bounding rectangle.
pub async fn get_element_rect(element_id: &str) -> Option<DomRect> {
    let element_id = serde_json::to_string(element_id).ok()?;
    let js = format!(
        r#"
        return (function() {{
            var el = document.getElementById({element_id});
            if (!el) return null;
            var rect = el.getBoundingClientRect();
            return {{
                x: Math.round(rect.left),
                y: Math.round(rect.top),
                width: Math.max(1, Math.round(rect.width)),
                height: Math.max(1, Math.round(rect.height))
            }};
        }})();
        "#
    );

    let result = document::eval(&js).await.ok()?;
    Some(DomRect {
        x: value_to_i32(result.get("x")?)?,
        y: value_to_i32(result.get("y")?)?,
        width: value_to_u32(result.get("width")?)?,
        height: value_to_u32(result.get("height")?)?,
    })
}

fn value_to_i32(value: &serde_json::Value) -> Option<i32> {
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .or_else(|| value.as_f64().map(|n| n as i32))
}

fn value_to_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| value.as_i64().and_then(|n| u32::try_from(n).ok()))
        .or_else(|| value.as_f64().map(|n| n.max(1.0) as u32))
}

/// Get the current cursor position from a textarea element
pub async fn get_cursor_position(editor_id: &str) -> Option<usize> {
    let js = format!(
        "return document.getElementById('{}')?.selectionStart ?? 0",
        editor_id
    );

    match document::eval(&js).await {
        Ok(result) => result
            .as_i64()
            .map(|n| n as usize)
            .or_else(|| result.as_u64().map(|n| n as usize))
            .or_else(|| result.as_f64().map(|n| n as usize)),
        Err(_) => None,
    }
}

/// Set editor content and restore cursor position
pub async fn set_editor_content(pane_id: PaneId, content: &str, cursor_pos: usize) {
    let content_json = serde_json::to_string(content).unwrap_or_default();
    let js = format!(
        r#"
        (function() {{
            var editor = document.getElementById('editor-{}');
            if (editor) {{
                editor.value = {};
                editor.focus();
                var newPos = Math.min({}, editor.value.length);
                editor.selectionStart = editor.selectionEnd = newPos;
                editor.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }}
        }})();
        "#,
        pane_id, content_json, cursor_pos
    );

    if let Err(e) = document::eval(&js).await {
        debug!("JS eval for set_editor_content failed: {e:?}");
    }
}

/// Insert indentation or indent selected lines
pub async fn insert_indent(pane_id: PaneId) {
    let js = format!(
        r#"
        (function() {{
            var editor = document.getElementById('editor-{}');
            if (!editor) return;
            var start = editor.selectionStart;
            var end = editor.selectionEnd;
            var value = editor.value;
            var indent = '    ';

            if (start === end) {{
                // No selection - insert 4 spaces at cursor
                editor.value = value.substring(0, start) + indent + value.substring(end);
                editor.selectionStart = editor.selectionEnd = start + indent.length;
            }} else {{
                // Selection exists - indent all selected lines
                var lineStart = value.lastIndexOf('\n', start - 1) + 1;
                var lineEnd = value.indexOf('\n', end);
                if (lineEnd === -1) lineEnd = value.length;

                var before = value.substring(0, lineStart);
                var selected = value.substring(lineStart, lineEnd);
                var after = value.substring(lineEnd);

                var indented = selected.split('\n').map(function(line) {{
                    return indent + line;
                }}).join('\n');

                editor.value = before + indented + after;
                var addedChars = (selected.split('\n').length) * indent.length;
                editor.selectionStart = start + indent.length;
                editor.selectionEnd = end + addedChars;
            }}
            editor.focus();
            editor.dispatchEvent(new Event('input', {{ bubbles: true }}));
        }})();
        "#,
        pane_id
    );

    if let Err(e) = document::eval(&js).await {
        debug!("JS eval for insert_indent failed: {e:?}");
    }
}

/// Execute a browser document command against the focused editor/input.
pub async fn exec_document_command(command: &str) {
    let command = serde_json::to_string(command).unwrap_or_default();
    let js = format!(
        r#"
        (function() {{
            var active = document.activeElement;
            if (active && (active.tagName === 'TEXTAREA' || active.tagName === 'INPUT')) {{
                active.focus();
            }}
            document.execCommand({command});
            if (active) {{
                active.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }}
        }})();
        "#
    );

    if let Err(e) = document::eval(&js).await {
        debug!("JS eval for document command failed: {e:?}");
    }
}

/// Convert character position to (line, col) coordinates (1-indexed)
pub fn position_to_line_col(text: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(text.len());
    let text_before = &text[..pos];
    let line = text_before.matches('\n').count() + 1;
    let col = text_before
        .rfind('\n')
        .map(|last_newline| pos - last_newline)
        .unwrap_or(pos + 1);
    (line, col)
}
