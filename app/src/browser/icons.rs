//! Inline SVG icons for the file explorer tree.
//!
//! VS Code Material/Seti-inspired glyphs, deliberately simplified to read
//! cleanly at 16px. Each file icon carries its own hue via an inline
//! `color:` set to a theme token (never a raw hex), and its shapes paint with
//! `currentColor`, so all colours stay driven by the active theme.

use super::tree::TreeNode;
use dioxus::prelude::*;

/// Expand/collapse chevron for folder rows. Inherits `currentColor` from the
/// `.tree-arrow` span (muted), and is rotated to point down by the
/// `.tree-arrow.expanded` CSS rule.
pub fn chevron() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.6",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M6 4 L10 8 L6 12" }
        }
    }
}

/// Colored icon for a tree node: a folder glyph for directories, otherwise a
/// per-extension file glyph (falling back to a neutral generic file).
pub fn file_icon(node: &TreeNode) -> Element {
    if node.is_dir {
        return folder_icon();
    }
    let ext = node
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rhai" => rhai_icon(),
        "rs" => rust_icon(),
        "toml" => toml_icon(),
        "json" => json_icon(),
        "md" => markdown_icon(),
        "glb" | "gltf" | "obj" => model_icon(),
        "png" | "jpg" | "jpeg" => image_icon(),
        "svg" => vector_icon(),
        "css" => css_icon(),
        "lock" => lock_icon(),
        "txt" => text_icon(),
        _ => generic_icon(),
    }
}

fn folder_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "currentColor",
            style: "color: var(--accent);",
            path { d: "M1.5 4 H5 L6.2 5.2 H14.5 V12 H1.5 Z" }
        }
    }
}

fn rhai_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.5",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--accent);",
            path { d: "M6 5 L3 8 L6 11" }
            path { d: "M10 5 L13 8 L10 11" }
        }
    }
}

fn rust_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--syntax-string);",
            circle { cx: "8", cy: "8", r: "2.4" }
            path { d: "M8 1.6 V3.3 M8 12.7 V14.4 M1.6 8 H3.3 M12.7 8 H14.4 M3.8 3.8 l1.2 1.2 M11 11 l1.2 1.2 M12.2 3.8 l-1.2 1.2 M5 11 l-1.2 1.2" }
        }
    }
}

fn toml_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.4",
            stroke_linecap: "round",
            style: "color: var(--syntax-string);",
            path { d: "M3 6 H13" }
            circle { cx: "9", cy: "6", r: "1.7", fill: "currentColor", stroke: "none" }
            path { d: "M3 10 H13" }
            circle { cx: "6", cy: "10", r: "1.7", fill: "currentColor", stroke: "none" }
        }
    }
}

fn json_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--warning);",
            path { d: "M6.5 3 c-1.2 0 -1.5 0.6 -1.5 1.8 v1.2 c0 0.9 -0.4 1.5 -1.3 1.5 c0.9 0 1.3 0.6 1.3 1.5 v1.2 c0 1.2 0.3 1.8 1.5 1.8" }
            path { d: "M9.5 3 c1.2 0 1.5 0.6 1.5 1.8 v1.2 c0 0.9 0.4 1.5 1.3 1.5 c-0.9 0 -1.3 0.6 -1.3 1.5 v1.2 c0 1.2 -0.3 1.8 -1.5 1.8" }
        }
    }
}

fn markdown_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--accent);",
            rect { x: "1.5", y: "3.5", width: "13", height: "9", rx: "1.5" }
            path { d: "M4 10 V7 l1.6 2 1.6 -2 v3" }
            path { d: "M9.8 6.8 V9.4 M8.5 8.2 L9.8 9.6 L11.1 8.2" }
        }
    }
}

fn model_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--success);",
            path { d: "M8 2 l6 3 v6 l-6 3 -6 -3 V5 Z" }
            path { d: "M2 5 l6 3 6 -3" }
            path { d: "M8 8 V14" }
        }
    }
}

fn image_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--syntax-comment);",
            rect { x: "2", y: "3", width: "12", height: "10", rx: "1.5" }
            circle { cx: "5.5", cy: "6.3", r: "1.2", fill: "currentColor", stroke: "none" }
            path { d: "M2.5 12 L6 8.5 l2 2 2.5 -3 3 3.5" }
        }
    }
}

fn vector_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            style: "color: var(--syntax-keyword);",
            path { d: "M5 11 L11 5" }
            rect { x: "2.5", y: "10", width: "3", height: "3", rx: "0.5" }
            rect { x: "10.5", y: "2.5", width: "3", height: "3", rx: "0.5" }
        }
    }
}

fn css_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.4",
            stroke_linecap: "round",
            style: "color: var(--accent);",
            path { d: "M6.5 2.5 L5 13.5 M11 2.5 L9.5 13.5 M3.7 6 H13 M3 10 H12.3" }
        }
    }
}

fn lock_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            style: "color: var(--text-muted);",
            rect { x: "3.5", y: "7", width: "9", height: "6.5", rx: "1.2", fill: "currentColor" }
            path {
                d: "M5.5 7 V5.3 a2.5 2.5 0 0 1 5 0 V7",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.3",
                stroke_linecap: "round",
            }
        }
    }
}

fn text_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.1",
            stroke_linejoin: "round",
            style: "color: var(--text-secondary);",
            path { d: "M4 2.5 H9 L12 5.5 V13.5 H4 Z" }
            path { d: "M9 2.5 V5.5 H12" }
            path { d: "M6 8.5 H10 M6 10.5 H10 M6 12.5 H8.5", stroke_linecap: "round" }
        }
    }
}

fn generic_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.1",
            stroke_linejoin: "round",
            style: "color: var(--text-muted);",
            path { d: "M4 2.5 H9 L12 5.5 V13.5 H4 Z" }
            path { d: "M9 2.5 V5.5 H12" }
        }
    }
}

/// Header action icon: new file (page with a plus).
pub fn new_file_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M3.5 2 H9 L12.5 5.5 V14 H3.5 Z" }
            path { d: "M9 2 V5.5 H12.5" }
            path { d: "M8 8 V12 M6 10 H10" }
        }
    }
}

/// Header action icon: new folder (folder outline with a plus).
pub fn new_folder_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M1.5 4 H5 L6.2 5.2 H14.5 V12 H1.5 Z" }
            path { d: "M8 7.2 V10.8 M6.2 9 H9.8" }
        }
    }
}

/// Header action icon: refresh (circular arrow).
pub fn refresh_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12.5 8 A4.5 4.5 0 1 1 11.18 4.82" }
            path { d: "M11.2 2.3 V5 H8.5" }
        }
    }
}

/// Header action icon: collapse all (two chevrons pointing inward).
pub fn collapse_all_icon() -> Element {
    rsx! {
        svg {
            width: "16",
            height: "16",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M4.5 3.5 L8 6.5 L11.5 3.5" }
            path { d: "M4.5 12.5 L8 9.5 L11.5 12.5" }
        }
    }
}
