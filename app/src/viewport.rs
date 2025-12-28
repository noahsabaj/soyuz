//! Reference panel - Quick API reference and examples

use dioxus::prelude::*;

/// Reference panel with API quick reference
#[component]
pub fn ViewportPanel() -> Element {
    let expanded_section: Signal<Option<&'static str>> = use_signal(|| None);

    rsx! {
        div { class: "reference-container",
            div { class: "viewport-header",
                span { class: "viewport-title", "Reference" }
            }

            div { class: "reference-content",
                // Primitives section
                ReferenceSection {
                    title: "Primitives",
                    expanded: expanded_section,
                    items: vec![
                        ("sphere(r)", "Sphere with radius r"),
                        ("cube(size)", "Cube with side length"),
                        ("box3(x, y, z)", "Box with dimensions"),
                        ("cylinder(r, h)", "Cylinder: radius, height"),
                        ("capsule(r, h)", "Rounded cylinder"),
                        ("torus(R, r)", "Donut: major, minor radius"),
                        ("cone(r, h)", "Cone: radius, height"),
                        ("ellipsoid(x, y, z)", "Stretched sphere"),
                    ]
                }

                // Operations section
                ReferenceSection {
                    title: "Combine",
                    expanded: expanded_section,
                    items: vec![
                        (".union(b)", "Add shapes together"),
                        (".subtract(b)", "Cut b from shape"),
                        (".intersect(b)", "Keep overlap only"),
                        (".smooth_union(b, k)", "Blend shapes (k=smoothness)"),
                        (".smooth_subtract(b, k)", "Smooth cut"),
                    ]
                }

                // Transforms section
                ReferenceSection {
                    title: "Transform",
                    expanded: expanded_section,
                    items: vec![
                        (".translate(x, y, z)", "Move shape"),
                        (".translate_x(x)", "Move along X"),
                        (".rotate_x(angle)", "Rotate around X (radians)"),
                        (".scale(factor)", "Scale uniformly"),
                        (".mirror_x()", "Mirror across YZ plane"),
                        (".symmetry_x()", "Make symmetric"),
                    ]
                }

                // Modifiers section
                ReferenceSection {
                    title: "Modify",
                    expanded: expanded_section,
                    items: vec![
                        (".hollow(t)", "Make hollow with thickness t"),
                        (".round(r)", "Round edges by r"),
                        (".onion(t)", "Layered shell effect"),
                        (".elongate(x, y, z)", "Stretch shape"),
                        (".twist(amount)", "Twist around Y axis"),
                        (".bend(amount)", "Bend shape"),
                    ]
                }

                // Repeat section
                ReferenceSection {
                    title: "Repeat",
                    expanded: expanded_section,
                    items: vec![
                        (".repeat(x, y, z)", "Infinite grid repeat"),
                        (".repeat_limited(...)", "Limited repeat with count"),
                        (".repeat_polar(n)", "Repeat n times around Y"),
                    ]
                }

                // Quick example
                div { class: "reference-example",
                    div { class: "example-title", "Quick Example" }
                    pre { class: "example-code",
                        {r#"let base = cylinder(0.5, 0.1);
let stem = cylinder(0.1, 0.8)
    .translate_y(0.4);

base.smooth_union(stem, 0.1)"#}
                    }
                }

                // Tips
                div { class: "reference-tips",
                    div { class: "tip-title", "Tips" }
                    ul {
                        li { "Chain operations: shape.translate(1,0,0).rotate_y(0.5)" }
                        li { "Use deg(45) to convert degrees to radians" }
                        li { "Ctrl+Enter to preview" }
                    }
                }
            }
        }
    }
}

#[component]
fn ReferenceSection(
    title: &'static str,
    expanded: Signal<Option<&'static str>>,
    items: Vec<(&'static str, &'static str)>,
) -> Element {
    let is_expanded = *expanded.read() == Some(title);

    rsx! {
        div { class: "reference-section",
            div {
                class: "reference-section-header",
                onclick: move |_| {
                    if is_expanded {
                        expanded.set(None);
                    } else {
                        expanded.set(Some(title));
                    }
                },
                span { class: "section-title", "{title}" }
                span { class: "section-toggle", if is_expanded { "−" } else { "+" } }
            }
            if is_expanded {
                div { class: "reference-section-content",
                    for (syntax, desc) in items {
                        div { class: "reference-item",
                            code { class: "ref-syntax", "{syntax}" }
                            span { class: "ref-desc", "{desc}" }
                        }
                    }
                }
            }
        }
    }
}
