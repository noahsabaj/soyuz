//! Markdown documentation panel.
//!
//! Renders the generated Soyuz documentation set embedded at compile time.
//! The Help menu owns three markdown tabs: Documentation, Cookbook, and README.
//! Sidebar navigation changes the selected page inside those tabs.

use crate::docs_generated::{DOCS, EmbeddedDoc, doc_by_id};
use crate::state::{AppStore, MarkdownDoc};
use dioxus::prelude::*;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Static cache for parsed HTML content.
static DOC_HTML: OnceLock<HashMap<&'static str, String>> = OnceLock::new();

/// Get parsed HTML content for a document type (cached).
fn get_html_content(doc: &'static EmbeddedDoc) -> &'static str {
    let html_by_id = DOC_HTML.get_or_init(|| {
        DOCS.iter()
            .map(|doc| (doc.id, markdown_to_html(doc.content)))
            .collect()
    });

    html_by_id.get(doc.id).map_or(
        "<h1>Documentation</h1><p>Document not found.</p>",
        String::as_str,
    )
}

/// Convert heading text to a URL-friendly slug.
fn slugify(text: &str) -> String {
    let mut result = String::new();

    for c in text.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if (c.is_whitespace() || c == '-' || c == '_') && !result.is_empty() {
            result.push('-');
        }
    }

    while result.ends_with('-') {
        result.pop();
    }

    result
}

/// Convert markdown to HTML with auto-generated heading IDs.
fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all());
    let events: Vec<Event> = parser.collect();

    let mut heading_ids: HashMap<usize, String> = HashMap::new();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    let mut i = 0;

    while i < events.len() {
        if let Event::Start(Tag::Heading { .. }) = &events[i] {
            let start_idx = i;
            let mut heading_text = String::new();

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

            let base_slug = slugify(&heading_text);
            let count = slug_counts.entry(base_slug.clone()).or_insert(0);
            let slug = if *count == 0 {
                base_slug.clone()
            } else {
                format!("{base_slug}-{count}")
            };
            if let Some(c) = slug_counts.get_mut(&base_slug) {
                *c += 1;
            }

            heading_ids.insert(start_idx, slug);
        }
        i += 1;
    }

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

/// Markdown panel component - displays formatted markdown documentation.
#[component]
pub fn MarkdownPanel(doc: MarkdownDoc) -> Element {
    let mut state = use_context::<AppStore>();
    let selected_id = doc.selected_id;
    let selected_doc = doc_by_id(selected_id)
        .or_else(|| doc_by_id(MarkdownDoc::DOCS_INDEX.selected_id))
        .unwrap_or(&DOCS[0]);
    let html_content = get_html_content(selected_doc);
    let show_sidebar = doc.container != crate::state::MarkdownContainer::Readme;
    let panel_class = if show_sidebar {
        "markdown-panel docs-browser"
    } else {
        "markdown-panel docs-single-page"
    };

    rsx! {
        div { class: panel_class,
            if show_sidebar {
                aside { class: "docs-sidebar",
                    for nav_doc in DOCS.iter().filter(move |nav_doc| doc.can_select(nav_doc.id)) {
                        {
                            let is_active = selected_id == nav_doc.id;
                            let nav_doc_id = nav_doc.id;
                            rsx! {
                                button {
                                    key: "{nav_doc.id}",
                                    class: if is_active { "docs-nav-item active" } else { "docs-nav-item" },
                                    title: "{nav_doc.href}",
                                    onclick: move |_| {
                                        state.write().select_active_markdown_doc(nav_doc_id);
                                    },
                                    span { class: "docs-nav-section", "{nav_doc.section}" }
                                    span { class: "docs-nav-title", "{nav_doc.title}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "docs-content-scroll",
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
                div { class: "content body",
                    dangerous_inner_html: "{html_content}"
                }
            }
        }
    }
}
