//! System Map component — pictorial render of a generated system.
//!
//! Auto-renders whenever the `Store<System>` in context changes:
//! `render_png` is fast enough (a few ms) that there's no value in
//! gating it behind a button. The rendered PNG is wrapped in a Blob,
//! turned into an object URL, and pointed at by an `<img>` element.
//!
//! Previous Blob URLs are revoked on each re-render so they don't
//! accumulate in the document.

use leptos::prelude::*;
use reactive_stores::Store;
use wasm_bindgen::JsValue;
use web_sys::{Blob, BlobPropertyBag, Url};

use crate::sysmap::render_png;
use crate::systems::system::System;

#[component]
pub fn SystemMap() -> impl IntoView {
    let primary = expect_context::<Store<System>>();
    let (image_url, set_image_url) = signal::<Option<String>>(None);
    let (error, set_error) = signal::<Option<String>>(None);

    // Re-render whenever the system store changes. The `read()` guard
    // subscribes the effect to the entire store, so any regeneration
    // (do_generate calling `system.set(...)`) triggers a redraw.
    Effect::new(move |_| {
        let sys = primary.read();
        let result = render_png(&sys);
        // Drop the guard before we touch other signals.
        drop(sys);
        match result {
            Ok(bytes) => match bytes_to_object_url(&bytes) {
                Ok(url) => {
                    if let Some(old) = image_url.get_untracked() {
                        let _ = Url::revoke_object_url(&old);
                    }
                    set_image_url.set(Some(url));
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(format!("Blob URL failed: {e:?}"))),
            },
            Err(e) => set_error.set(Some(e)),
        }
    });

    view! {
        <div class="system-map d-print-none" style="margin-top: 1.5rem;">
            <Show when=move || error.get().is_some()>
                <div class="error" style="color: #c44; margin-top: 0.5rem;">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || image_url.get().is_some()>
                <div>
                    <img
                        style="max-width: 100%; height: auto; border: 1px solid #333;"
                        src=move || image_url.get().unwrap_or_default()
                        alt="System map"
                    />
                </div>
            </Show>
        </div>
    }
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
