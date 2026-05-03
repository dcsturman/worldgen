//! Leptos component: World Map generator.
//!
//! Takes a UWP + seed, calls into `worldgen::worldmap::generate`, and shows
//! the SVG (which already includes the legend strip below the map). The
//! "Key" checkbox toggles whether the legend strip portion of the SVG is
//! shown in the live UI; the bundled legend is always part of the
//! downloaded PNG so saved maps carry their key with them.

use leptos::prelude::*;
use leptos_use::signal_debounced;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

use crate::worldmap;

const DEFAULT_UWP: &str = "A788899-A";
const DEFAULT_SEED: u64 = 0xC0FFEE;

#[component]
pub fn WorldMap() -> impl IntoView {
    let uwp = RwSignal::new(DEFAULT_UWP.to_string());
    let seed = RwSignal::new(DEFAULT_SEED);
    let error = RwSignal::new(None::<String>);
    // Default the legend visible — the key is core to reading the map and
    // many users won't think to look for it.
    let show_key = RwSignal::new(true);

    // Debounce UWP edits — generation is 100-500ms in WASM, so regenerate
    // only after the user pauses typing. `seed` is *not* debounced: re-roll
    // clicks should regenerate immediately.
    let uwp_debounced: Signal<String> = signal_debounced(uwp, 400.0);

    // "Generating…" indicator state. Two contributing signals:
    //   1. raw `uwp` differs from `uwp_debounced` — i.e. the user is
    //      mid-keystroke and the rendered map is stale.
    //   2. `pending_render`: raised before scheduling the heavy WASM compute
    //      and cleared after that compute returns. With the deferred-render
    //      pattern below, the badge actually paints before the freeze.
    let pending_render = RwSignal::new(false);

    // Manually-driven SVG signal. We previously used a Memo, but a Memo
    // recomputes synchronously the first time it's read inside the view —
    // which means setting `pending_render` in a sibling Effect never gets a
    // paint cycle in before the main thread blocks on
    // `worldmap::generate + render_svg` (often 500ms-2s). Instead we run the
    // compute via a 0ms `setTimeout`, which lets the browser paint the
    // "Generating…" badge first, then yield back into WASM for the work.
    //
    // KNOWN FOLLOW-UP: the synchronous compute still blocks the main thread
    // for ~500ms-2s. Future improvement: offload `worldmap::generate +
    // render_svg` to a Web Worker so the main thread stays responsive even
    // for large maps. setTimeout(0) gives the browser regular event-loop
    // turns (avoiding the "kill page?" prompt) but doesn't actually
    // parallelise — a worker would.
    let svg_html = RwSignal::new(String::new());

    let render_now = move |uwp_str: String, seed_v: u64| {
        if uwp_str.chars().filter(|c| *c != '-').count() < 8 {
            error.set(None);
            svg_html.set(String::new());
            return;
        }
        match worldmap::generate(&uwp_str, seed_v) {
            Ok(map) => {
                error.set(None);
                svg_html.set(worldmap::render_svg(&map));
            }
            Err(e) => {
                error.set(Some(e.to_string()));
                svg_html.set(String::new());
            }
        }
    };

    // Watch the (debounced uwp, seed) pair. On the initial run (`prev` is
    // None) we still need to fire so the default UWP/seed produces a map on
    // cold start. On subsequent changes, raise the badge and defer the
    // compute via setTimeout(0) so the badge paints first.
    Effect::new(move |prev: Option<(String, u64)>| {
        let cur = (uwp_debounced.get(), seed.get());
        let changed = prev.is_none() || prev.as_ref().is_some_and(|p| p != &cur);
        if changed {
            pending_render.set(true);
            let uwp_str = cur.0.clone();
            let seed_v = cur.1;
            let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                render_now(uwp_str, seed_v);
                pending_render.set(false);
            });
            if let Some(win) = web_sys::window() {
                let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    0,
                );
            }
        }
        cur
    });

    let pending = Signal::derive(move || uwp.get() != uwp_debounced.get() || pending_render.get());

    let reroll = move |_| {
        seed.update(|s| *s = s.wrapping_add(1));
    };

    // PNG generation runs synchronously in WASM and can take 1–3s. Raise the
    // badge before kicking it off, then defer the actual work via a 0ms
    // timeout so the browser gets one paint cycle to actually show the
    // indicator before the main thread blocks.
    let download_png = move |_| {
        pending_render.set(true);
        let uwp_now = uwp.get();
        let seed_now = seed.get();
        let error_handle = error;
        let pending_handle = pending_render;
        let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
            match worldmap::generate(&uwp_now, seed_now)
                .map_err(|e| e.to_string())
                .and_then(|m| worldmap::render_png(&m))
            {
                Ok(bytes) => {
                    if let Err(e) = trigger_png_download(&bytes, "worldmap.png") {
                        error_handle.set(Some(format!("PNG download failed: {e:?}")));
                    }
                }
                Err(e) => error_handle.set(Some(e)),
            }
            pending_handle.set(false);
        });
        if let Some(win) = web_sys::window() {
            let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                0,
            );
        }
    };

    // SVG includes a legend strip in a band below the map (viewBox extends
    // past SHEET_HEIGHT by LEGEND_HEIGHT). When the user unchecks "Key" we
    // hide that band by clipping the bottom of the SVG. The clip-pct must
    // stay in sync with `render::LEGEND_HEIGHT / (SHEET_HEIGHT + LEGEND_HEIGHT)`.
    // SHEET_HEIGHT = 520, LEGEND_HEIGHT = 135, total = 655, legend = 20.6%.
    let canvas_clip = move || {
        if show_key.get() {
            "clip-path: none;".to_string()
        } else {
            "clip-path: inset(0 0 20.6% 0);".to_string()
        }
    };

    view! {
        <div class:App>
            <h1>"World Map Generator"</h1>
            <div class="d-print-none key-region world-entry-form">
                <label>
                    "UWP "
                    <input type="text"
                        prop:value=move || uwp.get()
                        on:input=move |ev| uwp.set(event_target_value(&ev))
                        size="9"
                        style="width: 7rem;"
                    />
                </label>
                <label style="margin-left: 1rem;">
                    "Seed "
                    <input type="number"
                        prop:value=move || seed.get().to_string()
                        on:input=move |ev| {
                            if let Ok(n) = event_target_value(&ev).parse::<u64>() {
                                seed.set(n);
                            }
                        }
                        size="14"
                        style="width: 9rem;"
                    />
                </label>
                <button class="blue-button" on:click=reroll style="margin-left: 1rem;">
                    "Re-roll"
                </button>
                <button class="blue-button" on:click=download_png style="margin-left: 0.5rem;">
                    "Download PNG"
                </button>
                <label style="margin-left: 0.75rem; display: inline-flex; align-items: center; gap: 0.25rem;" title="Show map legend strip below the map">
                    <input
                        type="checkbox"
                        prop:checked=move || show_key.get()
                        on:change=move |ev| show_key.set(event_target_checked(&ev))
                    />
                    "Key"
                </label>
                {move || error.get().map(|e| view! { <div style="color:#c33;margin-top:0.5rem;">{e}</div> })}
            </div>
            <div style="position: relative;">
                <div
                    class="worldmap-canvas"
                    style=canvas_clip
                    inner_html=move || svg_html.get()
                />
                {move || pending.get().then(|| view! { <GeneratingBadge /> })}
            </div>
        </div>
    }
}

