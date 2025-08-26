//! # Available Goods Module
//! 
//! This module handles the generation and management of trade goods available for purchase
//! at specific worlds in the Traveller universe. It provides functionality for creating
//! realistic trade markets based on world characteristics, population, trade classifications,
//! and broker skills.
//! 
//! ## Key Features
//! 
//! - **Dynamic Market Generation**: Creates available goods based on world trade classes
//! - **Population-Based Availability**: Adjusts quantities based on world population
//! - **Broker Skill Integration**: Modifies prices based on buyer/seller broker skills
//! - **Trade Classification Support**: Respects world trade class restrictions
//! - **Illegal Goods Handling**: Optional inclusion of restricted/illegal items
//! - **Price Fluctuation**: Realistic price variations based on supply/demand
//! 
//! ## Market Mechanics
//! 
//! The system generates markets through several phases:
//! 1. **Base Availability**: Goods available based on world trade classes
//! 2. **Random Goods**: Additional items based on population rolls
//! 3. **Quantity Generation**: Dice-based quantity determination with population modifiers
//! 4. **Price Calculation**: Broker skills and trade DMs affect final pricing
//! 5. **Market Sorting**: Goods can be sorted by discount percentage
//! 
//! ## Usage Examples
//! 
//! ```rust
//! use worldgen::trade::{available_goods::AvailableGoodsTable, TradeClass};
//! 
//! // Generate market for an agricultural world
//! let trade_classes = vec![TradeClass::Agricultural, TradeClass::Rich];
//! let mut market = AvailableGoodsTable::for_world(
//!     &trade_table, 
//!     &trade_classes, 
//!     7,     // Population 7
//!     false  // No illegal goods
//! )?;
//! 
//! // Apply broker skills and price goods
//! market.price_goods_to_buy(&trade_classes, 2, 1); // Buyer skill 2, seller skill 1
//! market.sort_by_discount(); // Sort by best deals first
//! ```

use rand::Rng;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

use crate::trade::table::{Availability, TradeTable, TradeTableEntry};
use crate::trade::TradeClass;

/// A trade good available for purchase at a specific world
/// 
/// Represents an individual commodity or manufactured good that can be purchased
/// from a world's market. Contains all necessary information for trade calculations
/// including quantities, pricing, and source trade table data.
/// 
/// ## Pricing Mechanics
/// 
/// - **Base Cost**: Original price from trade tables
/// - **Current Cost**: Modified price after broker skills and market conditions
/// - **Sell Price**: Optional price for selling at destination (calculated separately)
/// 
/// ## Quantity Tracking
/// 
/// - **Available Quantity**: Total amount available for purchase
/// - **Purchased Quantity**: Amount already bought (for manifest tracking)
/// 
/// ## Trade Integration
/// 
/// Each good maintains a reference to its source trade table entry, allowing
/// access to trade DMs, availability restrictions, and other metadata needed
/// for advanced trade calculations.
#[derive(Debug, Clone)]
pub struct AvailableGood {
    /// Name of the good
    pub name: String,
    /// Available quantity
    pub quantity: i32,
    /// Purchased quantity
    pub purchased: i32,
    /// Original base cost of the good
    pub base_cost: i32,
    /// Current cost of the good (after pricing)
    pub cost: i32,
    pub sell_price: Option<i32>,
    /// Original trade table entry this good was derived from
    pub source_entry: TradeTableEntry,
}

impl Display for AvailableGood {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let discount_percent = (self.cost as f64 / self.base_cost as f64 * 100.0).round();
        write!(
            f,
            "{}: {} @ {} ({}% of base)",
            self.name, self.quantity, self.cost, discount_percent as i32
        )
    }
}

