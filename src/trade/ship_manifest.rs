//! # Ship Manifest Module
//!
//! This module defines the ship manifest structure and revenue calculation
//! functionality for passenger and freight transport in the Traveller universe.
//!
//! The manifest tracks different classes of passengers and freight lots,
//! and calculates revenue based on distance traveled and passenger/freight types.

/// Represents a ship's manifest of passengers and freight
///
/// Tracks the number of passengers in each class and the indices of
/// freight lots being carried. Used to calculate total revenue for
/// a trading voyage between worlds.
#[derive(Debug, Clone, Default)]
pub struct ShipManifest {
    /// Number of high passage passengers (luxury accommodations)
    pub high_passengers: i32,
    /// Number of middle passage passengers (standard accommodations)
    pub medium_passengers: i32,
    /// Number of basic passage passengers (economy accommodations)
    pub basic_passengers: i32,
    /// Number of low passage passengers (cold sleep/suspended animation)
    pub low_passengers: i32,
    /// Indices of freight lots from available freight being carried
    pub freight_lot_indices: Vec<usize>,
}

/// Revenue (in credits) per high passage passenger by distance (in parsecs)
///
/// Index 0 is unused, indices 1-6 represent jump distances 1-6
const HIGH_COST: [i32; 7] = [0, 9000, 14000, 21000, 34000, 60000, 210000];

/// Revenue per middle passage passenger by distance (in parsecs)
///
/// Index 0 is unused, indices 1-6 represent jump distances 1-6
const MEDIUM_COST: [i32; 7] = [0, 6500, 10000, 14000, 23000, 40000, 130000];

/// Revenue per basic passage passenger by distance (in parsecs)
///
/// Index 0 is unused, indices 1-6 represent jump distances 1-6
const BASIC_COST: [i32; 7] = [0, 2000, 3000, 5000, 8000, 14000, 55000];

/// Revenue per low passage passenger by distance (in parsecs)
///
/// Index 0 is unused, indices 1-6 represent jump distances 1-6
const LOW_COST: [i32; 7] = [0, 700, 1300, 2200, 3900, 7200, 27000];

/// Revenue per freight lot by distance (in parsecs)
///
/// Index 0 is unused, indices 1-6 represent jump distances 1-6
const FREIGHT_COST: [i32; 7] = [0, 1000, 1600, 2600, 4400, 8500, 32000];

impl ShipManifest {
    /// Calculates total passenger revenue for the manifest
    ///
    /// Computes revenue based on the number of passengers in each class
    /// and the distance traveled. Revenue scales with both passenger
    /// class and jump distance.
    ///
    /// # Arguments
    ///
    /// * `distance` - Jump distance in parsecs (clamped to 1-6)
    ///
    /// # Returns
    ///
    /// Total passenger revenue in credits
    ///
    /// # Examples
    ///
    /// ```
    /// use worldgen::trade::ship_manifest::ShipManifest;
    ///
    /// let mut manifest = ShipManifest::default();
    /// manifest.high_passengers = 2;
    /// manifest.medium_passengers = 4;
    ///
    /// let revenue = manifest.passenger_revenue(2);
    /// // Returns revenue for 2 high + 4 medium passengers at distance 2
    /// ```
    pub fn passenger_revenue(&self, distance: i32) -> i32 {
        let distance_index = distance.clamp(1, 6) as usize;
        let high_cost = HIGH_COST[distance_index] * self.high_passengers;
        let medium_cost = MEDIUM_COST[distance_index] * self.medium_passengers;
        let basic_cost = BASIC_COST[distance_index] * self.basic_passengers;
        let low_cost = LOW_COST[distance_index] * self.low_passengers;
        high_cost + medium_cost + basic_cost + low_cost
    }

    /// Calculates total freight revenue for the manifest
    ///
    /// Computes revenue based on the number of freight lots being carried
    /// and the distance traveled. Each freight lot generates the same
    /// revenue regardless of size.
    ///
    /// # Arguments
    ///
    /// * `distance` - Jump distance in parsecs (clamped to 1-6)
    ///
    /// # Returns
    ///
    /// Total freight revenue in credits
    ///
    /// # Examples
    ///
    /// ```
    /// use worldgen::trade::ship_manifest::ShipManifest;
    ///
    /// let mut manifest = ShipManifest::default();
    /// manifest.freight_lot_indices = vec![0, 2, 5]; // 3 freight lots
    ///
    /// let revenue = manifest.freight_revenue(3);
    /// // Returns revenue for 3 freight lots at distance 3
    /// ```
    pub fn freight_revenue(&self, distance: i32) -> i32 {
        let distance_index = distance.clamp(1, 6) as usize;
        FREIGHT_COST[distance_index] * self.freight_lot_indices.len() as i32
    }
}