/// "Generating…" overlay centered on the map canvas while a regeneration is
/// in flight (user is typing past the debounce, or the debounced UWP / seed
/// just changed). Centered + a translucent ring so it stays legible against
/// any biome color underneath. Inlines a tiny CSS keyframes block for the
/// spinner — no global stylesheet edit needed.
#[component]
fn GeneratingBadge() -> impl IntoView {
    view! {
        <div
            style="position: absolute; top: 50%; left: 50%; \
                   transform: translate(-50%, -50%); \
                   display: flex; align-items: center; gap: 0.55rem; \
                   background: rgba(0,0,0,0.7); color: #f6f4ec; \
                   padding: 0.5rem 1rem; border-radius: 999px; \
                   font-size: 1rem; font-weight: 500; \
                   pointer-events: none; \
                   box-shadow: 0 4px 14px rgba(0,0,0,0.45), \
                               0 0 0 3px rgba(246,244,236,0.25); \
                   z-index: 5;"
            aria-live="polite"
        >
            <style>
                {".worldmap-spinner { \
                     width: 1.05rem; height: 1.05rem; \
                     border: 2px solid rgba(246,244,236,0.35); \
                     border-top-color: #f6f4ec; \
                     border-radius: 50%; \
                     animation: worldmap-spin 0.8s linear infinite; \
                  } \
                  @keyframes worldmap-spin { to { transform: rotate(360deg); } }"}
            </style>
            <span class="worldmap-spinner" />
            <span>"Generating…"</span>
        </div>
    }
}

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
    document.body().unwrap().append_child(&a)?;
    a.click();
    document.body().unwrap().remove_child(&a)?;

    Url::revoke_object_url(&url)?;
    Ok(())
}
