use rand::Rng;
use std::fmt::{Display, Formatter, Result as FmtResult};
use leptos::leptos_dom::logging::console_log;
use log::debug;

use crate::{TradeClass, table::TradeTable, table::TradeTableEntry};

/// Represents a good available for purchase at a specific world
#[derive(Debug, Clone)]
pub struct AvailableGood {
    /// Name of the good
    pub name: String,
    /// Available quantity
    pub quantity: i32,
    /// Original base cost of the good
    pub base_cost: i32,
    /// Current cost of the good (after pricing)
    pub cost: i32,
    /// Original trade table entry this good was derived from
    pub source_entry: TradeTableEntry,
    /// Best purchase DM for this good on this world (if any)
    pub best_purchase_dm: i16,
    /// Best sale DM for this good on this world (if any)
    pub best_sale_dm: i16,
}

impl Display for AvailableGood {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let discount_percent = (self.cost as f64 / self.base_cost as f64 * 100.0).round();
        write!(f, "{}: {} @ {} ({}% of base)", 
            self.name, 
            self.quantity, 
            self.cost,
            discount_percent as i32
        )
    }
}

/// Table of goods available for purchase at a specific world
#[derive(Debug, Clone)]
pub struct AvailableGoodsTable {
    /// List of available goods
    goods: Vec<AvailableGood>,
}

impl Display for AvailableGoodsTable {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.goods.is_empty() {
            writeln!(f, "No goods available")
        } else {
            for good in &self.goods {
                writeln!(f, "{}", good)?;
            }
            Ok(())
        }
    }
}

impl AvailableGoodsTable {
    /// Create a new empty table
    pub fn new() -> Self {
        Self {
            goods: Vec::new(),
        }
    }
    
