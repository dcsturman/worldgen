//! Worlds stored before Callisto still load and show as they did
//! (IMPLEMENTATION.md §10).
//!
//! The trade computer keeps each session's `TradeState`, with its origin and
//! destination `World`s, in Firestore. `fixtures/pre_callisto_worlds.json`
//! is JSON written by worldgen at b7cf0da, the commit before Callisto stage 0
//! (PR #62): main worlds as the trade computer builds them, and Book 6
//! planets, moons and belts with their facilities. Its `shown` list is what
//! that code displayed for each world. Never regenerate it: it stands for
//! what was stored then.

use serde::Deserialize;
use worldgen::comms::TradeState;
use worldgen::systems::world::World;

#[derive(Deserialize)]
struct Shown {
    name: String,
    uwp: String,
    facilities: String,
    trade_classes: String,
}

#[derive(Deserialize)]
struct Fixture {
    worlds: Vec<World>,
    shown: Vec<Shown>,
    trade_state: TradeState,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/pre_callisto_worlds.json"))
        .expect("JSON stored before Callisto still deserializes")
}

#[test]
fn pre_callisto_worlds_load_and_show_as_they_did() {
    let f = fixture();
    assert_eq!(f.worlds.len(), f.shown.len());
    for (w, shown) in f.worlds.iter().zip(&f.shown) {
        assert_eq!(w.name, shown.name);
        assert_eq!(w.to_uwp(), shown.uwp, "{}: UWP", shown.name);
        assert_eq!(w.facilities_string(), shown.facilities, "{}: facilities", shown.name);
        assert_eq!(w.trade_classes_string(), shown.trade_classes, "{}: trade codes", shown.name);
        // Nothing Callisto adds appears on a world stored without it.
        assert!(w.callisto.is_none(), "{}: no Callisto physics", shown.name);
        assert!(w.decorations().is_empty(), "{}: no decorations", shown.name);
    }
}

#[test]
fn a_pre_callisto_trade_session_loads() {
    let state = fixture().trade_state;
    let origin = state.origin_world.expect("origin world kept");
    let dest = state.dest_world.expect("destination world kept");
    assert_eq!((origin.name.as_str(), origin.to_uwp().as_str()), ("Regina", "A788899-C"));
    assert_eq!((dest.name.as_str(), dest.to_uwp().as_str()), ("Pourne", "A9B2887-A"));
}
