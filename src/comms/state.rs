//! # Trade State
//!
//! Shared state structure for synchronizing trade data between clients and server.

use serde::{Deserialize, Serialize};

use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;

/// The synchronized trade state shared between all connected clients
///
/// Worlds are NOT included in the state - instead, world name and UWP are sent.
/// Clients regenerate World objects from name/UWP using the existing Effects.
/// This ensures unidirectional data flow: name/uwp → world (no loops).
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TradeState {
    /// Version number for state compatibility
    pub version: u32,
    /// Origin world name
    pub origin_world_name: String,
    /// Origin world UWP (9-character code)
    pub origin_uwp: String,
    /// Destination world name (empty if no destination)
    pub dest_world_name: String,
    /// Destination world UWP (empty if no destination)
    pub dest_uwp: String,
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

