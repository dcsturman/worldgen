//! Minimal end-to-end consumer of the worldgen library API.
//!
//! Run with:
//!     cargo run --example external_consumer --no-default-features
//!
//! Writes `/tmp/system.png` and `/tmp/planet.png` for a fixed TravellerMap
//! identity (Noricum, Trojan Reach 3128). The PNG contents are the same
//! on every run — that's the whole point.

use std::fs;
use worldgen::seed::{planet_seed, system_seed};
use worldgen::{
    Constraint, PartialUwp, SystemConstraints, generate_planet_png, generate_system_png,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sector = "Trojan Reach";
    let (hex_x, hex_y) = (31, 28);
    let world_name = "Noricum";
    let world_uwp = "D8867BB-1";
    let main_world_orbit = 3;

    // Derive deterministic seeds from world identity.
    let sys_seed = system_seed(sector, hex_x, hex_y);
    let plan_seed = planet_seed(sys_seed, main_world_orbit, world_name);
    println!("system seed = {sys_seed:#018x}");
    println!("planet seed = {plan_seed:#018x}");

    // Build constraints for a single fully-specified main world.
    let constraints = SystemConstraints {
        bodies: vec![Constraint::Planet {
            name: Some(world_name.into()),
            orbit: None,
            uwp: Some(PartialUwp::parse(world_uwp)?),
            num_satellites: None,
            is_mainworld: true,
        }],
    };

    // Generate and write the system map.
    let system_png = generate_system_png(sys_seed, constraints)?;
    fs::write("/tmp/system.png", &system_png)?;
    println!("wrote /tmp/system.png ({} bytes)", system_png.len());

    // Generate and write the planet surface map.
    let planet_png = generate_planet_png(plan_seed, world_uwp, Some(world_name))?;
    fs::write("/tmp/planet.png", &planet_png)?;
    println!("wrote /tmp/planet.png ({} bytes)", planet_png.len());

    Ok(())
}
