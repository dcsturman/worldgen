//! # Ship Manifest Module
//!
//! This module defines the ship manifest structure and revenue calculation
//! functionality for passenger and freight transport in the Traveller universe.
//!
//! The manifest tracks different classes of passengers, freight lots, and trade goods,
//! and calculates revenue based on distance traveled and passenger/freight types.
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use log::{debug, error};

use crate::systems::world::World;
use crate::trade::available_goods::{AvailableGoodsTable, Good};
use crate::trade::available_passengers::AvailablePassengers;

/// Represents a ship's manifest of passengers, freight, and trade goods
///
/// Tracks the number of passengers in each class, the indices of
/// freight lots being carried, and speculative trade goods purchased.
/// Used to calculate total revenue for a trading voyage between worlds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// Trade goods purchased for speculation
    pub trade_goods: AvailableGoodsTable,
    /// Accumulated profit across processed trades (in credits)
    pub profit: i64,
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
///
/// Index 0 is unused, indices 1-6 represent jump distances 1-6
const FREIGHT_COST: [i32; 7] = [0, 1000, 1600, 2600, 4400, 8500, 32000];

impl ShipManifest {
    /// Returns the total number of passengers in the manifest
    pub fn total_passengers_not_low(&self) -> i32 {
        self.high_passengers + self.medium_passengers + self.basic_passengers
    }

    /// Returns the total tonnage of freight in the manifest, given available_passengers
    pub fn total_freight_tons(&self, available_passengers: &AvailablePassengers) -> i32 {
        self.freight_lot_indices
            .iter()
            .map(|&index| {
                available_passengers
                    .freight_lots
                    .get(index)
                    .map(|lot| lot.size)
                    .unwrap_or(0)
            })
            .sum()
    }

    /// Price all goods in the manifest.
    ///
    /// This is very similar to AvailableGoodsTable::price_goods_to_sell, but we
    /// have individual goods vs the full table, so iterate over the goods and price
    /// each.  We use the trade classes of the world provided.
    pub fn price_goods(&mut self, world: &World, buyer: i16, supplier: i16) {
        let trade_classes = world.get_trade_classes();
        self.trade_goods
            .price_goods_to_sell(Some(trade_classes), buyer, supplier);
    }

    ///
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

    /// Adds or updates a trade good in the manifest
    ///
    /// If the good already exists in the manifest (matched by source_entry.index),
    /// updates its quantity. If the quantity becomes 0 or negative, removes the good.
    /// If the good doesn't exist and quantity > 0, adds it to the manifest.
    ///
    /// # Arguments
    ///
    /// * `good` - The trade good to add or update
    /// * `quantity` - The new quantity for this good in the manifest
    ///
    /// # Examples
    ///
    /// ```
    /// use worldgen::trade::ship_manifest::ShipManifest;
    /// use worldgen::trade::available_goods::Good;
    /// use worldgen::trade::table::{TradeTableEntry, Availability, Quantity};
    /// use std::collections::HashMap;
    ///
    /// let mut manifest = ShipManifest::default();
    /// let entry = TradeTableEntry {
    ///     index: 1,
    ///     name: "Test Good".to_string(),
    ///     availability: Availability::All,
    ///     quantity: Quantity { dice: 1, multiplier: 6 },
    ///     base_cost: 1000,
    ///     purchase_dm: HashMap::new(),
    ///     sale_dm: HashMap::new(),
    /// };
    /// let good = Good {
    ///     name: "Test Good".to_string(),
    ///     quantity: 10,
    ///     transacted: 0,
    ///     base_cost: 1000,
    ///     buy_cost: 1000,
    ///     buy_cost_comment: String::new(),
    ///     sell_price: None,
    ///     sell_price_comment: String::new(),
    ///     source_index: entry.index,
    /// };
    ///
    /// // Add a good with quantity 5
    /// let new_good = Good { quantity: 5, ..good.clone() };
    /// manifest.update_trade_good(new_good);
    /// // Update the same good to quantity 3
    /// let new_good = Good { quantity: 3, ..good.clone() }; 
    /// manifest.update_trade_good(new_good);
    /// // Remove the good by setting quantity to 0
    /// let new_good = Good { quantity: 0, ..good.clone() }; /// 
    /// manifest.update_trade_good(new_good);
    /// ```
    pub fn update_trade_good(&mut self, good: Good) {
        self.trade_goods.update_good(good);
    }

    /// Calculates the total tonnage of trade goods in the manifest
    ///
    /// Returns the sum of all trade good quantities currently in the manifest.
    ///
    /// # Returns
    ///
    /// Total tonnage of trade goods
    pub fn trade_goods_tonnage(&self) -> i32 {
        self.trade_goods.total_size() - self.trade_goods.total_transacted_size()
    }

    /// Calculates the total cost of trade goods in the manifest
    ///
    /// Returns the total purchase cost of all trade goods currently in the manifest.
    ///
    /// # Returns
    ///
    /// Total cost of trade goods in credits
    pub fn trade_goods_cost(&self) -> i64 {
        self.trade_goods.total_buy_cost() as i64
    }

    /// Calculates the total potential proceeds from trade goods in the manifest
    ///
    /// Returns the total potential selling value of all trade goods currently
    /// in the manifest, based on planned sell amounts if available.
    ///
    /// # Returns
    ///
    /// Total potential proceeds from trade goods in credits
    pub fn trade_goods_proceeds(&self) -> i64 {
        self.trade_goods.total_sell_cost() as i64
    }

    pub fn zero_transacted(&mut self) {
        self.trade_goods.zero_transacted();
    }

    /// Reset passengers and freight selections, preserving trade goods and sell plans
    pub fn reset_passengers_and_freight(&mut self) {
        self.high_passengers = 0;
        self.medium_passengers = 0;
        self.basic_passengers = 0;
        self.low_passengers = 0;
        self.freight_lot_indices.clear();
        self.zero_transacted();
    }

    /// Process trades: add current Total to profit and clear passenger/freight counts and sell plans
    /// Does NOT clear trade_goods quantities (tons) or list; only resets sell_plan to 0 and passenger/freight
    pub fn process_trades(&mut self, distance: i32, buy_goods: &[Good]) {
        // Compute current totals
        let passenger_revenue = self.passenger_revenue(distance) as i64;
        let freight_revenue = self.freight_revenue(distance) as i64;
        let goods_profit = self.trade_goods_proceeds()
            - buy_goods
                .iter()
                .map(|g| g.transacted as i64 * g.buy_cost as i64)
                .sum::<i64>();

        // Remove sold quantities from the manifest.
        self.trade_goods.process_trades();

        // Add any goods that were bought to the manifest.
        let new_goods = buy_goods.iter().filter_map(|g| {
            if g.transacted <= 0 {
                None
            } else {
                let mut new_good = g.clone();
                new_good.quantity = g.transacted;
                new_good.transacted = 0;
                Some(new_good)
            }
        });

        for good in new_goods {
            self.trade_goods.add_good(good);
        }

        // Clear the sell plan.
        self.zero_transacted();

        // Compute total revenue
        let total = passenger_revenue + freight_revenue + goods_profit;

        debug!("Processing trades: passenger_revenue={passenger_revenue}, freight_revenue={freight_revenue}, goods_profit={goods_profit}, total={total}");
        // Add to accumulated profit
        self.profit += total;

        // Reset passengers, freight, and drop the cost expended for future trades.
        self.reset_passengers_and_freight();
    }
}
