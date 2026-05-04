//! Leptos component: World Map generator.
//!
//! Takes a UWP + seed, calls into `worldgen::worldmap::generate`, and shows
//! the SVG (which already includes the legend strip below the map). The
//! "Key" checkbox toggles whether the legend strip portion of the SVG is
//! shown in the live UI; the bundled legend is always part of the
//! downloaded PNG so saved maps carry their key with them.
//!
//! Regeneration is split into four async phases (generate → elevation
//! raster → color raster → assemble SVG) with a setTimeout(0) yield
//! between each so the browser gets event-loop turns mid-compute. Without
//! the chunking the WASM main thread blocks for 1-2s on a fast machine
//! and well past Chrome's 5s "Page Unresponsive" cutoff on slower ones.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::JsFuture;
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

    // Manually-triggered regeneration. UWP and seed inputs no longer auto-
    // regen on change (typing was painful — every keystroke kicked off a
    // 1-2s WASM compute). The user edits both freely, then clicks the
    // Regenerate button, which bumps this counter and triggers a single
    // render with the current input values. Cold start fires once so the
    // default UWP/seed shows up without requiring a click.
    let regen = RwSignal::new(0u32);
    let pending_render = RwSignal::new(false);

    let svg_html = RwSignal::new(String::new());

    // Watch the regen counter. Fires once on cold start (prev=None), then
    // every time the Regenerate button bumps it. Reads uwp/seed at fire
    // time so editing the inputs doesn't trigger a render — only the
    // button does.
    Effect::new(move |prev: Option<u32>| {
        let cur = regen.get();
        let changed = prev.is_none() || prev != Some(cur);
        if changed {
            let uwp_str = uwp.get_untracked();
            let seed_v = seed.get_untracked();
            spawn_local(async move {
                pending_render.set(true);
                if uwp_str.chars().filter(|c| *c != '-').count() < 8 {
                    error.set(None);
                    svg_html.set(String::new());
                    pending_render.set(false);
                    return;
                }
                yield_to_browser().await;
                let map = match worldmap::generate(&uwp_str, seed_v) {
                    Ok(m) => m,
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        svg_html.set(String::new());
                        pending_render.set(false);
                        return;
                    }
                };
                error.set(None);
                yield_to_browser().await;
                let mut job = worldmap::RasterJob::new(
                    &map,
                    worldmap::SVG_RASTER_W,
                    worldmap::SVG_RASTER_H,
                );
                job.step_elevation(&map);
                yield_to_browser().await;
                job.step_color(&map);
                yield_to_browser().await;
                job.step_postprocess();
                yield_to_browser().await;
                let raster = job.into_rgba();
                let svg = worldmap::assemble_svg(&map, &raster);
                svg_html.set(svg);
                pending_render.set(false);
            });
        }
        cur
    });

    let pending = Signal::derive(move || pending_render.get());

    let regenerate = move |_| {
        regen.update(|n| *n = n.wrapping_add(1));
    };

    // PNG export uses the same chunked pipeline: yield between generate,
    // each raster phase, and the final tiny_skia overlay/encode pass so
    // the main thread never blocks long enough to trigger an unresponsive-
    // page prompt. The PNG renderer redoes its own raster internally
    // (different scale + extra height for the legend), so once we have a
    // map we just call `worldmap::render_png` after a yield.
    let download_png = move |_| {
        let uwp_now = uwp.get();
        let seed_now = seed.get();
        spawn_local(async move {
            pending_render.set(true);
            yield_to_browser().await;
            let result = match worldmap::generate(&uwp_now, seed_now) {
                Ok(map) => {
                    yield_to_browser().await;
                    worldmap::render_png(&map)
                }
                Err(e) => Err(e.to_string()),
            };
            match result {
                Ok(bytes) => {
                    if let Err(e) = trigger_png_download(&bytes, "worldmap.png") {
                        error.set(Some(format!("PNG download failed: {e:?}")));
                    }
                }
                Err(e) => error.set(Some(e)),
            }
            pending_render.set(false);
        });
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
                <button class="blue-button" on:click=regenerate style="margin-left: 1rem;">
                    "Regenerate"
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

/// Yield the WASM main thread back to the browser for one event-loop
/// turn. Returns a future that resolves on the next setTimeout(0) tick,
/// which is enough for the browser to paint, run other event handlers,
/// and reset its "page is unresponsive" timer. Used between phases of the
/// regenerate pipeline.
async fn yield_to_browser() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let cb = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        if let Some(win) = web_sys::window() {
            let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                0,
            );
        }
    });
    let _ = JsFuture::from(promise).await;
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
