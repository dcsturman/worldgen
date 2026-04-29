//! Ship simulator — automated trade-run simulation.

pub mod economy;
pub mod incidents;
pub mod map_render;
pub mod protocol;
pub mod route;
pub mod types;

#[cfg(feature = "backend")]
pub mod executor;
#[cfg(feature = "backend")]
pub mod firestore;
#[cfg(feature = "backend")]
pub mod world_fetch;
