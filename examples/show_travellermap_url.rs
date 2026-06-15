//! Tiny verification tool: prints the TravellerMap base URL that
//! `util::travellermap_base_url()` resolves to in the current build.
//!
//! The URL is resolved at compile time via `option_env!`, so this
//! tells you which URL is baked into binaries built right now from
//! the current `TRAVELLERMAP_URL` env var (or the default if unset).
//!
//! Usage:
//!
//! ```text
//! cargo run --no-default-features --example show_travellermap_url
//! TRAVELLERMAP_URL=https://my.tmap.local \
//!     cargo run --no-default-features --example show_travellermap_url
//! ```

fn main() {
    println!(
        "travellermap_base_url() = {:?}",
        worldgen::util::travellermap_base_url()
    );
}