/// Collection of goods available for purchase at a specific world
/// 
/// Manages the complete trade market for a world, including generation of available
/// goods based on world characteristics, pricing based on broker skills, and
/// market manipulation functions for trade calculations.
/// 
/// ## Market Generation
/// 
/// Markets are generated through a multi-step process:
/// 1. **Trade Class Goods**: Items available due to world's trade classifications
/// 2. **Population Rolls**: Random goods based on population-driven dice rolls
/// 3. **Quantity Calculation**: Dice-based quantities with population modifiers
/// 4. **Availability Filtering**: Respects legal restrictions and trade class limits
/// 
/// ## Price Dynamics
/// 
/// The table supports sophisticated pricing through:
/// - Broker skill differentials between buyer and seller
/// - Trade classification DMs (Difficulty Modifiers)
/// - Random market fluctuations via dice rolls
/// - Separate buy/sell price calculations for different destinations
/// 
/// ## Market Operations
/// 
/// - **Sorting**: Goods can be sorted by discount percentage
/// - **Lookup**: Fast access to specific goods by trade table index
/// - **Display**: Human-readable market summaries with pricing information
#[derive(Debug, Clone, Default)]
pub struct AvailableGoodsTable {
    /// List of available goods
    pub goods: Vec<AvailableGood>,
}

impl Display for AvailableGoodsTable {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.goods.is_empty() {
            writeln!(f, "No goods available")
        } else {
            for good in &self.goods {
                writeln!(f, "{good}")?;
            }
            Ok(())
        }
    }
}

impl AvailableGoodsTable {
    /// Create a new empty table
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a complete market for a specific world
    /// 
    /// Creates an available goods table based on the world's characteristics,
    /// including trade classifications, population level, and legal restrictions.
    /// 
    /// ## Generation Process
    /// 
    /// 1. **Trade Class Filtering**: Adds goods available to world's trade classes
    /// 2. **Population Rolls**: Makes population-based random rolls for additional goods
    /// 3. **Quantity Generation**: Rolls dice for quantities with population modifiers
    /// 4. **Duplicate Handling**: Combines quantities for duplicate goods
    /// 
    /// ## Parameters
    /// 
    /// * `trade_table` - Master trade table containing all possible goods
    /// * `world_trade_classes` - Trade classifications for this world
    /// * `population` - World population code (affects quantity and variety)
    /// * `illegal_ok` - Whether to include illegal/restricted goods (indices 61-66)
    /// 
    /// ## Population Effects
    /// 
    /// - **Low Population (≤3)**: -3 to all quantities, fewer random goods
    /// - **High Population (≥9)**: +3 to all quantities, more random goods
    /// - **Roll Count**: Makes `population` number of random good rolls
    /// 
    /// ## Returns
    /// 
    /// `Result<Self, String>` - Complete market table or error message
    /// 
    /// ## Examples
    /// 
    /// ```rust
    /// // Generate market for agricultural world, population 6, no illegal goods
    /// let market = AvailableGoodsTable::for_world(
    ///     &trade_table,
    ///     &[TradeClass::Agricultural, TradeClass::Rich],
    ///     6,
    ///     false
    /// )?;
    /// ```
    pub fn for_world(
        trade_table: &TradeTable,
        world_trade_classes: &[TradeClass],
        population: i32,
        illegal_ok: bool,
    ) -> Result<Self, String> {
        let mut table = Self::new();

        // Add goods based on trade classes
        for entry in trade_table.entries() {
            // Check if the entry is available based on world's trade classes
            let available = match &entry.availability {
                Availability::All => true,
                Availability::List(classes) => classes
                    .iter()
                    .any(|tc| world_trade_classes.contains(tc) && (illegal_ok || entry.index < 60)),
            };

            if available {
                table.add_entry(entry.clone(), population)?;
            }
        }

        // Add random goods based on population
        let mut rng = rand::rng();
        let max_tens = if illegal_ok { 6 } else { 5 };

        for _ in 0..population {
            // Roll 2d6 for the index
            let tens = rng.random_range(1..=max_tens);
            let ones = rng.random_range(1..=6);
            let index = tens * 10 + ones;

            if let Some(entry) = trade_table.get(index) {
                table.add_entry_rng(entry.clone(), &mut rng, population)?;
            }
        }

        Ok(table)
    }

