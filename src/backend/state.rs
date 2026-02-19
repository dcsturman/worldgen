//! # Trade State Module
//!
//! This module defines the unified `TradeState` struct that combines all trade computer
//! state into a single JSON-serializable object for storage in Firestore.
//!
//! ## Overview
//!
//! Previously, the trade computer stored state in multiple browser local storage keys:
//! - `worldgen:origin_world:v1` - The origin world
//! - `worldgen:dest_world:v1` - The destination world (optional)
//! - `worldgen:available_goods:v1` - Available goods table
//! - `worldgen:available_passengers:v1` - Available passengers (optional)
//! - `worldgen:manifest:v1` - Ship manifest
//! - `worldgen:buyer_broker_skill:v1` - Player's broker skill
//! - `worldgen:seller_broker_skill:v1` - System broker skill
//! - `worldgen:steward_skill:v1` - Steward skill
//! - `worldgen:illegal_goods:v1` - Whether to include illegal goods
//!
//! This module consolidates all of these into a single `TradeState` struct that can be
//! stored as a single Firestore document, enabling multi-user synchronization.

use serde::{Deserialize, Serialize};

use crate::systems::world::World;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;
use crate::{INITIAL_NAME, INITIAL_UPP};

/// Unified trade state for Firestore storage
///
/// This struct combines all trade computer state into a single JSON-serializable
/// object. It is stored as a single document in Firestore, enabling real-time
/// synchronization across multiple users.
///
/// ## Document Structure
///
/// In Firestore, this is stored as:
/// - Collection: `trade_sessions`
/// - Document ID: Session identifier (e.g., "default" for shared state)
/// - Fields: All fields from this struct, serialized as JSON
///
/// ## Versioning
///
/// The `version` field allows for future schema migrations. When loading state,
/// the application can check the version and apply any necessary transformations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TradeState {
    /// Schema version for future migrations
    pub version: u32,

    /// The origin world (always exists, starts with default value)
    pub origin_world: World,

    /// The destination world (optional - trade computer works without it)
    pub dest_world: Option<World>,

    /// Available goods table for the current origin world
    pub available_goods: AvailableGoodsTable,

    /// Available passengers and freight (only when destination is set)
    pub available_passengers: Option<AvailablePassengers>,

    /// Ship manifest containing cargo, passengers, and accumulated profit
    pub ship_manifest: ShipManifest,

    /// Player's broker skill (affects purchase prices)
    pub buyer_broker_skill: i16,

    /// System broker skill (affects selling prices)
    pub seller_broker_skill: i16,

    /// Steward skill (affects passenger generation and revenue)
    pub steward_skill: i16,

    /// Whether to include illegal goods in market generation
    pub illegal_goods: bool,
}

impl Default for TradeState {
    fn default() -> Self {
        // Create the default origin world from the initial UPP
        let origin_world = World::from_upp(INITIAL_NAME, INITIAL_UPP, false, true)
            .expect("Default UPP should always be valid");

        Self {
            version: 1,
            origin_world,
            dest_world: None,
            available_goods: AvailableGoodsTable::default(),
            available_passengers: None,
            ship_manifest: ShipManifest::default(),
            buyer_broker_skill: 0,
            seller_broker_skill: 0,
            steward_skill: 0,
            illegal_goods: false,
        }
    }
}

impl TradeState {
    /// Current schema version
    pub const CURRENT_VERSION: u32 = 1;

    /// Creates a new TradeState with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates and potentially migrates state from an older version
    ///
    /// This method should be called after loading state from Firestore
    /// to ensure compatibility with the current schema version.
    pub fn migrate(mut self) -> Self {
        // Future migrations would go here
        // Example:
        // if self.version < 2 {
        //     // Apply migration from v1 to v2
        //     self.version = 2;
        // }

        // Ensure version is current
        self.version = Self::CURRENT_VERSION;
        self
    }
}
