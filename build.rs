//! Cargo build script.
//!
//! Tells cargo to rerun this script (and therefore invalidate the build
//! cache) whenever the `TRAVELLERMAP_URL` environment variable changes.
//! That env var is consumed at compile time by `option_env!` inside
//! [`crate::util::travellermap_base_url`] — without this directive,
//! cargo would happily reuse a binary built with the old URL baked in
//! even after the user changes the env var, which is exactly the kind
//! of "why isn't my change taking effect" trap that wastes hours.

fn main() {
    println!("cargo:rerun-if-env-changed=TRAVELLERMAP_URL");
}