    /// Create a table of available goods for a specific world
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
                crate::table::Availability::All => true,
                crate::table::Availability::List(classes) => {
                    classes.iter().any(|tc| world_trade_classes.contains(tc) && (illegal_ok || entry.index < 60))
                }
            };
            
            if available {
                table.add_entry(entry.clone(), population, world_trade_classes)?;
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
                table.add_entry_with_rng(entry.clone(), &mut rng, population, world_trade_classes)?;
            }
        }
        
        Ok(table)
    }
    
    /// Add a trade table entry to the available goods using the provided RNG
    fn add_entry_with_rng(
        &mut self, 
        entry: TradeTableEntry, 
        rng: &mut impl Rng,
        world_population: i32,
        world_trade_classes: &[TradeClass],
    ) -> Result<(), String> {
        // Roll for quantity
        let dice_count = entry.quantity.dice;
        let multiplier = entry.quantity.multiplier;
        
        // Reduce dice count by 3 for low population worlds (but never below 1)
        let adjusted_dice_count = if world_population <= 3 {
            dice_count.saturating_sub(3).max(1)
        } else {
            dice_count
        };
        
        let mut total = 0;
        for _ in 0..adjusted_dice_count {
            total += rng.random_range(1..=6);
        }
        let quantity = total as i32 * multiplier as i32;
        
        // If the good is already in the table, add to its quantity
        if let Some(existing) = self.goods.iter_mut().find(|g| g.source_entry.index == entry.index) {
            existing.quantity += quantity;
            return Ok(());
        }
        
        // Find the best purchase DM for this world's trade classes
        let best_purchase_dm = find_best_dm(&entry.purchase_dm, world_trade_classes);
        
        // Find the best sale DM for this world's trade classes
        let best_sale_dm = find_best_dm(&entry.sale_dm, world_trade_classes);
        
        // Otherwise, create a new entry
        let good = AvailableGood {
            name: entry.name.clone(),
            quantity,
            base_cost: entry.base_cost,
            cost: entry.base_cost,
            source_entry: entry,
            best_purchase_dm,
            best_sale_dm,
        };
        
        self.goods.push(good);
        Ok(())
    }
    
    /// Add a trade table entry to the available goods
    fn add_entry(
        &mut self, 
        entry: TradeTableEntry, 
        world_population: i32,
        world_trade_classes: &[TradeClass]
    ) -> Result<(), String> {
        let mut rng = rand::rng();
        self.add_entry_with_rng(entry, &mut rng, world_population, world_trade_classes)
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

    /// Adjust the prices of goods based on broker skills and trade DMs
    pub fn price_goods(&mut self, buyer_broker_skill: i16, supplier_broker_skill: i16) {
        let mut rng = rand::rng();
        for good in &mut self.goods {
            // Roll 2d6
            let roll = rng.random_range(1..=6) + rng.random_range(1..=6) + rng.random_range(1..=6);
            
            // Calculate the modified roll
            let modified_roll = roll as i16 
                + buyer_broker_skill 
                - supplier_broker_skill 
                + good.best_purchase_dm 
                - good.best_sale_dm;
            
            // Determine the price multiplier based on the modified roll
            let price_multiplier = match modified_roll {
                i16::MIN..=-3 => 3.0,   // 300%
                -2 => 2.5,              // 250%
                -1 => 2.0,              // 200%
                0 => 1.75,              // 175%
                1 => 1.5,               // 150%
                2 => 1.35,              // 135%
                3 => 1.25,              // 125%
                4 => 1.2,               // 120%
                5 => 1.15,              // 115%
                6 => 1.1,               // 110%
                7 => 1.05,              // 105%
                8 => 1.0,               // 100%
                9 => 0.95,              // 95%
                10 => 0.9,              // 90%
                11 => 0.85,             // 85%
                12 => 0.8,              // 80%
                13 => 0.75,             // 75%
                14 => 0.7,              // 70%
                15 => 0.65,             // 65%
                16 => 0.6,              // 60%
                17 => 0.55,             // 55%
                18 => 0.5,              // 50%
                19 => 0.45,             // 45%
                20 => 0.4,              // 40%
                21 => 0.35,             // 35%
                22 => 0.3,              // 30%
                23 => 0.25,             // 25%
                24 => 0.2,              // 20%
                25.. => 0.15,           // 15%
            };
            
            // Apply the multiplier to the cost
            good.cost = (good.base_cost as f64 * price_multiplier).round() as i32;
        }
    }

    /// Sort goods from most discounted to least discounted
    pub fn sort_by_discount(&mut self) {
        self.goods.sort_by(|a, b| {
            // Calculate discount percentage for each good
            let a_discount = a.cost as f64 / a.base_cost as f64;
            let b_discount = b.cost as f64 / b.base_cost as f64;
            
            // Sort from lowest ratio (biggest discount) to highest ratio (smallest discount)
            a_discount.partial_cmp(&b_discount).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Find the best (highest) DM for a given set of trade classes
fn find_best_dm(
    dm_map: &std::collections::HashMap<TradeClass, i16>,
    world_trade_classes: &[TradeClass]
) -> i16 {
    let mut best_dm: i16 = 0;
    
    for trade_class in world_trade_classes {
        if let Some(&dm) = dm_map.get(trade_class) {
            if dm > best_dm {
                best_dm = dm;
            }
        }
    }
    
    best_dm
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TradeClass;
    use std::collections::HashMap;
    
    #[test_log::test]
    fn test_available_goods_table() {
        // Create a standard trade table
        let trade_table = TradeTable::standard().expect("Failed to create standard trade table");
        
        // Create a world with Agricultural and Rich trade classes
        let world_trade_classes = vec![TradeClass::Agricultural, TradeClass::Rich];
        
        // Create an available goods table for the world
        let available_goods = AvailableGoodsTable::for_world(
            &trade_table,
            &world_trade_classes,
            5, // Population 5
            false, // No illegal goods
        ).expect("Failed to create available goods table");
        
        // Verify that the table is not empty
        assert!(!available_goods.is_empty());
        
        // Check that all common goods are available (they have Availability::All)
        for i in 11..=16 {
            assert!(available_goods.get_by_index(i).is_some(), "Common good {} should be available", i);
        }
        
        // Check that agricultural goods are available
        let ag_goods = [33, 34, 52, 55]; // Some goods available for Agricultural worlds
        for i in ag_goods.iter() {
            assert!(available_goods.get_by_index(*i).is_some(), "Agricultural good {} should be available", i);
        }
        
        // Check that no illegal goods are available when illegal_ok is false
        for i in 61..=66 {
            assert!(available_goods.get_by_index(i).is_none(), "Illegal good {} should not be available", i);
        }
        
        // Check that DMs are correctly calculated
        // Common Electronics (11) has purchase DM for Rich+1
        let electronics = available_goods.get_by_index(11).unwrap();
        assert_eq!(electronics.best_purchase_dm, 1);
        
        // Agricultural Products (33) has purchase DM for Agricultural+2
        let ag_products = available_goods.get_by_index(33).unwrap();
        assert_eq!(ag_products.best_purchase_dm, 2);
        
        // Create another table with illegal goods allowed
        let available_goods_with_illegal = AvailableGoodsTable::for_world(
            &trade_table,
            &world_trade_classes,
            5, // Population 5
            true, // Allow illegal goods
        ).expect("Failed to create available goods table with illegal goods");
        
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
        let best_dm = find_best_dm(&dm_map, &world_trade_classes);
        assert_eq!(best_dm, 5);
        
        // World with only Agricultural trade class
        let world_trade_classes = vec![TradeClass::Agricultural];
        
        // Agricultural should be the best (and only) DM
        let best_dm = find_best_dm(&dm_map, &world_trade_classes);
        assert_eq!(best_dm, 3);
        
        // World with no matching trade classes
        let world_trade_classes = vec![TradeClass::Industrial, TradeClass::Poor];
        
        // No DM should be found
        let best_dm = find_best_dm(&dm_map, &world_trade_classes);
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
            availability: crate::table::Availability::All,
            quantity: crate::table::Quantity { dice: 2, multiplier: 1 },
            base_cost: 10000,
            purchase_dm,
            sale_dm,
        };
        
        // Create a world with both trade classes
        let world_trade_classes = vec![TradeClass::Rich, TradeClass::Agricultural];
        
        // Create a table with a single good
        let mut table = AvailableGoodsTable::new();
        table.add_entry(entry.clone(), 5, &world_trade_classes).unwrap();
        
        // Get the original cost
        let original_cost = table.goods()[0].cost;
        
        // Price the goods with equal broker skills
        // This will use a random roll, so we can't predict the exact price,
        // but we can check that the price has changed
        table.price_goods(0, 0);
        
        // The price should be different due to the DMs (purchase +2, sale -3)
        // and the random roll
        assert_ne!(table.goods()[0].cost, original_cost);
        
        // Create another table for a more controlled test
        let mut table2 = AvailableGoodsTable::new();
        table2.add_entry(entry.clone(), 5, &world_trade_classes).unwrap();
        
        // Set up a test where we know the outcome
        // If buyer has skill 3, supplier has skill 1, purchase DM is +2, sale DM is +3
        // Then the modifier is: +3 - 1 + 2 - 3 = +1
        // For a roll of 7, the modified roll would be 7 + 1 = 8, which is 100%
        // We can't control the roll, but we can check that the price is within
        // a reasonable range based on the skills and DMs
        table2.price_goods(3, 1);
        
        // The price should be affected by the skills and DMs
        // We can't assert an exact value due to the random roll
        let new_cost = table2.goods()[0].cost;
        println!("Original cost: {}, New cost: {}", original_cost, new_cost);
        
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
            source_entry: TradeTableEntry {
                index: 1,
                name: "Test Good".to_string(),
                availability: crate::table::Availability::All,
                quantity: crate::table::Quantity { dice: 2, multiplier: 1 },
                base_cost: 5000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
            best_purchase_dm: 2,
            best_sale_dm: 0,
        };
        
        // Check the display output
        assert_eq!(format!("{}", good), "Test Good: 10 @ 5000 (100% of base)");
        
        // Create a table with a single good
        let mut table = AvailableGoodsTable::new();
        table.goods.push(good);
        
        // Check the display output
        let expected = "Test Good: 10 @ 5000 (100% of base)\n";
        assert_eq!(format!("{}", table), expected);
        
        // Create an empty table
        let empty_table = AvailableGoodsTable::new();
        
        // Check the display output
        assert_eq!(format!("{}", empty_table), "No goods available\n");
    }

    // Test to test end to end by creating a TradeTable from the standard table
    // and then creating an AvailableGoodsTable from it, and then printing out
    // the available goods
    #[test_log::test]
    fn test_end_to_end() {
        // Create a standard trade table
        let trade_table = TradeTable::standard().expect("Failed to create standard trade table");
        
        // Create a world with Agricultural and Rich trade classes
        let world_trade_classes = vec![TradeClass::Agricultural, TradeClass::Rich];
        
        // Create an available goods table for the world
        let mut available_goods = AvailableGoodsTable::for_world(
            &trade_table,
            &world_trade_classes,
            5, // Population 5
            false, // No illegal goods
        ).expect("Failed to create available goods table");
        
        // Price the goods with equal broker skills
        available_goods.price_goods(0, 0);
        
        // Sort by discount
        available_goods.sort_by_discount();
        
        // Print out the available goods
        println!("{}", available_goods);
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
            cost: 5000,  // 50% of base
            source_entry: TradeTableEntry {
                index: 1,
                name: "Good 1".to_string(),
                availability: crate::table::Availability::All,
                quantity: crate::table::Quantity { dice: 2, multiplier: 1 },
                base_cost: 10000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
            best_purchase_dm: 0,
            best_sale_dm: 0,
        };
        
        let good2 = AvailableGood {
            name: "Good 2".to_string(),
            quantity: 10,
            base_cost: 10000,
            cost: 8000,  // 80% of base
            source_entry: TradeTableEntry {
                index: 2,
                name: "Good 2".to_string(),
                availability: crate::table::Availability::All,
                quantity: crate::table::Quantity { dice: 2, multiplier: 1 },
                base_cost: 10000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
            best_purchase_dm: 0,
            best_sale_dm: 0,
        };
        
        let good3 = AvailableGood {
            name: "Good 3".to_string(),
            quantity: 10,
            base_cost: 10000,
            cost: 2000,  // 20% of base
            source_entry: TradeTableEntry {
                index: 3,
                name: "Good 3".to_string(),
                availability: crate::table::Availability::All,
                quantity: crate::table::Quantity { dice: 2, multiplier: 1 },
                base_cost: 10000,
                purchase_dm: HashMap::new(),
                sale_dm: HashMap::new(),
            },
            best_purchase_dm: 0,
            best_sale_dm: 0,
        };
        
        // Add goods in random order
        table.goods.push(good1);
        table.goods.push(good2);
        table.goods.push(good3);
        
        // Sort by discount
        table.sort_by_discount();
        
        // Check that goods are sorted from most discounted to least discounted
        assert_eq!(table.goods[0].name, "Good 3");  // 20% of base
        assert_eq!(table.goods[1].name, "Good 1");  // 50% of base
        assert_eq!(table.goods[2].name, "Good 2");  // 80% of base
        
        // Print the sorted table
        println!("Sorted table:\n{}", table);
    }
}
