//! Small `?` icon that reveals a hover tooltip after a 1-second delay.
//!
//! Pure CSS — no JS timers — so the delay is precise and there's nothing
//! to clean up on unmount. Renders nothing if `text` is empty, so callers
//! can pass tooltip strings that haven't been written yet without
//! cluttering the UI with empty bubbles.

use leptos::ev;
use leptos::prelude::*;

#[component]
pub fn HelpTooltip(
    text: &'static str,
    /// Render the bubble below the icon instead of above. Use for inputs
    /// near the top of the page where the default upward bubble would
    /// clip past the viewport edge.
    #[prop(optional)]
    below: bool,
) -> impl IntoView {
    (!text.is_empty()).then(|| {
        let bubble_class = if below {
            "help-tooltip-text help-tooltip-below"
        } else {
            "help-tooltip-text"
        };
        view! {
            <span
                class="help-tooltip"
                aria-label=text
                // Inside <label>: clicks on the icon would otherwise focus
                // the associated input. Swallow the click so hovering the
                // `?` is purely informational.
                on:click=|ev: ev::MouseEvent| {
                    ev.prevent_default();
                    ev.stop_propagation();
                }
            >
                "?"
                <span class=bubble_class>{text}</span>
            </span>
        }
    })
}