    /// Add a trade table entry using provided random number generator
    /// 
    /// Internal method for adding goods to the market with explicit RNG control.
    /// Handles quantity generation, population modifiers, and duplicate consolidation.
    /// 
    /// ## Quantity Calculation
    /// 
    /// 1. **Base Roll**: Rolls specified number of d6 dice
    /// 2. **Multiplier**: Applies entry's quantity multiplier
    /// 3. **Population Modifier**: Adjusts based on world population
    ///    - Population ≤3: -3 to quantity
    ///    - Population ≥9: +3 to quantity
    ///    - Population 4-8: No modifier
    /// 
    /// ## Duplicate Handling
    /// 
    /// If a good with the same trade table index already exists in the market,
    /// the new quantity is added to the existing entry rather than creating
    /// a duplicate listing.
    /// 
    /// ## Parameters
    /// 
    /// * `entry` - Trade table entry to add
    /// * `rng` - Random number generator for quantity rolls
    /// * `world_population` - Population code for quantity modifiers
    /// 
    /// ## Returns
    /// 
    /// `Result<(), String>` - Success or error message
    fn add_entry_rng(
        &mut self,
        entry: TradeTableEntry,
        rng: &mut impl Rng,
        world_population: i32,
    ) -> Result<(), String> {
        // Roll for quantity
        let dice_count: i32 = entry.quantity.dice as i32;
        let multiplier: i32 = entry.quantity.multiplier as i32;

        let mut total = 0i32;
        for _ in 0..dice_count {
            total += rng.random_range(1..=6);
        }
        let mut quantity = total * multiplier;

        quantity += if world_population <= 3 {
            -3
        } else if world_population >= 9 {
            3
        } else {
            0
        };

        // If the good is already in the table, add to its quantity
        if let Some(existing) = self
            .goods
            .iter_mut()
            .find(|g| g.source_entry.index == entry.index)
        {
            existing.quantity += quantity;
            return Ok(());
        }

        // Otherwise, create a new entry
        let good = AvailableGood {
            name: entry.name.clone(),
            quantity,
            purchased: 0,
            base_cost: entry.base_cost,
            cost: entry.base_cost,
            sell_price: None,
            source_entry: entry,
        };

        self.goods.push(good);
        Ok(())
    }

    /// Add a trade table entry to the available goods
    fn add_entry(&mut self, entry: TradeTableEntry, world_population: i32) -> Result<(), String> {
        let mut rng = rand::rng();
        self.add_entry_rng(entry, &mut rng, world_population)
    }

    /// Get a specific good by its index
    pub fn get_by_index(&self, index: i16) -> Option<&AvailableGood> {
        self.goods.iter().find(|g| g.source_entry.index == index)
    }

    /// Get all available goods
    pub fn goods(&self) -> &[AvailableGood] {
        &self.goods
    }

    /// Get the number of different goods available
    pub fn len(&self) -> usize {
        self.goods.len()
    }

    /// Check if there are no goods available
    pub fn is_empty(&self) -> bool {
        self.goods.is_empty()
    }

