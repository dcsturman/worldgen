//! Leptos component: World Map generator.
//!
//! Takes a UWP + seed, calls into `worldgen::worldmap::generate`, and shows
//! the result in one of two projections, chosen by the Flat/Globe switch:
//!
//! - **Flat** — the equirectangular SVG (which already includes the legend
//!   strip below the map). The "Key" checkbox toggles whether the legend
//!   strip is shown; the bundled legend is always part of the downloaded PNG
//!   so saved maps carry their key with them.
//! - **Globe** — an orthographic projection of the *same* terrain
//!   ([`worldmap::globe`]), warped onto a sphere and spun on a `<canvas>` via
//!   `requestAnimationFrame`. The surface texture is built once per
//!   world; each animation tick just re-warps it at an advancing longitude,
//!   so the spin is smooth and cheap.
//!
//! Regeneration is split into async phases with a `setTimeout(0)` yield
//! between each so the browser gets event-loop turns mid-compute. Without the
//! chunking the WASM main thread blocks for 1-2s on a fast machine and well
//! past Chrome's 5s "Page Unresponsive" cutoff on slower ones.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, BlobPropertyBag, CanvasRenderingContext2d, HtmlAnchorElement, HtmlCanvasElement,
    ImageData, Url, UrlSearchParams,
};

use crate::worldmap::{self, GlobeTexture};

const DEFAULT_UWP: &str = "A788899-A";
const DEFAULT_SEED: u64 = 0xC0FFEE;

/// Side length (px) of the on-screen globe canvas.
const GLOBE_DISPLAY_SIZE: u32 = 460;
/// Radians of spin advanced per animation frame. ~0.01 rad at 60 fps ≈ 0.6
/// rad/s → roughly a 10-second rotation: a slow, readable turn.
const GLOBE_SPIN_PER_FRAME: f64 = 0.01;
/// DOM id of the globe canvas, so the animation loop can fetch it without
/// threading a typed `NodeRef` through the async build.
const GLOBE_CANVAS_ID: &str = "worldmap-globe-canvas";

/// Pull `?uwp=…&seed=…&name=…` off the current URL.
///
/// Returns the parsed values and a flag indicating whether *any* of the
/// three params were present — used by the cold-start path to decide
/// whether to auto-render. With no params we keep the original behavior
/// (blank canvas until the user clicks Regenerate); with any param
/// supplied (the typical "open from system view" case) we render
/// immediately so the new tab isn't useless.
fn read_query_params() -> (Option<String>, Option<u64>, Option<String>, bool) {
    let Some(window) = web_sys::window() else {
        return (None, None, None, false);
    };
    let Ok(search) = window.location().search() else {
        return (None, None, None, false);
    };
    let Ok(params) = UrlSearchParams::new_with_str(&search) else {
        return (None, None, None, false);
    };
    let uwp = params.get("uwp");
    let seed = params.get("seed").and_then(|s| s.parse::<u64>().ok());
    let name = params.get("name");
    let any = uwp.is_some() || seed.is_some() || name.is_some();
    (uwp, seed, name, any)
}

