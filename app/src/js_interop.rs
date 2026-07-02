//! JavaScript interop utilities for DOM manipulation
//!
//! Consolidates all document::eval calls into reusable async functions,
//! reducing code duplication across components.

// map_or is less readable for position calculation
#![allow(clippy::map_unwrap_or)]

use crate::state::PaneId;
use dioxus::prelude::document;
use tracing::debug;

/// DOM rectangle in viewport coordinates, in physical (device) pixels.
#[derive(Clone, Copy, Debug)]
pub struct DomRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Get an element's bounding rectangle in physical (device) pixels.
///
/// `getBoundingClientRect` reports CSS pixels; multiplying by
/// `devicePixelRatio` converts to the device coordinate space that X11 child
/// window embedding positions in. On fractional-DPI displays (e.g. Xft.dpi
/// 153 = ~160%) the two differ, and mixing them up embeds the preview at the
/// wrong place and size.
pub async fn get_element_rect(element_id: &str) -> Option<DomRect> {
    let element_id = serde_json::to_string(element_id).ok()?;
    let js = format!(
        r#"
        return (function() {{
            var el = document.getElementById({element_id});
            if (!el) return null;
            var rect = el.getBoundingClientRect();
            var dpr = window.devicePixelRatio || 1;
            return {{
                x: Math.round(rect.left * dpr),
                y: Math.round(rect.top * dpr),
                width: Math.max(1, Math.round(rect.width * dpr)),
                height: Math.max(1, Math.round(rect.height * dpr))
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
    // `cursor_pos` is a Rust UTF-8 byte offset, but JS `selectionStart`/`value.length`
    // are measured in UTF-16 code units; convert so the caret lands correctly on
    // content containing non-ASCII characters.
    let cursor_u16 = byte_to_utf16(content, cursor_pos);
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
        pane_id, content_json, cursor_u16
    );

    if let Err(e) = document::eval(&js).await {
        debug!("JS eval for set_editor_content failed: {e:?}");
    }
}

/// Move the caret to the start of a 1-indexed line and scroll it into view.
pub async fn go_to_line(pane_id: PaneId, line: usize) {
    let js = format!(
        r#"
        (function() {{
            var editor = document.getElementById('editor-{pane_id}');
            if (!editor) return;
            var lines = editor.value.split('\n');
            var target = Math.max(1, Math.min({line}, lines.length));
            var offset = 0;
            for (var i = 0; i < target - 1; i++) {{
                offset += lines[i].length + 1; // +1 for the newline
            }}
            editor.focus();
            editor.selectionStart = editor.selectionEnd = offset;
            var style = window.getComputedStyle(editor);
            var lineHeight = parseFloat(style.lineHeight);
            if (isNaN(lineHeight)) lineHeight = parseFloat(style.fontSize) * 1.2;
            editor.scrollTop = Math.max(0, (target - 1) * lineHeight - editor.clientHeight / 2);
            editor.dispatchEvent(new Event('scroll', {{ bubbles: true }}));
        }})();
        "#
    );

    if let Err(e) = document::eval(&js).await {
        debug!("JS eval for go_to_line failed: {e:?}");
    }
}

/// Insert indentation or indent selected lines, using `tab_size` spaces (F61).
pub async fn insert_indent(pane_id: PaneId, tab_size: u32) {
    // Build the indent from the configured tab size (clamped to a sane range) and
    // embed it as a JSON string literal so it is safely escaped in the script.
    let spaces = " ".repeat(tab_size.clamp(1, 16) as usize);
    let indent_json = serde_json::to_string(&spaces).unwrap_or_else(|_| "\"    \"".to_string());
    let js = format!(
        r#"
        (function() {{
            var editor = document.getElementById('editor-{pane_id}');
            if (!editor) return;
            var start = editor.selectionStart;
            var end = editor.selectionEnd;
            var value = editor.value;
            var indent = {indent_json};

            if (start === end) {{
                // No selection - insert one indent at the cursor
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
        "#
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

/// Convert a JS UTF-16 code-unit offset into a Rust UTF-8 byte index.
///
/// Walks characters accumulating their UTF-16 length and returns the byte index
/// at which the running UTF-16 count reaches `u16_off`. Offsets that fall inside a
/// surrogate pair or past the end clamp to the next char boundary / `text.len()`,
/// so the result is always a valid byte boundary (never panics on slicing).
fn utf16_to_byte(text: &str, u16_off: usize) -> usize {
    let mut u16_count = 0;
    for (byte_idx, ch) in text.char_indices() {
        if u16_count >= u16_off {
            return byte_idx;
        }
        u16_count += ch.len_utf16();
    }
    text.len()
}

/// Convert a Rust UTF-8 byte index into a JS UTF-16 code-unit offset.
fn byte_to_utf16(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    let mut u16_count = 0;
    for (byte_idx, ch) in text.char_indices() {
        if byte_idx >= byte {
            break;
        }
        u16_count += ch.len_utf16();
    }
    u16_count
}

/// Convert a cursor position to (line, col) coordinates (1-indexed).
///
/// `pos` is a JS `selectionStart` value measured in UTF-16 code units. It is
/// converted to a UTF-8 byte index before slicing so multi-byte characters (e.g.
/// `é`) neither panic nor miscount the column.
pub fn position_to_line_col(text: &str, pos: usize) -> (usize, usize) {
    let byte_pos = utf16_to_byte(text, pos);
    let text_before = &text[..byte_pos];
    let line = text_before.matches('\n').count() + 1;
    let line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    // Column is counted in characters from the start of the line.
    let col = text[line_start..byte_pos].chars().count() + 1;
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_byte_round_trip_handles_non_ascii() {
        let text = "aé😀b"; // 'a'=1u16/1b, 'é'=1u16/2b, '😀'=2u16/4b, 'b'=1u16/1b
        // Byte boundaries: a@0, é@1, 😀@3, b@7, end@8
        assert_eq!(utf16_to_byte(text, 0), 0);
        assert_eq!(utf16_to_byte(text, 1), 1); // after 'a'
        assert_eq!(utf16_to_byte(text, 2), 3); // after 'é'
        assert_eq!(utf16_to_byte(text, 4), 7); // after '😀' (2 u16 units)
        assert_eq!(utf16_to_byte(text, 5), 8); // after 'b'
        assert_eq!(utf16_to_byte(text, 999), text.len()); // clamps

        assert_eq!(byte_to_utf16(text, 0), 0);
        assert_eq!(byte_to_utf16(text, 1), 1);
        assert_eq!(byte_to_utf16(text, 3), 2);
        assert_eq!(byte_to_utf16(text, 7), 4);
        assert_eq!(byte_to_utf16(text, 8), 5);
    }

    #[test]
    fn position_to_line_col_does_not_panic_on_non_ascii() {
        let text = "héllo\nwörld";
        // UTF-16 offset 7 = 'w' on line 2 (h é l l o \n w -> 6 u16 units before 'w')
        let (line, col) = position_to_line_col(text, 7);
        assert_eq!(line, 2);
        assert_eq!(col, 2);
        // Out-of-range offset clamps without panicking.
        let _ = position_to_line_col(text, 9999);
    }
}
