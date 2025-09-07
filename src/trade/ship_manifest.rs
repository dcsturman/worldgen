//! # Ship Manifest Module
//!
//! This module defines the ship manifest structure and revenue calculation
//! functionality for passenger and freight transport in the Traveller universe.
//!
//! The manifest tracks different classes of passengers, freight lots, and trade goods,
//! and calculates revenue based on distance traveled and passenger/freight types.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::trade::available_goods::AvailableGood;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::table::TradeTable;

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
    pub trade_goods: Vec<AvailableGood>,
    /// Planned sell amounts for goods (keyed by source_entry.index)
    pub sell_plan: HashMap<i16, i32>,
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
    /// use worldgen::trade::available_goods::AvailableGood;
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
    /// let good = AvailableGood {
    ///     name: "Test Good".to_string(),
    ///     quantity: 10,
    ///     purchased: 0,
    ///     base_cost: 1000,
    ///     buy_cost: 1000,
    ///     buy_cost_comment: String::new(),
    ///     sell_price: None,
    ///     sell_price_comment: String::new(),
    ///     source_index: entry.index,
    /// };
    ///
    /// // Add a good with quantity 5
    /// manifest.update_trade_good(&good, 5);
    /// // Update the same good to quantity 3
    /// manifest.update_trade_good(&good, 3);
    /// // Remove the good by setting quantity to 0
    /// manifest.update_trade_good(&good, 0);
    /// ```
    pub fn update_trade_good(&mut self, good: &AvailableGood, quantity: i32) {
        let index = good.source_index;
        // Find existing good by source entry index
        if let Some(pos) = self
            .trade_goods
            .iter()
            .position(|g| g.source_index == index)
        {
            if quantity <= 0 {
                // Remove the good if quantity is 0 or negative
                self.trade_goods.remove(pos);
                self.sell_plan.remove(&index);
            } else {
                // Update the existing good's quantity
                let mut updated_good = good.clone();
                updated_good.purchased = quantity;
                self.trade_goods[pos] = updated_good;
                // Ensure sell plan defaults to zero, and clamp to available if previously set
                let entry = self.sell_plan.entry(index).or_insert(0);
                *entry = (*entry).min(quantity).max(0);
            }
        } else if quantity > 0 {
            // Add new good if it doesn't exist and quantity > 0
            let mut new_good = good.clone();
            new_good.purchased = quantity;
            self.trade_goods.push(new_good);
            // Default sell plan to zero amount
            self.sell_plan.insert(index, 0);
        }
    }

    /// Sets the planned sell amount for a given good (clamped to [0, purchased])
    pub fn set_sell_amount(&mut self, good: &AvailableGood, amount: i32) {
        if let Some(existing) = self
            .trade_goods
            .iter()
            .find(|g| g.source_index == good.source_index)
        {
            let clamped = amount.clamp(0, existing.purchased);
            self.sell_plan.insert(good.source_index, clamped);
        }
    }

    /// Returns the planned sell amount for a given good (defaults to purchased)
    pub fn get_sell_amount(&self, good: &AvailableGood) -> i32 {
        let purchased = self
            .trade_goods
            .iter()
            .find(|g| g.source_index == good.source_index)
            .map(|g| g.purchased)
            .unwrap_or(0);
        self.sell_plan
            .get(&good.source_index)
            .copied()
            .unwrap_or(0)
            .min(purchased)
            .max(0)
    }

    /// Gets the quantity of a specific trade good in the manifest
    ///
    /// Returns the quantity of the specified good currently in the manifest,
    /// or 0 if the good is not in the manifest.
    pub fn get_trade_good_quantity(&self, good: &AvailableGood) -> i32 {
        self.get_trade_good_quantity_by_index(good.source_index)
    }

    /// Gets the quantity of a specific trade good by its trade table index
    pub fn get_trade_good_quantity_by_index(&self, index: i16) -> i32 {
        self.trade_goods
            .iter()
            .find(|g| g.source_index == index)
            .map(|g| g.purchased)
            .unwrap_or(0)
    }

    /// Removes a trade good from the manifest by index, if present
    pub fn remove_trade_good_by_index(&mut self, index: i16) {
        if let Some(pos) = self
            .trade_goods
            .iter()
            .position(|g| g.source_index == index)
        {
            self.trade_goods.remove(pos);
        }
        self.sell_plan.remove(&index);
    }

    /// Commits the planned sell amount for the given good index:
    /// subtracts the planned amount from the manifest (down to 0),
    /// removes the good if it reaches 0, and resets the sell plan to 0.
    pub fn commit_sale_by_index(&mut self, index: i16) {
        // Find the good
        if let Some(pos) = self
            .trade_goods
            .iter()
            .position(|g| g.source_index == index)
        {
            let sell_amt = self.get_sell_amount_by_index(index);
            if sell_amt <= 0 {
                // Nothing to sell; just ensure plan is 0
                self.sell_plan.insert(index, 0);
                return;
            }
            // Compute new quantity and update/remove
            let current_qty = self.trade_goods[pos].purchased;
            let new_qty = (current_qty - sell_amt).max(0);
            if new_qty == 0 {
                self.trade_goods.remove(pos);
            } else if let Some(g) = self.trade_goods.get_mut(pos) {
                g.purchased = new_qty;
            }
            // Reset plan
            self.sell_plan.insert(index, 0);
        } else {
            // Not in manifest; clear any stale plan
            self.sell_plan.remove(&index);
        }
    }

    /// Commits all planned sales across all goods in the manifest
    pub fn commit_all_sales(&mut self) {
        // Collect indices first to avoid borrow issues while mutating
        let indices: Vec<i16> = self.trade_goods.iter().map(|g| g.source_index).collect();
        for idx in indices {
            self.commit_sale_by_index(idx);
        }
    }

    /// Calculates the total tonnage of trade goods in the manifest
    ///
    /// Returns the sum of all trade good quantities currently in the manifest.
    ///
    /// # Returns
    ///
    /// Total tonnage of trade goods
    pub fn trade_goods_tonnage(&self) -> i32 {
        self.trade_goods.iter().map(|g| g.purchased).sum()
    }

    /// Calculates the total cost of trade goods in the manifest
    ///
    /// Returns the total purchase cost of all trade goods currently in the manifest.
    ///
    /// # Returns
    ///
    /// Total cost of trade goods in credits
    pub fn trade_goods_cost(&self) -> i64 {
        self.trade_goods
            .iter()
            .map(|g| g.purchased as i64 * g.buy_cost as i64)
            .sum()
    }

    /// Zeros out the buy costs of all trade goods in the manifest
    pub fn zero_buy_costs(&mut self) {
        for good in self.trade_goods.iter_mut() {
            good.buy_cost = 0;
        }
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
        self.trade_goods
            .iter()
            .map(|g| {
                if let Some(sell_price) = g.sell_price {
                    let to_sell = self
                        .sell_plan
                        .get(&g.source_index)
                        .copied()
                        .unwrap_or(0)
                        .min(g.purchased)
                        .max(0);
                    to_sell as i64 * sell_price as i64
                } else {
                    0
                }
            })
            .sum()
    }

    /// Set sell amount by trade table index (non-negative; UI is responsible for clamping)
    pub fn set_sell_amount_by_index(&mut self, index: i16, amount: i32) {
        let clamped = amount.max(0);
        self.sell_plan.insert(index, clamped);
    }

    /// Get sell amount by trade table index (defaults to 0 if unset)
    pub fn get_sell_amount_by_index(&self, index: i16) -> i32 {
        self.sell_plan.get(&index).copied().unwrap_or(0)
    }

    /// Reset passengers and freight selections, preserving trade goods and sell plans
    pub fn reset_passengers_and_freight(&mut self) {
        self.high_passengers = 0;
        self.medium_passengers = 0;
        self.basic_passengers = 0;
        self.low_passengers = 0;
        self.freight_lot_indices.clear();
        self.sell_plan.clear();
    }

    /// Process trades: add current Total to profit and clear passenger/freight counts and sell plans
    /// Does NOT clear trade_goods quantities (tons) or list; only resets sell_plan to 0 and passenger/freight
    pub fn process_trades(&mut self, distance: i32, show_sell: bool) {
        // Compute current totals
        let passenger_revenue = self.passenger_revenue(distance) as i64;
        let freight_revenue = self.freight_revenue(distance) as i64;
        let goods_profit = if show_sell {
            let cost = self.trade_goods_cost();
            let proceeds = self.trade_goods_proceeds();
            proceeds - cost
        } else {
            0
        };

        // Subtract the value in each sell plan from the amount of goods in the manifest
        // Remove from trade goods if less than 0.
        for (index, amount) in self.sell_plan.iter() {
            if let Some(pos) = self
                .trade_goods
                .iter()
                .position(|g| g.source_index == *index)
            {
                let new_qty = self.trade_goods[pos].purchased - amount;
                self.trade_goods[pos].purchased = new_qty.max(0);
                if new_qty <= 0 {
                    self.trade_goods.remove(pos);
                }
            }
        }

        // Clear the sell plan.
        self.sell_plan.clear();

        // Compute total revenue
        let total = passenger_revenue + freight_revenue + goods_profit;
        // Add to accumulated profit
        self.profit += total;

        // Reset passengers, freight, and drop the cost expended for future trades.
        self.reset_passengers_and_freight();
        self.zero_buy_costs();
    }

    pub fn manifest_goods_list(&self) -> Vec<AvailableGood> {
        let manifest_goods: Vec<AvailableGood> = self
            .trade_goods
            .clone()
            .iter_mut()
            .map(|g| {
                g.quantity = 0;
                g.purchased = 0;
                g.buy_cost = g.base_cost;
                g.buy_cost_comment.clear();
                g.sell_price = None;
                g.sell_price_comment.clear();
                g.clone()
            })
            .collect();

        // These are goods where entered a plan to sell all we have in our manifest.
        // Thus they are missing from the trade_goods itself.
        let planned_goods: Vec<AvailableGood> = self
            .sell_plan
            .iter()
            .filter_map(|(idx, amt)| {
                if *amt > 0 && !manifest_goods.iter().any(|g| g.source_index == *idx) {
                    TradeTable::default().get(*idx).map(|entry| {
                        let mut g = AvailableGood {
                            name: entry.name.clone(),
                            quantity: 0,
                            purchased: 0,
                            base_cost: entry.base_cost,
                            buy_cost: entry.base_cost,
                            buy_cost_comment: String::new(),
                            sell_price: None,
                            sell_price_comment: String::new(),
                            source_index: entry.index,
                        };
                        g.purchased = *amt;
                        g
                    })
                } else {
                    None
                }
            })
            .collect();

        manifest_goods.into_iter().chain(planned_goods).collect()
    }
}
