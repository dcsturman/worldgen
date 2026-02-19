//! # Trade State
//!
//! Shared state structure for synchronizing trade data between clients and server.

use serde::{Deserialize, Serialize};

use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;
use crate::trade::ZoneClassification;

/// The synchronized trade state shared between all connected clients
///
/// Worlds are NOT included in the state - instead, world name, UWP, coordinates, and zone are sent.
/// The server generates World objects from these fields and calculates distance from coordinates.
/// Clients still use TravellerMap for world lookup (user picks the world), but the server is
/// authoritative for World generation, trade tables, pricing, and passenger generation.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TradeState {
    /// Version number for state compatibility
    pub version: u32,
    /// Origin world name
    pub origin_world_name: String,
    /// Origin world UWP (9-character code)
    pub origin_uwp: String,
    /// Origin world galactic hex coordinates (from TravellerMap)
    pub origin_coords: Option<(i32, i32)>,
    /// Origin world travel zone classification
    pub origin_zone: ZoneClassification,
    /// Destination world name (empty if no destination)
    pub dest_world_name: String,
    /// Destination world UWP (empty if no destination)
    pub dest_uwp: String,
    /// Destination world galactic hex coordinates (from TravellerMap)
    pub dest_coords: Option<(i32, i32)>,
    /// Destination world travel zone classification
    pub dest_zone: ZoneClassification,
    /// Available goods at the origin
    pub available_goods: AvailableGoodsTable,
    /// Available passengers at the origin
    pub available_passengers: Option<AvailablePassengers>,
    /// Current ship manifest (selected goods, passengers, freight)
    pub ship_manifest: ShipManifest,
    /// Buyer's broker skill level
    pub buyer_broker_skill: i16,
    /// Seller's broker skill level
    pub seller_broker_skill: i16,
    /// Steward skill level (affects passenger recruitment)
    pub steward_skill: i16,
    /// Whether illegal goods are allowed
    pub illegal_goods: bool,
}

