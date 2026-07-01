//! Shared, reusable UI components (small in-house design system).
//!
//! These exist to remove duplication that had accreted across panels — most
//! importantly the three near-identical confirmation dialogs (close-tab, delete,
//! overwrite), now unified on the `dioxus-primitives` `AlertDialog` via
//! [`ConfirmDialog`].

use dioxus::prelude::*;

/// A modal confirmation dialog with Cancel / Confirm buttons.
///
/// Built on `dioxus_primitives::alert_dialog` for an accessible modal (role,
/// focus trap, Escape-to-dismiss, `aria-labelledby`/`describedby`). The parent
/// mounts this only while the prompt should be visible, so the underlying alert
/// dialog is always open; Escape and the buttons route to `on_cancel`/`on_confirm`
/// which change the parent state and unmount us. Set `destructive` for a red
/// confirm button (delete / close-without-saving); leave it false for neutral
/// confirmations (overwrite).
///
/// Note: like a Radix AlertDialog, a backdrop click does *not* dismiss — the user
/// must pick Cancel or Escape — which is intentional for destructive prompts.
#[component]
pub fn ConfirmDialog(
    /// Bold heading line.
    #[props(into)]
    title: String,
    /// Body text.
    #[props(into)]
    message: String,
    /// Label for the confirm button (defaults to "Confirm").
    #[props(into, default = String::from("Confirm"))]
    confirm_label: String,
    /// Render the confirm button in the destructive (red) style.
    #[props(default = false)]
    destructive: bool,
    /// Called when the user confirms.
    on_confirm: EventHandler<()>,
    /// Called when the user cancels (Cancel button or Escape).
    on_cancel: EventHandler<()>,
) -> Element {
    use dioxus_primitives::alert_dialog::{
        AlertDialogActions, AlertDialogContent, AlertDialogDescription, AlertDialogRoot,
        AlertDialogTitle,
    };

    let confirm_class = if destructive {
        "confirm-dialog-btn confirm-dialog-btn-confirm destructive"
    } else {
        "confirm-dialog-btn confirm-dialog-btn-confirm"
    };

    rsx! {
        AlertDialogRoot {
            class: "confirm-dialog-backdrop",
            open: true,
            on_open_change: move |open: bool| {
                if !open {
                    on_cancel.call(());
                }
            },
            AlertDialogContent {
                class: "confirm-dialog".to_string(),
                AlertDialogTitle { class: "confirm-dialog-title", "{title}" }
                AlertDialogDescription { class: "confirm-dialog-description", "{message}" }
                AlertDialogActions { class: "confirm-dialog-actions",
                    // Plain buttons rather than AlertDialog{Cancel,Action}: those call
                    // `set_open(false)` *before* the user handler, which — combined with
                    // `on_open_change` → `on_cancel` — would fire cancel *then* confirm.
                    button {
                        r#type: "button",
                        class: "confirm-dialog-btn confirm-dialog-btn-cancel",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "{confirm_class}",
                        onclick: move |_| on_confirm.call(()),
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}

/// A single icon button in the left activity bar.
///
/// Extracts the hand-duplicated `<button class="activity-item">` blocks that the
/// activity bar previously pasted five times: `active` toggles the selected
/// styling and the icon SVG is supplied as children.
#[component]
pub fn ActivityButton(
    /// Highlight the button as the active/selected item.
    active: bool,
    /// Accessible label (`aria-label`).
    #[props(into)]
    label: String,
    /// Tooltip text.
    #[props(into)]
    title: String,
    /// Invoked when the button is clicked.
    on_click: EventHandler<()>,
    /// Icon markup, typically an inline `svg`.
    children: Element,
) -> Element {
    use dioxus_primitives::ContentSide;
    use dioxus_primitives::tooltip::{Tooltip, TooltipContent, TooltipTrigger};

    let class = if active {
        "activity-item active"
    } else {
        "activity-item"
    };

    rsx! {
        Tooltip { class: "activity-tooltip",
            // Render the real <button> AS the trigger, so there is a single focusable
            // element (the button shows the tooltip on hover/focus) rather than a
            // focusable wrapper div nested around a focusable button.
            TooltipTrigger {
                r#as: move |attrs: Vec<Attribute>| rsx! {
                    button {
                        r#type: "button",
                        class: "{class}",
                        aria_label: "{label}",
                        onclick: move |_| on_click.call(()),
                        ..attrs,
                        {children.clone()}
                    }
                },
            }
            TooltipContent {
                class: "activity-tooltip-content",
                side: ContentSide::Right,
                "{title}"
            }
        }
    }
}
