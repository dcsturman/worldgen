//! System Map component — pictorial render of a generated system.
//!
//! Auto-renders whenever the `Store<System>` in context changes:
//! `render_png` is fast enough (a few ms) that there's no value in
//! gating it behind a button. The rendered PNG is wrapped in a Blob,
//! turned into an object URL, and pointed at by an `<img>` element.
//!
//! Previous Blob URLs are revoked on each re-render so they don't
//! accumulate in the document.
//!
//! Surfaces three on-screen affordances next to the map:
//!   * **Download PNG** — saves the current render to the user's
//!     downloads folder for import into a VTT.
//!   * **Print system** — opens a modal asking whether to include
//!     the map in the printout, then calls `window.print()`.
//!   * **Include in print** checkbox state lives in a signal — when
//!     `false`, the map image gets `d-print-none` so the browser's
//!     print rendering hides it.

use leptos::prelude::*;
use reactive_stores::Store;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

use crate::sysmap::render_png;
use crate::systems::system::System;

#[component]
pub fn SystemMap() -> impl IntoView {
    let primary = expect_context::<Store<System>>();
    let (image_url, set_image_url) = signal::<Option<String>>(None);
    let (error, set_error) = signal::<Option<String>>(None);
    let (raw_png, set_raw_png) = signal::<Option<Vec<u8>>>(None);
    let include_in_print = RwSignal::new(false);

    // Re-render whenever the system store changes. The `read()` guard
    // subscribes the effect to the entire store, so any regeneration
    // (do_generate calling `system.set(...)`) triggers a redraw.
    Effect::new(move |_| {
        let sys = primary.read();
        let result = render_png(&sys);
        drop(sys);
        match result {
            Ok(bytes) => {
                match bytes_to_object_url(&bytes) {
                    Ok(url) => {
                        if let Some(old) = image_url.get_untracked() {
                            let _ = Url::revoke_object_url(&old);
                        }
                        set_image_url.set(Some(url));
                        set_error.set(None);
                    }
                    Err(e) => set_error.set(Some(format!("Blob URL failed: {e:?}"))),
                }
                set_raw_png.set(Some(bytes));
            }
            Err(e) => set_error.set(Some(e)),
        }
    });

    let dialog_ref = NodeRef::<leptos::html::Dialog>::new();

    let open_print_dialog = move |_| {
        if let Some(dialog) = dialog_ref.get() {
            let _ = dialog.show_modal();
        }
    };

    // The `<form method="dialog">` submit buttons set the dialog's
    // `returnValue` to their `value` attribute and close the dialog
    // automatically. We read that here and act accordingly.
    let on_dialog_close = move |_| {
        let Some(dialog) = dialog_ref.get() else {
            return;
        };
        let choice = dialog.return_value();
        match choice.as_str() {
            "with-map" => {
                include_in_print.set(true);
                request_print();
            }
            "without-map" => {
                include_in_print.set(false);
                request_print();
            }
            _ => {}
        }
    };

    let on_download = move |_| {
        if let Some(bytes) = raw_png.get_untracked()
            && let Err(e) = trigger_png_download(&bytes, "system-map.png")
        {
            set_error.set(Some(format!("PNG download failed: {e:?}")));
        }
    };

    view! {
        <div class="system-map" style="margin-top: 1.5rem;">
            <div class="d-print-none" style="display: flex; gap: 0.5rem; margin-bottom: 0.5rem;">
                <button class="blue-button" on:click=open_print_dialog>
                    "Print system"
                </button>
                <button class="blue-button" on:click=on_download>
                    "Download PNG"
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <div class="error d-print-none" style="color: #c44; margin-top: 0.5rem;">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || image_url.get().is_some()>
                <div class:d-print-none=move || !include_in_print.get()>
                    <img
                        style="max-width: 100%; height: auto; border: 1px solid #333;"
                        src=move || image_url.get().unwrap_or_default()
                        alt="System map"
                    />
                </div>
            </Show>
            <dialog
                node_ref=dialog_ref
                on:close=on_dialog_close
                class="print-options-dialog"
                style="padding: 1.5rem; border-radius: 0.5rem; max-width: 28rem;"
            >
                <form method="dialog">
                    <p style="margin-top: 0;">
                        "Include the system map in the printed output?"
                    </p>
                    <div style="display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem;">
                        <button value="cancel">"Cancel"</button>
                        <button value="without-map">"Print without map"</button>
                        <button value="with-map" autofocus>"Print with map"</button>
                    </div>
                </form>
            </dialog>
        </div>
    }
}

/// Defer `window.print()` by one animation frame so the CSS class
/// change from the dialog choice has time to apply before the
/// browser captures the print state.
fn request_print() {
    use wasm_bindgen::closure::Closure;
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb = Closure::once_into_js(move || {
        if let Some(w) = web_sys::window() {
            let _ = w.print();
        }
    });
    let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
}

fn bytes_to_object_url(bytes: &[u8]) -> Result<String, JsValue> {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());
    let opts = BlobPropertyBag::new();
    opts.set_type("image/png");
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)?;
    Url::create_object_url_with_blob(&blob)
}

/// Trigger a PNG download of `bytes` with the given filename. Creates
/// a temporary `<a download>` anchor, clicks it, then cleans it up
/// along with the object URL.
fn trigger_png_download(bytes: &[u8], filename: &str) -> Result<(), JsValue> {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());
    let opts = BlobPropertyBag::new();
    opts.set_type("image/png");
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)?;
    let url = Url::create_object_url_with_blob(&blob)?;

    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let a: HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(filename);
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("no body"))?;
    body.append_child(&a)?;
    a.click();
    body.remove_child(&a)?;
    Url::revoke_object_url(&url)?;
    Ok(())
}