    /// Calculate purchase prices based on broker skills and trade conditions
    /// 
    /// Adjusts the cost of all goods in the market based on the relative broker
    /// skills of buyer and seller, plus trade classification bonuses/penalties.
    /// Uses random rolls to simulate market fluctuations.
    /// 
    /// ## Price Calculation Formula
    /// 
    /// ```text
    /// Modified Roll = 3d6 + Buyer Broker - Seller Broker + Purchase DM - Sale DM
    /// Final Cost = Base Cost × Price Multiplier (based on Modified Roll)
    /// ```
    /// 
    /// ## Price Multiplier Table
    /// 
    /// | Modified Roll | Multiplier | Discount |
    /// |---------------|------------|----------|
    /// | ≤-3          | 300%       | -200%    |
    /// | -2           | 250%       | -150%    |
    /// | -1           | 200%       | -100%    |
    /// | 0            | 175%       | -75%     |
    /// | 1            | 150%       | -50%     |
    /// | 2            | 135%       | -35%     |
    /// | 3            | 125%       | -25%     |
    /// | 4-15         | 100%       | Base     |
    /// | 16           | 90%        | 10%      |
    /// | 17           | 80%        | 20%      |
    /// | 18           | 70%        | 30%      |
    /// | 19           | 45%        | 55%      |
    /// | 20           | 40%        | 60%      |
    /// | 21           | 35%        | 65%      |
    /// | 22           | 30%        | 70%      |
    /// | 23           | 25%        | 75%      |
    /// | 24           | 20%        | 80%      |
    /// | ≥25          | 15%        | 85%      |
    /// 
    /// ## Trade DMs
    /// 
    /// - **Purchase DM**: Bonus when world produces this good type
    /// - **Sale DM**: Penalty when world consumes this good type
    /// - **Net Effect**: Purchase DM - Sale DM affects final price
    /// 
    /// ## Parameters
    /// 
    /// * `origin_trade_classes` - Trade classes of the selling world
    /// * `buyer_broker_skill` - Buyer's broker skill level
    /// * `supplier_broker_skill` - Seller's broker skill level
    /// 
    /// ## Examples
    /// 
    /// ```rust
    /// // Skilled buyer (3) vs average seller (1) on agricultural world
    /// market.price_goods_to_buy(&[TradeClass::Agricultural], 3, 1);
    /// // Expect better prices due to +2 skill differential
    /// ```
    pub fn price_goods_to_buy(
        &mut self,
        origin_trade_classes: &[TradeClass],
        buyer_broker_skill: i16,
        supplier_broker_skill: i16,
    ) {
        let mut rng = rand::rng();
        for good in &mut self.goods {
            // Roll 2d6
            let roll = rng.random_range(1..=6) + rng.random_range(1..=6) + rng.random_range(1..=6);

            let best_purchase_origin_dm =
                find_total_dm(&good.source_entry.purchase_dm, origin_trade_classes);
            let best_sale_origin_dm =
                find_total_dm(&good.source_entry.sale_dm, origin_trade_classes);
            // Calculate the modified roll
            let modified_roll = roll as i16 + buyer_broker_skill - supplier_broker_skill
                + best_purchase_origin_dm
                - best_sale_origin_dm;

            // Determine the price multiplier based on the modified roll
            let price_multiplier = match modified_roll {
                i16::MIN..=-3 => 3.0, // 300%
                -2 => 2.5,            // 250%
                -1 => 2.0,            // 200%
                0 => 1.75,            // 175%
                1 => 1.5,             // 150%
                2 => 1.35,            // 135%
                3 => 1.25,            // 125%
                4 => 1.2,             // 120%
                5 => 1.15,            // 115%
                6 => 1.1,             // 110%
                7 => 1.05,            // 105%
                8 => 1.0,             // 100%
                9 => 0.95,            // 95%
                10 => 0.9,            // 90%
                11 => 0.85,           // 85%
                12 => 0.8,            // 80%
                13 => 0.75,           // 75%
                14 => 0.7,            // 70%
                15 => 0.65,           // 65%
                16 => 0.6,            // 60%
                17 => 0.55,           // 55%
                18 => 0.5,            // 50%
                19 => 0.45,           // 45%
                20 => 0.4,            // 40%
                21 => 0.35,           // 35%
                22 => 0.3,            // 30%
                23 => 0.25,           // 25%
                24 => 0.2,            // 20%
                25.. => 0.15,         // 15%
            };

            /* console_log(
                format!(
                    "Pricing {}: Roll {roll}, Modified roll: {modified_roll}, Price multiplier: {}",
                    good.name, price_multiplier
                )
                .as_str(),
            );
            */

            // Apply the multiplier to the cost
            good.cost = (good.base_cost as f64 * price_multiplier).round() as i32;
        }
    }

