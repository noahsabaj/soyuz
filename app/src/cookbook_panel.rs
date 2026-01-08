//! Cookbook panel - displays SOYUZ_COOKBOOK.md as formatted HTML
//!
//! Renders the embedded cookbook documentation with proper markdown styling.
//! The content is parsed at runtime using pulldown-cmark but memoized to avoid
//! re-parsing on each render. Headings are automatically assigned IDs based on
//! their text content for anchor link navigation.

use dioxus::prelude::*;
use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

/// Embedded cookbook markdown (compile-time)
const COOKBOOK_MD: &str = include_str!("../../SOYUZ_COOKBOOK.md");

/// Convert heading text to a URL-friendly slug
/// "Quick Start" -> "quick-start"
/// "Environment & Lighting" -> "environment--lighting" (& becomes empty, spaces become -)
fn slugify(text: &str) -> String {
    let mut result = String::new();

    for c in text.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if c.is_whitespace() || c == '-' || c == '_' {
            // Only add dash if result is non-empty (avoid leading dashes)
            if !result.is_empty() {
                result.push('-');
            }
        }
        // Other characters (punctuation like &) are simply skipped,
        // which can create consecutive dashes (intentional for TOC compatibility)
    }

    // Trim trailing dashes
    while result.ends_with('-') {
        result.pop();
    }

    result
}

/// Convert markdown to HTML with auto-generated heading IDs
fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all());
    let events: Vec<Event> = parser.collect();

    // First pass: collect heading texts to generate IDs
    let mut heading_ids: HashMap<usize, String> = HashMap::new();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut i = 0;

    while i < events.len() {
        if let Event::Start(Tag::Heading { .. }) = &events[i] {
            let start_idx = i;
            let mut heading_text = String::new();

            // Collect text until we hit the end of heading
            i += 1;
            while i < events.len() {
                match &events[i] {
                    Event::Text(text) | Event::Code(text) => {
                        heading_text.push_str(text);
                    }
                    Event::End(TagEnd::Heading(_)) => break,
                    _ => {}
                }
                i += 1;
            }

            // Generate slug and handle duplicates
            let base_slug = slugify(&heading_text);
            let count = slug_counts.entry(base_slug.clone()).or_insert(0);
            let slug = if *count == 0 {
                base_slug.clone()
            } else {
                format!("{}-{}", base_slug, count)
            };
            // Increment count for this slug
            if let Some(c) = slug_counts.get_mut(&base_slug) {
                *c += 1;
            }

            heading_ids.insert(start_idx, slug);
        }
        i += 1;
    }

    // Second pass: transform events to inject IDs
    let transformed: Vec<Event> = events
        .into_iter()
        .enumerate()
        .map(|(idx, event)| {
            if let Some(id) = heading_ids.get(&idx)
                && let Event::Start(Tag::Heading {
                    level,
                    id: _,
                    classes,
                    attrs,
                }) = event
            {
                return Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::Boxed(id.clone().into_boxed_str())),
                    classes,
                    attrs,
                });
            }
            event
        })
        .collect();

    let mut html_output = String::new();
    html::push_html(&mut html_output, transformed.into_iter());
    html_output
}

/// Cookbook panel component - displays formatted markdown documentation
#[component]
pub fn CookbookPanel() -> Element {
    // Parse markdown to HTML once (no reactive dependencies)
    let html_content = use_memo(|| markdown_to_html(COOKBOOK_MD));

    rsx! {
        div { class: "cookbook-panel",
            // Intercept anchor link clicks and scroll instead of navigating
            script {
                dangerous_inner_html: "
                    (function() {{
                        var panel = document.currentScript.parentElement;
                        panel.addEventListener('click', function(e) {{
                            var link = e.target.closest('a');
                            if (link) {{
                                var href = link.getAttribute('href');
                                if (href && href.charAt(0) === String.fromCharCode(35)) {{
                                    e.preventDefault();
                                    e.stopPropagation();
                                    e.stopImmediatePropagation();
                                    var targetId = href.substring(1);
                                    var target = document.getElementById(targetId);
                                    if (target) {{
                                        target.scrollIntoView({{ behavior: 'smooth', block: 'start' }});
                                    }}
                                    return false;
                                }}
                            }}
                        }}, true);
                    }})();
                "
            }
            div { class: "cookbook-content markdown-body",
                dangerous_inner_html: "{html_content}"
            }
        }
    }
}