#[component]
pub fn WorldMap() -> impl IntoView {
    let (qp_uwp, qp_seed, qp_name, has_query_params) = read_query_params();

    let uwp = RwSignal::new(qp_uwp.unwrap_or_else(|| DEFAULT_UWP.to_string()));
    let seed = RwSignal::new(qp_seed.unwrap_or(DEFAULT_SEED));
    let world_name = RwSignal::new(qp_name);
    let error = RwSignal::new(None::<String>);
    // Default the legend visible — the key is core to reading the map and
    // many users won't think to look for it.
    let show_key = RwSignal::new(true);
    // Projection toggle: false = flat (SVG), true = spinning globe (canvas).
    let globe_mode = RwSignal::new(false);

    // Manually-triggered regeneration. UWP and seed inputs no longer auto-
    // regen on change (typing was painful — every keystroke kicked off a
    // 1-2s WASM compute). The user edits both freely, then clicks the
    // Regenerate button, which bumps this counter and triggers a single
    // render with the current input values. We deliberately do NOT fire
    // on cold start *unless* the URL supplied query params: arriving with
    // ?uwp=…&seed=… means the user came in from a "Map" link and expects
    // the map immediately, but a bare /worldmap visit stays blank so a
    // 1-2s WASM compute doesn't blindside them.
    let regen = RwSignal::new(0u32);
    let pending_render = RwSignal::new(false);

    let svg_html = RwSignal::new(String::new());

    // Handle to the running globe animation, if any. Lives for the life of
    // the component; the globe effect stops the previous loop before
    // starting a new one, and `on_cleanup` stops it when the component is
    // torn down. Holds `Rc`s, so it stays out of any signal (which require
    // Send) and lives in a plain interior-mutable cell instead.
    let anim: Rc<RefCell<Option<GlobeAnim>>> = Rc::new(RefCell::new(None));

    // Watch the regen counter. Cold-start fires only when query params
    // hydrated the inputs (`has_query_params`); otherwise the first tick
    // is skipped and the map stays blank until the Regenerate button
    // bumps the counter. Reads uwp/seed at fire time so editing the
    // inputs doesn't trigger a render — only the button does.
    Effect::new(move |prev: Option<u32>| {
        let cur = regen.get();
        let changed = matches!(prev, Some(p) if p != cur);
        let cold_with_params = prev.is_none() && has_query_params;
        if changed || cold_with_params {
            let uwp_str = uwp.get_untracked();
            let seed_v = seed.get_untracked();
            let name_v = world_name.get_untracked();
            spawn_local(async move {
                pending_render.set(true);
                if uwp_str.chars().filter(|c| *c != '-').count() < 8 {
                    error.set(None);
                    svg_html.set(String::new());
                    pending_render.set(false);
                    return;
                }
                yield_to_browser().await;
                let map = match worldmap::generate(&uwp_str, seed_v, name_v.as_deref()) {
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
                let mut job =
                    worldmap::RasterJob::new(&map, worldmap::SVG_RASTER_W, worldmap::SVG_RASTER_H);
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

    // Globe effect: (re)build the sphere texture and (re)start the spin
    // whenever the projection is switched to Globe, or a Regenerate happens
    // while Globe is showing. Switching back to Flat stops the loop. A
    // generation counter guards against a stale async build (from a quick
    // toggle/regenerate) starting an animation after a newer one.
    let build_gen = Rc::new(Cell::new(0u32));
    {
        let anim = anim.clone();
        let build_gen = build_gen.clone();
        Effect::new(move |_| {
            let on = globe_mode.get();
            let _ = regen.get(); // also rebuild on Regenerate while in globe mode

            // Any state change invalidates an in-flight build and stops the
            // current loop.
            let my_gen = build_gen.get().wrapping_add(1);
            build_gen.set(my_gen);
            if let Some(a) = anim.borrow_mut().take() {
                a.stop();
            }
            if !on {
                return;
            }

            let uwp_str = uwp.get_untracked();
            let seed_v = seed.get_untracked();
            let name_v = world_name.get_untracked();
            let anim = anim.clone();
            let build_gen = build_gen.clone();
            spawn_local(async move {
                if uwp_str.chars().filter(|c| *c != '-').count() < 8 {
                    error.set(Some("UWP must be at least 8 characters".to_string()));
                    return;
                }
                pending_render.set(true);
                yield_to_browser().await;
                let map = match worldmap::generate(&uwp_str, seed_v, name_v.as_deref()) {
                    Ok(m) => m,
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        pending_render.set(false);
                        return;
                    }
                };
                error.set(None);
                yield_to_browser().await;
                // Sample the surface into a full equirectangular texture, the
                // one expensive pass (warping each frame from it is cheap).
                // Chunked across yields like the flat-map raster so a slow
                // machine never blocks long enough to trip the browser's
                // "Page Unresponsive" dialog.
                let mut job =
                    worldmap::GlobeTextureJob::new(worldmap::globe::TEX_W, worldmap::globe::TEX_H);
                job.step_elevation(&map);
                yield_to_browser().await;
                job.step_color(&map);
                job.populate_city_lights(&map);
                yield_to_browser().await;
                let tex = job.into_texture();
                pending_render.set(false);
                yield_to_browser().await;

                // A newer toggle/regenerate superseded this build — drop it.
                if build_gen.get() != my_gen {
                    return;
                }
                if let Some(loop_) = start_globe_loop(Rc::new(tex), GLOBE_DISPLAY_SIZE)
                    && let Some(old) = anim.borrow_mut().replace(loop_)
                {
                    old.stop();
                }
            });
        });
    }

    // No explicit `on_cleanup` (it requires Send+Sync, which `Rc` isn't):
    // `GlobeAnim`'s `Drop` stops the loop, and the `anim` `Rc` is owned by
    // the effect closures — when the component is disposed they drop, the
    // last `Rc` clone drops the `GlobeAnim`, and the spin halts.

    let pending = Signal::derive(move || pending_render.get());

    let regenerate = move |_| {
        regen.update(|n| *n = n.wrapping_add(1));
    };

    // Download adapts to the current projection: a static PNG of the flat map,
    // or a spinning APNG of the globe. Both reuse the chunked pipeline so the
    // main thread never blocks long enough to trip an unresponsive-page prompt.
    let download = move |_| {
        let uwp_now = uwp.get();
        let seed_now = seed.get();
        let name_now = world_name.get();
        let is_globe = globe_mode.get();
        spawn_local(async move {
            pending_render.set(true);
            yield_to_browser().await;
            let result = match worldmap::generate(&uwp_now, seed_now, name_now.as_deref()) {
                Ok(map) => {
                    yield_to_browser().await;
                    if is_globe {
                        // Match the server's spinning-globe defaults.
                        worldmap::render_globe_apng(&map, GLOBE_DISPLAY_SIZE, 36, 1, 5)
                    } else {
                        worldmap::render_png(&map)
                    }
                }
                Err(e) => Err(e.to_string()),
            };
            match result {
                Ok(bytes) => {
                    let filename = if is_globe {
                        "worldmap-globe.png"
                    } else {
                        "worldmap.png"
                    };
                    if let Err(e) = trigger_png_download(&bytes, filename) {
                        error.set(Some(format!("Download failed: {e:?}")));
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

    // Show/hide each projection's surface without unmounting it, so the
    // canvas element stays in the DOM (the animation loop fetches it by id).
    let flat_display = move || if globe_mode.get() { "display: none;" } else { "" };
    let globe_display = move || if globe_mode.get() { "" } else { "display: none;" };

    view! {
        <div class:App>
            <h1>
                {move || match world_name.get() {
                    Some(n) if !n.is_empty() => format!("World Map: {n}"),
                    _ => "World Map Generator".to_string(),
                }}
            </h1>
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
                <button class="blue-button" on:click=download style="margin-left: 0.5rem;">
                    {move || if globe_mode.get() { "Download APNG" } else { "Download PNG" }}
                </button>
                // Flat ⟷ Globe projection switch.
                <span class="proj-toggle" title="Switch between the flat map and a spinning globe">
                    <span class="proj-toggle-label" class:active=move || !globe_mode.get()>"Flat"</span>
                    <label class="switch">
                        <input
                            type="checkbox"
                            prop:checked=move || globe_mode.get()
                            on:change=move |ev| globe_mode.set(event_target_checked(&ev))
                        />
                        <span class="slider"></span>
                    </label>
                    <span class="proj-toggle-label" class:active=move || globe_mode.get()>"Globe"</span>
                </span>
                <label
                    class:hidden=move || globe_mode.get()
                    style="margin-left: 0.75rem; display: inline-flex; align-items: center; gap: 0.25rem;"
                    title="Show map legend strip below the map"
                >
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
                    style=move || format!("{} {}", canvas_clip(), flat_display())
                    inner_html=move || svg_html.get()
                />
                <canvas
                    id=GLOBE_CANVAS_ID
                    class="worldmap-globe"
                    width=GLOBE_DISPLAY_SIZE
                    height=GLOBE_DISPLAY_SIZE
                    style=globe_display
                />
                {move || pending.get().then(|| view! { <GeneratingBadge /> })}
            </div>
        </div>
    }
}

/// Shared, lazily-initialized cell holding the self-rescheduling rAF closure.
/// The loop reschedules through a `Weak` to this, so the strong owner
/// ([`GlobeAnim`]) is what keeps it alive.
type RafClosureCell = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// A running globe animation. Dropping/stopping it cancels the pending
/// `requestAnimationFrame` callback and frees the closure (no leak across
/// regenerations).
struct GlobeAnim {
    cancel: Rc<Cell<bool>>,
    handle: Rc<Cell<Option<i32>>>,
    // Sole strong owner of the rAF closure; the loop reschedules via a Weak,
    // so dropping this (after cancelling the pending frame) frees the closure.
    _closure: RafClosureCell,
}

impl GlobeAnim {
    fn stop(&self) {
        self.cancel.set(true);
        if let Some(h) = self.handle.take()
            && let Some(win) = web_sys::window()
        {
            let _ = win.cancel_animation_frame(h);
        }
    }
}

impl Drop for GlobeAnim {
    fn drop(&mut self) {
        // Cancel the pending frame *before* `_closure` is freed, so the
        // browser never invokes a dropped wasm closure.
        self.stop();
    }
}

/// Build and start a spinning-globe animation on the canvas identified by
/// [`GLOBE_CANVAS_ID`]. Returns `None` if the canvas or its 2D context can't
/// be found. The returned [`GlobeAnim`] keeps the loop alive until stopped.
fn start_globe_loop(tex: Rc<GlobeTexture>, size: u32) -> Option<GlobeAnim> {
    let document = web_sys::window()?.document()?;
    let canvas = document
        .get_element_by_id(GLOBE_CANVAS_ID)?
        .dyn_into::<HtmlCanvasElement>()
        .ok()?;
    canvas.set_width(size);
    canvas.set_height(size);
    let ctx = canvas
        .get_context("2d")
        .ok()??
        .dyn_into::<CanvasRenderingContext2d>()
        .ok()?;

    let cancel = Rc::new(Cell::new(false));
    let handle: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));

    let mut buf = vec![0u8; (size as usize) * (size as usize) * 4];
    let mut spin = 0.0f64;

    let closure: RafClosureCell = Rc::new(RefCell::new(None));
    let weak = Rc::downgrade(&closure);
    let cancel_c = cancel.clone();
    let handle_c = handle.clone();

    *closure.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        if cancel_c.get() {
            return;
        }
        tex.warp_into(&mut buf, size, spin);
        spin += GLOBE_SPIN_PER_FRAME;
        if spin > std::f64::consts::TAU {
            spin -= std::f64::consts::TAU;
        }
        // ImageData wants RGBA; our buffer already is. Blit to the canvas.
        if let Ok(image_data) =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&buf), size, size)
        {
            let _ = ctx.put_image_data(&image_data, 0.0, 0.0);
        }
        // Schedule the next frame via the Weak so this closure isn't kept
        // alive by its own retain cycle.
        if let Some(strong) = weak.upgrade()
            && let Some(cb) = strong.borrow().as_ref()
        {
            handle_c.set(request_animation_frame(cb));
        }
    }) as Box<dyn FnMut()>));

    if let Some(cb) = closure.borrow().as_ref() {
        handle.set(request_animation_frame(cb));
    }

    Some(GlobeAnim {
        cancel,
        handle,
        _closure: closure,
    })
}

/// Schedule `cb` for the next animation frame, returning the request handle
/// (so it can be cancelled) or `None` if there's no window.
fn request_animation_frame(cb: &Closure<dyn FnMut()>) -> Option<i32> {
    web_sys::window()?
        .request_animation_frame(cb.as_ref().unchecked_ref())
        .ok()
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