    /// Calculate selling prices for goods at potential destination worlds
    /// 
    /// Determines the selling price for goods when transported to worlds with
    /// specific trade classifications. Uses the same pricing mechanics as
    /// purchase pricing but from the seller's perspective.
    /// 
    /// ## Destination Pricing
    /// 
    /// When destination trade classes are provided:
    /// - Calculates modified roll using destination world's trade DMs
    /// - Applies same price multiplier table as purchase pricing
    /// - Sets `sell_price` field for each good
    /// 
    /// When no destination is specified:
    /// - Clears all `sell_price` fields (sets to `None`)
    /// - Used for markets where destination is unknown
    /// 
    /// ## Trade DM Application
    /// 
    /// For selling, the DM calculation is reversed:
    /// - **Purchase DM**: Applied as bonus (destination wants this good)
    /// - **Sale DM**: Applied as penalty (destination produces this good)
    /// 
    /// ## Parameters
    /// 
    /// * `possible_destination_trade_classes` - Trade classes of destination world(s)
    /// * `buyer_broker_skill` - Destination buyer's broker skill
    /// * `supplier_broker_skill` - Current seller's broker skill
    /// 
    /// ## Examples
    /// 
    /// ```rust
    /// // Calculate selling prices for industrial destination
    /// market.price_goods_to_sell(
    ///     Some(vec![TradeClass::Industrial, TradeClass::HighTech]),
    ///     1, // Destination buyer skill
    ///     2  // Our broker skill
    /// );
    /// 
    /// // Clear selling prices (no destination selected)
    /// market.price_goods_to_sell(None, 0, 0);
    /// ```
    pub fn price_goods_to_sell(
        &mut self,
        possible_destination_trade_classes: Option<Vec<TradeClass>>,
        buyer_broker_skill: i16,
        supplier_broker_skill: i16,
    ) {
        let mut rng = rand::rng();
        for good in &mut self.goods {
            if let Some(destination_trade_classes) = &possible_destination_trade_classes {
                // Roll 2d6
                let roll =
                    rng.random_range(1..=6) + rng.random_range(1..=6) + rng.random_range(1..=6);

                let best_purchase_origin_dm =
                    find_total_dm(&good.source_entry.purchase_dm, destination_trade_classes);
                let best_sale_origin_dm =
                    find_total_dm(&good.source_entry.sale_dm, destination_trade_classes);

                // Calculate the modified roll
                let modified_roll = roll as i16 - buyer_broker_skill + supplier_broker_skill
                    - best_purchase_origin_dm
                    + best_sale_origin_dm;

                // Determine the price multiplier based on the modified roll
                let price_multiplier = match modified_roll {
                    i16::MIN..=-3 => 0.1,
                    -2 => 0.2,
                    -1 => 0.3,
                    0 => 0.4, // 175%
                    1 => 0.45,
                    2 => 0.5,
                    3 => 0.55,
                    4 => 0.60,
                    5 => 0.65,
                    6 => 0.70,
                    7 => 0.75,
                    8 => 0.80,
                    9 => 0.85,
                    10 => 0.9,
                    11 => 1.0,
                    12 => 1.05,
                    13 => 1.10,
                    14 => 1.15,
                    15 => 1.20,
                    16 => 1.25,
                    17 => 1.30,
                    18 => 1.40,
                    19 => 1.50,
                    20 => 1.60,
                    21 => 1.75,
                    22 => 2.0,
                    23 => 2.5,
                    24 => 3.0,
                    25.. => 4.0,
                };

                // Apply the multiplier to the cost
                good.sell_price = Some((good.base_cost as f64 * price_multiplier).round() as i32);
            } else {
                good.sell_price = None;
            }
        }
    }

    /// Sort goods from most discounted to least discounted
    pub fn sort_by_discount(&mut self) {
        self.goods.sort_by(|a, b| {
            // Calculate discount percentage for each good
            let a_discount = a.cost as f64 / a.base_cost as f64;
            let b_discount = b.cost as f64 / b.base_cost as f64;

            // Sort from lowest ratio (biggest discount) to highest ratio (smallest discount)
            a_discount
                .partial_cmp(&b_discount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Calculate total trade DMs for a set of world trade classes
/// 
/// Sums all applicable Difficulty Modifiers (DMs) from a trade good's DM map
/// that match the world's trade classifications. This determines the total
/// bonus or penalty applied to trade rolls for this good at this world.
/// 
/// ## DM Accumulation
/// 
/// Unlike some systems that take the best single DM, this function sums
/// all applicable DMs, allowing worlds with multiple relevant trade classes
/// to receive cumulative bonuses.
/// 
/// ## Parameters
/// 
/// * `dm_map` - HashMap mapping trade classes to their DM values
/// * `world_trade_classes` - Trade classifications of the world
/// 
/// ## Returns
/// 
/// `i16` - Total DM (sum of all applicable modifiers)
/// 
/// ## Examples
/// 
/// ```rust
/// // Agricultural world (+2) that's also Rich (+1) for electronics
/// let total_dm = find_total_dm(&electronics_purchase_dm, &[Agricultural, Rich]);
/// // Returns: 3 (2 + 1)
/// 
/// // Industrial world with no applicable DMs for agricultural products  
/// let total_dm = find_total_dm(&ag_products_purchase_dm, &[Industrial]);
/// // Returns: 0
/// ```
fn find_total_dm(
    dm_map: &std::collections::HashMap<TradeClass, i16>,
    world_trade_classes: &[TradeClass],
) -> i16 {
    let mut total_dm: i16 = 0;

    for trade_class in world_trade_classes {
        if let Some(&dm) = dm_map.get(trade_class) {
            total_dm += dm;
        }
    }

    total_dm
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::table::{Availability, Quantity};
    use crate::trade::TradeClass;

    use std::collections::HashMap;

    #[test_log::test]
    fn test_available_goods_table() {
        // Create a standard trade table
        let trade_table = TradeTable::default();

        // Create a world with Agricultural and Rich trade classes
        let world_trade_classes = vec![TradeClass::Agricultural, TradeClass::Rich];

        // Create an available goods table for the world
        let available_goods = AvailableGoodsTable::for_world(
            &trade_table,
            &world_trade_classes,
            5,     // Population 5
            false, // No illegal goods
        )
        .expect("Failed to create available goods table");

        // Verify that the table is not empty
        assert!(!available_goods.is_empty());

        // Check that all common goods are available (they have Availability::All)
        for i in 11..=16 {
            assert!(
                available_goods.get_by_index(i).is_some(),
                "Common good {i} should be available",
            );
        }

        // Check that agricultural goods are available
        let ag_goods = [33, 34, 52, 55]; // Some goods available for Agricultural worlds
        for i in ag_goods.iter() {
            assert!(
                available_goods.get_by_index(*i).is_some(),
                "Agricultural good {i} should be available",
            );
        }

        // Check that no illegal goods are available when illegal_ok is false
        for i in 61..=66 {
            assert!(
                available_goods.get_by_index(i).is_none(),
                "Illegal good {i} should not be available",
            );
        }

        // Check that DMs are correctly calculated
        // Common Electronics (11) has purchase DM for Rich+1
        let electronics = available_goods.get_by_index(11).unwrap();
        assert_eq!(
            find_total_dm(&electronics.source_entry.purchase_dm, &world_trade_classes),
            1
        );

        // Agricultural Products (33) has purchase DM for Agricultural+2
        let ag_products = available_goods.get_by_index(33).unwrap();
        assert_eq!(
            find_total_dm(&ag_products.source_entry.purchase_dm, &world_trade_classes),
            2
        );

        // Create another table with illegal goods allowed
        let available_goods_with_illegal = AvailableGoodsTable::for_world(
            &trade_table,
            &world_trade_classes,
            5,    // Population 5
            true, // Allow illegal goods
        )
        .expect("Failed to create available goods table with illegal goods");

        // Note: We can't deterministically test for the presence of illegal goods
        // since they're randomly selected, but we can check that the table was created
        assert!(!available_goods_with_illegal.is_empty());
    }

    #[test_log::test]
    fn test_find_best_dm() {
        let mut dm_map = std::collections::HashMap::new();
        dm_map.insert(TradeClass::Agricultural, 3);
        dm_map.insert(TradeClass::Rich, 5);
        dm_map.insert(TradeClass::HighTech, 2);

        // World with Agricultural and Rich trade classes
        let world_trade_classes = vec![TradeClass::Agricultural, TradeClass::Rich];

        // Rich should be the best DM
        let best_dm = find_total_dm(&dm_map, &world_trade_classes);
        assert_eq!(best_dm, 5);

        // World with only Agricultural trade class
        let world_trade_classes = vec![TradeClass::Agricultural];

        // Agricultural should be the best (and only) DM
        let best_dm = find_total_dm(&dm_map, &world_trade_classes);
        assert_eq!(best_dm, 3);

        // World with no matching trade classes
        let world_trade_classes = vec![TradeClass::Industrial, TradeClass::Poor];

        // No DM should be found
        let best_dm = find_total_dm(&dm_map, &world_trade_classes);
        assert_eq!(best_dm, 0);
    }

    #[test_log::test]
    fn test_price_goods() {
        // Create a simple trade table entry for testing
        let mut purchase_dm = HashMap::new();
        purchase_dm.insert(TradeClass::Rich, 2);

        let mut sale_dm = HashMap::new();
        sale_dm.insert(TradeClass::Agricultural, 3);

        let entry = TradeTableEntry {
            index: 1,
            name: "Test Good".to_string(),
            availability: Availability::All,
            quantity: Quantity {
                dice: 2,
                multiplier: 1,
            },
            base_cost: 10000,
            purchase_dm,
            sale_dm,
        };

        // Create a world with both trade classes
        let world_trade_classes = vec![TradeClass::Rich, TradeClass::Agricultural];

        // Create a table with a single good
        let mut table = AvailableGoodsTable::new();
        table.add_entry(entry.clone(), 5).unwrap();

        // Get the original cost
        let original_cost = table.goods()[0].cost;

        // Price the goods with equal broker skills
        // This will use a random roll, so we can't predict the exact price,
        // but we can check that the price has changed
        table.price_goods_to_buy(&world_trade_classes, 0, 0);

        // The price should be different due to the DMs (purchase +2, sale -3)
        // and the random roll
        assert_ne!(table.goods()[0].cost, original_cost);

        // Create another table for a more controlled test
        let mut table2 = AvailableGoodsTable::new();
        table2.add_entry(entry.clone(), 5).unwrap();

        // Set up a test where we know the outcome
        // If buyer has skill 3, supplier has skill 1, purchase DM is +2, sale DM is +3
        // Then the modifier is: +3 - 1 + 2 - 3 = +1
        // For a roll of 7, the modified roll would be 7 + 1 = 8, which is 100%
        // We can't control the roll, but we can check that the price is within
        // a reasonable range based on the skills and DMs
        table2.price_goods_to_buy(&world_trade_classes, 3, 1);

        // The price should be affected by the skills and DMs
        // We can't assert an exact value due to the random roll
        let new_cost = table2.goods()[0].cost;
        println!("Original cost: {original_cost}, New cost: {new_cost}");

        // The price should be within a reasonable range
        // With the given skills and DMs, the price multiplier should be between 0.5 and 2.0
        // depending on the roll
        assert!(new_cost >= (original_cost as f64 * 0.5) as i32);
        assert!(new_cost <= (original_cost as f64 * 2.0) as i32);
    }

    #[test_log::test]
    fn test_display() {
        // Create a simple good
        let good = AvailableGood {
            name: "Test Good".to_string(),
            quantity: 10,
            base_cost: 5000,
            cost: 5000,
            purchased: 0,
            sell_price: None,
            source_entry: TradeTableEntry {
                index: 1,
                name: "Test Good".to_string(),
                availability: Availability::All,
                quantity: Quantity {
                    dice: 2,
                    multiplier: 1,
                },
                base_cost: 5000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
        };

        // Check the display output
        assert_eq!(format!("{good}"), "Test Good: 10 @ 5000 (100% of base)");

        // Create a table with a single good
        let mut table = AvailableGoodsTable::new();
        table.goods.push(good);

        // Check the display output
        let expected = "Test Good: 10 @ 5000 (100% of base)\n";
        assert_eq!(format!("{table}"), expected);

        // Create an empty table
        let empty_table = AvailableGoodsTable::new();

        // Check the display output
        assert_eq!(format!("{empty_table}"), "No goods available\n");
    }

    // Test to test end to end by creating a TradeTable from the standard table
    // and then creating an AvailableGoodsTable from it, and then printing out
    // the available goods
    #[test_log::test]
    fn test_end_to_end() {
        // Create a standard trade table
        let trade_table = TradeTable::default();

        // Create a world with Agricultural and Rich trade classes
        let world_trade_classes = vec![TradeClass::Agricultural, TradeClass::Rich];

        // Create an available goods table for the world
        let mut available_goods = AvailableGoodsTable::for_world(
            &trade_table,
            &world_trade_classes,
            5,     // Population 5
            false, // No illegal goods
        )
        .expect("Failed to create available goods table");

        // Price the goods with equal broker skills
        available_goods.price_goods_to_buy(&world_trade_classes, 0, 0);

        // Sort by discount
        available_goods.sort_by_discount();

        // Print out the available goods
        println!("{available_goods}");
    }

    #[test_log::test]
    fn test_sort_by_discount() {
        // Create a table with multiple goods at different discount levels
        let mut table = AvailableGoodsTable::new();

        // Add goods with different discounts
        let good1 = AvailableGood {
            name: "Good 1".to_string(),
            quantity: 10,
            base_cost: 10000,
            cost: 5000, // 50% of base
            purchased: 0,
            sell_price: None,
            source_entry: TradeTableEntry {
                index: 1,
                name: "Good 1".to_string(),
                availability: Availability::All,
                quantity: Quantity {
                    dice: 2,
                    multiplier: 1,
                },
                base_cost: 10000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
        };

        let good2 = AvailableGood {
            name: "Good 2".to_string(),
            quantity: 10,
            base_cost: 10000,
            cost: 8000, // 80% of base
            purchased: 0,
            sell_price: None,
            source_entry: TradeTableEntry {
                index: 2,
                name: "Good 2".to_string(),
                availability: Availability::All,
                quantity: Quantity {
                    dice: 2,
                    multiplier: 1,
                },
                base_cost: 10000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
        };

        let good3 = AvailableGood {
            name: "Good 3".to_string(),
            quantity: 10,
            base_cost: 10000,
            cost: 2000, // 20% of base
            purchased: 0,
            sell_price: None,
            source_entry: TradeTableEntry {
                index: 3,
                name: "Good 3".to_string(),
                availability: Availability::All,
                quantity: Quantity {
                    dice: 2,
                    multiplier: 1,
                },
                base_cost: 10000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
        };

        // Add goods in random order
        table.goods.push(good1);
        table.goods.push(good2);
        table.goods.push(good3);

        // Sort by discount
        table.sort_by_discount();

        // Check that goods are sorted from most discounted to least discounted
        assert_eq!(table.goods[0].name, "Good 3"); // 20% of base
        assert_eq!(table.goods[1].name, "Good 1"); // 50% of base
        assert_eq!(table.goods[2].name, "Good 2"); // 80% of base

        // Print the sorted table
        println!("Sorted table:\n{table}");
    }

    #[test_log::test]
    fn test_price_goods_to_sell() {
        // Create a simple trade table entry for testing
        let mut purchase_dm = HashMap::new();
        purchase_dm.insert(TradeClass::Rich, 2);

        let mut sale_dm = HashMap::new();
        sale_dm.insert(TradeClass::Agricultural, 3);

        let entry = TradeTableEntry {
            index: 1,
            name: "Test Good".to_string(),
            availability: Availability::All,
            quantity: Quantity {
                dice: 2,
                multiplier: 1,
            },
            base_cost: 10000,
            purchase_dm,
            sale_dm,
        };

        // Create a table with a single good
        let mut table = AvailableGoodsTable::new();
        table.add_entry(entry.clone(), 5).unwrap();

        // Test with destination trade classes
        let destination_trade_classes = vec![TradeClass::Rich, TradeClass::Agricultural];

        // Price the goods for sale
        table.price_goods_to_sell(Some(destination_trade_classes.clone()), 0, 0);

        // The good should now have a sell price
        let good = &table.goods()[0];
        assert!(good.sell_price.is_some());

        // The sell price should be different from base cost due to DMs and random roll
        let sell_price = good.sell_price.unwrap();
        assert_ne!(sell_price, good.base_cost);

        // Test with no destination trade classes (None case)
        table.price_goods_to_sell(None, 0, 0);

        // The sell price should be None when no destination is provided
        let good = &table.goods()[0].clone();
        assert!(good.sell_price.is_none());

        // Test with different broker skills
        table.price_goods_to_sell(Some(destination_trade_classes), 3, 1);

        // The sell price should be affected by broker skills
        let new_sell_price = table.goods()[0].sell_price.unwrap();

        // With buyer skill 3 and supplier skill 1, the modifier is -3 + 1 = -2
        // This should generally result in lower sell prices
        // We can't assert exact values due to random rolls, but we can check it's reasonable
        assert!(new_sell_price >= (good.base_cost as f64 * 0.1) as i32);
        assert!(new_sell_price <= (good.base_cost as f64 * 4.0) as i32);
    }
}
