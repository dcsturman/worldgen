//! # Trade Table Module
//!
//! This module provides the core trade table functionality for the Traveller universe,
//! including trade good definitions, availability rules, and pricing modifiers.
//!
//! The trade table contains all standard trade goods with their base costs, availability
//! restrictions, quantity dice, and trade class modifiers for buying and selling.

use super::TradeClass;
use std::collections::HashMap;

use crate::trade::string_to_trade_class;

/// Main trade table containing all available trade goods
///
/// Maps two-digit indices (11-66) to trade table entries. The indices correspond
/// to 2d6 dice rolls used to randomly determine available goods.
#[derive(Debug, Clone)]
pub struct TradeTable {
    /// Map from two-digit index (11-66) to trade table entry
    entries: HashMap<i16, TradeTableEntry>,
}

/// Individual entry in the trade table representing a specific trade good
///
/// Contains all information needed to determine availability, quantity,
/// base cost, and trade modifiers for a specific good.
#[derive(Debug, Clone)]
pub struct TradeTableEntry {
    /// Two-digit index for dice roll lookup (11-66)
    pub index: i16,
    /// Human-readable name of the trade good
    pub name: String,
    /// Availability restrictions based on world trade classes
    pub availability: Availability,
    /// Dice formula for determining quantity available
    pub quantity: Quantity,
    /// Base cost per unit in credits
    pub base_cost: i32,
    /// Purchase price modifiers by trade class
    pub purchase_dm: HashMap<TradeClass, i16>,
    /// Sale price modifiers by trade class
    pub sale_dm: HashMap<TradeClass, i16>,
}

/// Availability restrictions for trade goods
///
/// Determines which worlds can produce or have access to specific trade goods
/// based on their trade classifications.
#[derive(Debug, Clone)]
pub enum Availability {
    /// Available on all worlds regardless of trade class
    All,
    /// Only available on worlds with specific trade classes
    List(Vec<TradeClass>),
}

/// Quantity determination for trade goods
///
/// Represents the dice formula used to determine how much of a good
/// is available for purchase (e.g., "2Dx10" means roll 2d6 and multiply by 10).
#[derive(Debug, Clone)]
pub struct Quantity {
    /// Number of dice to roll (1-3 typically)
    pub dice: u8,
    /// Multiplier applied to dice result
    pub multiplier: i16,
}

impl Default for TradeTable {
    /// Creates a new TradeTable with the standard trade goods
    ///
    /// Loads the complete set of 36 standard trade goods from the
    /// embedded data array. Panics if the data cannot be parsed.
    fn default() -> Self {
        let mut table = TradeTable {
            entries: HashMap::new(),
        };
        table
            .load_from_data(STANDARD_TRADE_GOODS)
            .unwrap_or_else(|e| {
                panic!("Failed to load standard trade goods from file: {e}");
            });
        table
    }
}

impl TradeTable {
    /// Creates a new empty trade table
    ///
    /// Equivalent to `TradeTable::default()` but more explicit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves a trade table entry by its index
    ///
    /// # Arguments
    ///
    /// * `index` - Two-digit index (11-66) corresponding to 2d6 roll
    ///
    /// # Returns
    ///
    /// Reference to the trade table entry, or None if index not found
    pub fn get(&self, index: i16) -> Option<&TradeTableEntry> {
        self.entries.get(&index)
    }

    /// Inserts a trade table entry into the table
    ///
    /// # Arguments
    ///
    /// * `entry` - Trade table entry to insert
    pub fn insert(&mut self, entry: TradeTableEntry) {
        self.entries.insert(entry.index, entry);
    }

    /// Load trade goods from embedded data array
    ///
    /// Parses trade good data from a structured array format and populates
    /// the trade table. Each row contains: index, name, availability, quantity,
    /// base_cost, purchase_dm, sale_dm.
    ///
    /// # Arguments
    ///
    /// * `data` - Array of trade good data rows
    ///
    /// # Returns
    ///
    /// Result indicating success or error message
    pub fn load_from_data(&mut self, data: &[&[&str; 7]]) -> Result<(), String> {
        for (line_num, row) in data.iter().enumerate() {
            let entry = Self::parse_data_row(row, line_num + 1)?;
            self.insert(entry);
        }

        Ok(())
    }

    /// Parse a single data row into a TradeTableEntry
    ///
    /// # Arguments
    ///
    /// * `row` - Array of 7 strings containing trade good data
    /// * `line_num` - Line number for error reporting
    ///
    /// # Returns
    ///
    /// Parsed trade table entry or error message
    fn parse_data_row(row: &[&str; 7], line_num: usize) -> Result<TradeTableEntry, String> {
        TradeTableEntry::from_string_with_line(
            row[0], // index
            row[1], // name
            row[2], // availability
            row[3], // quantity
            row[4], // base_cost
            row[5], // purchase_dm
            row[6], // sale_dm,
            line_num,
        )
    }

    /// Get all entries in the table
    ///
    /// # Returns
    ///
    /// Iterator over all trade table entries
    pub fn entries(&self) -> impl Iterator<Item = &TradeTableEntry> {
        self.entries.values()
    }

    /// Get the number of entries in the table
    ///
    /// # Returns
    ///
    /// Number of trade goods in the table
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the table is empty
    ///
    /// # Returns
    ///
    /// True if no trade goods are loaded
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl TradeTableEntry {
    /// Create a new TradeTableEntry from a set of strings with line number
    ///
    /// Parses all components of a trade table entry from string format.
    /// Provides detailed error messages including line numbers for debugging.
    ///
    /// # Arguments
    ///
    /// * `index_str` - Two-digit index string (e.g., "11", "66")
    /// * `name` - Human-readable name of the trade good
    /// * `availability_str` - Availability string ("All" or space-separated trade codes)
    /// * `quantity_str` - Quantity formula (e.g., "2Dx10", "1Dx5")
    /// * `base_cost_str` - Base cost in credits as string
    /// * `purchase_dm_str` - Purchase modifiers (e.g., "In+2 Ht+3")
    /// * `sale_dm_str` - Sale modifiers (e.g., "Ni+2 Lt+1")
    /// * `line_num` - Line number for error reporting
    ///
    /// # Returns
    ///
    /// Parsed trade table entry or detailed error message
    #[allow(clippy::too_many_arguments)]
    pub fn from_string_with_line(
        index_str: &str,
        name: &str,
        availability_str: &str,
        quantity_str: &str,
        base_cost_str: &str,
        purchase_dm_str: &str,
        sale_dm_str: &str,
        line_num: usize,
    ) -> Result<Self, String> {
        // Parse index (two digits between 1-6)
        if index_str.len() != 2 {
            return Err(format!(
                "Line {line_num}: Index must be 2 digits, got {index_str}"
            ));
        }
        let d1 = index_str.chars().nth(0).unwrap().to_digit(10);
        let d2 = index_str.chars().nth(1).unwrap().to_digit(10);

        if d1.is_none()
            || d2.is_none()
            || d1.unwrap() < 1
            || d1.unwrap() > 6
            || d2.unwrap() < 1
            || d2.unwrap() > 6
        {
            return Err(format!(
                "Line {line_num}: Index digits must be between 1-6, got {index_str}"
            ));
        }

        let index = (d1.unwrap() * 10 + d2.unwrap()) as i16;

        // Parse availability
        let availability = if availability_str == "All" {
            Availability::All
        } else {
            let codes: Vec<&str> = availability_str.split_whitespace().collect();
            let mut trade_classes = Vec::new();

            for code in codes {
                if let Some(tc) = string_to_trade_class(code) {
                    trade_classes.push(tc);
                } else {
                    return Err(format!(
                        "Line {line_num} (Index {index}): Invalid trade class code: {code}"
                    ));
                }
            }

            Availability::List(trade_classes)
        };

        // Parse quantity (nDxM format)
        let parts: Vec<&str> = quantity_str.split(&['D', 'x']).collect();
        if parts.len() != 3 {
            return Err(format!(
                "Line {line_num} (Index {index}): Quantity must be in format nDxM, got {quantity_str}"
            ));
        }

        let dice = parts[0].parse::<u8>().map_err(|_| {
            format!(
                "Line {line_num} (Index {index}): Invalid dice count: {}",
                parts[0]
            )
        })?;
        let multiplier = parts[2].parse::<i16>().map_err(|_| {
            format!(
                "Line {} (Index {}): Invalid multiplier: {}",
                line_num, index, parts[2]
            )
        })?;

        let quantity = Quantity { dice, multiplier };

        // Parse base cost
        let base_cost = base_cost_str.parse::<i32>().map_err(|_| {
            format!("Line {line_num} (Index {index}): Invalid base cost: {base_cost_str}")
        })?;

        // Parse purchase DMs
        let purchase_dm = parse_dm_string_with_line(purchase_dm_str, line_num, Some(index))?;

        // Parse sale DMs
        let sale_dm = parse_dm_string_with_line(sale_dm_str, line_num, Some(index))?;

        Ok(TradeTableEntry {
            index,
            name: name.to_string(),
            availability,
            quantity,
            base_cost,
            purchase_dm,
            sale_dm,
        })
    }

    /// Create a new TradeTableEntry from strings without line number tracking
    ///
    /// Convenience wrapper around `from_string_with_line` for cases where
    /// line number tracking is not needed.
    ///
    /// # Arguments
    ///
    /// Same as `from_string_with_line` except no `line_num` parameter
    ///
    /// # Returns
    ///
    /// Parsed trade table entry or error message
    pub fn from_string(
        index_str: &str,
        name: &str,
        availability_str: &str,
        quantity_str: &str,
        base_cost_str: &str,
        purchase_dm_str: &str,
        sale_dm_str: &str,
    ) -> Result<Self, String> {
        Self::from_string_with_line(
            index_str,
            name,
            availability_str,
            quantity_str,
            base_cost_str,
            purchase_dm_str,
            sale_dm_str,
            0, // Use 0 to indicate unknown line number
        )
    }
}

/// Parse DM (Dice Modifier) strings with line number and optional index
///
/// Parses strings like "In+2 Ht+3 Ri+1" into a HashMap mapping trade classes
/// to their modifier values. Used for both purchase and sale modifiers.
///
/// # Arguments
///
/// * `dm_str` - String containing space-separated trade class modifiers
/// * `line_num` - Line number for error reporting
/// * `index` - Optional trade good index for error reporting
///
/// # Returns
///
/// HashMap mapping trade classes to modifier values, or error message
///
/// # Format
///
/// Each modifier consists of a trade class code followed immediately by
/// a + or - and a numeric value (e.g., "In+2", "Ag-1").
fn parse_dm_string_with_line(
    dm_str: &str,
    line_num: usize,
    index: Option<i16>,
) -> Result<HashMap<TradeClass, i16>, String> {
    let mut dm_map = HashMap::new();

    if dm_str.is_empty() {
        return Ok(dm_map);
    }

    let parts: Vec<&str> = dm_str.split_whitespace().collect();

    for part in parts {
        // Find the position of + or -
        let sign_pos = part.find(['+', '-']).ok_or_else(|| {
            if let Some(idx) = index {
                format!("Line {line_num} (Index {idx}): No + or - found in DM: {part}")
            } else {
                format!("Line {line_num}: No + or - found in DM: {part}")
            }
        })?;

        let code = &part[..sign_pos];
        let dm_str = &part[sign_pos..];

        let trade_class = string_to_trade_class(code).ok_or_else(|| {
            if let Some(idx) = index {
                format!("Line {line_num} (Index {idx}): Invalid trade class code: {code}")
            } else {
                format!("Line {line_num}: Invalid trade class code: {code}")
            }
        })?;

        let dm = dm_str.parse::<i16>().map_err(|_| {
            if let Some(idx) = index {
                format!("Line {line_num} (Index {idx}): Invalid DM value: {dm_str}")
            } else {
                format!("Line {line_num}: Invalid DM value: {dm_str}")
            }
        })?;

        dm_map.insert(trade_class, dm);
    }

    Ok(dm_map)
}

/// Standard trade goods table in a compact format
///
/// Contains all 36 standard trade goods from the Traveller rules.
/// Each entry is an array of 7 strings:
/// [index, name, availability, quantity, base_cost, purchase_dm, sale_dm]
///
/// The table covers:
/// - Common goods (11-16): Basic trade items available everywhere
/// - Advanced goods (21-26): High-tech items with restricted availability  
/// - Specialized goods (31-36): Unique items for specific world types
/// - Raw materials (41-46): Industrial inputs and precious materials
/// - Bulk goods (51-56): Large-volume, low-value items
/// - Illegal goods (61-66): Contraband with high risk/reward
static STANDARD_TRADE_GOODS: &[&[&str; 7]] = &[
    // Common goods (11-16) - Available on all worlds
    &[
        "11",
        "Common Electronics",
        "All",
        "2Dx10",
        "20000",
        "In+2 Ht+3 Ri+1",
        "Ni+2 Lt+1 Po+1",
    ],
    &[
        "12",
        "Common Industrial Goods",
        "All",
        "2Dx10",
        "10000",
        "Na+2 In+5",
        "Ni+3 Hi+2",
    ],
    &[
        "13",
        "Common Manufactured Goods",
        "All",
        "2Dx10",
        "20000",
        "Na+2 In+5",
        "Ni+3 Hi+2",
    ],
    &[
        "14",
        "Common Raw Materials",
        "All",
        "2Dx20",
        "5000",
        "Ag+3 Ga+2",
        "In+2 Po+2",
    ],
    &[
        "15",
        "Common Consumables",
        "All",
        "2Dx20",
        "500",
        "Ag+3 Wa+2 Ga+1 As-4",
        "As+1 Fl+1 Ic+1 Hi+1",
    ],
    &[
        "16",
        "Common Ore",
        "All",
        "2Dx20",
        "1000",
        "As+4",
        "In+3 Ni+1",
    ],
    // Advanced goods (21-26) - High-tech and industrial worlds only
    &[
        "21",
        "Advanced Electronics",
        "Ht In",
        "1Dx5",
        "100000",
        "In+2 Ht+3",
        "Ri+2 Ni+1 As+3",
    ],
    &[
        "22",
        "Advanced Machine Parts",
        "Ht In",
        "1Dx5",
        "75000",
        "In+2 Ht+1",
        "As+2 Ni+1",
    ],
    &[
        "23",
        "Advanced Manufactured Goods",
        "Ht In",
        "1Dx5",
        "100000",
        "In+1",
        "Hi+1 Ri+2",
    ],
    &[
        "24",
        "Advanced Weapons",
        "Ht In",
        "1Dx5",
        "150000",
        "Ht+2",
        "Po+1 Az+2 Rz+4",
    ],
    &[
        "25",
        "Advanced Vehicles",
        "Ht In",
        "1Dx5",
        "180000",
        "Ht+2",
        "Ri+2 As+2",
    ],
    &[
        "26",
        "Biochemicals",
        "Ag Wa",
        "1Dx5",
        "50000",
        "Ag+1 Wa+2",
        "In+2",
    ],
    // Specialized goods (31-36) - Specific world types
    &[
        "31",
        "Crystals & Gems",
        "As De Ic",
        "1Dx5",
        "20000",
        "As+2 De+1 Ic+1",
        "In+3 Ri+2",
    ],
    &[
        "32",
        "Cybernetics",
        "Ht",
        "1Dx1",
        "250000",
        "Ht+1",
        "As+1 Ic+1 Ri+2",
    ],
    &[
        "33",
        "Live Animals",
        "Ag Ga",
        "1Dx10",
        "10000",
        "Ag+2",
        "Lo+3",
    ],
    &[
        "34",
        "Luxury Consumables",
        "Ag Ga Wa",
        "1Dx10",
        "20000",
        "Ag+2 Wa+1",
        "Ri+2 Hi+2",
    ],
    &["35", "Luxury Goods", "Hi", "1Dx1", "200000", "Hi+1", "Ri+4"],
    &[
        "36",
        "Medical Supplies",
        "Ht Hi",
        "1Dx5",
        "50000",
        "Ht+2",
        "In+2 Po+1 Ri+1",
    ],
    // Raw materials and industrial inputs (41-46)
    &[
        "41",
        "Petrochemicals",
        "De Fl Ic Wa",
        "1Dx10",
        "10000",
        "De+2",
        "In+2 Ag+1 Lt+2",
    ],
    &[
        "42",
        "Pharmaceuticals",
        "As De Hi Wa",
        "1Dx1",
        "100000",
        "As+2 Hi+1",
        "Ri+2 Lt+1",
    ],
    &["43", "Polymers", "In", "1Dx10", "7000", "In+1", "Ri+2 Ni+1"],
    &[
        "44",
        "Precious Metals",
        "As De Ic Fl",
        "1Dx1",
        "50000",
        "As+3 De+1 Ic+2",
        "In+2 Ri+3 Ht+1",
    ],
    &[
        "45",
        "Radioactives",
        "As De Lo",
        "1Dx1",
        "1000000",
        "As+2 Lo+2",
        "In+3 Ht+1 Ni-2 Ag-3",
    ],
    &["46", "Robots", "In", "1Dx5", "400000", "In+1", "Ag+2 Ht+1"],
    // Bulk goods (51-56) - High volume, lower value
    &[
        "51",
        "Spices",
        "De Ga Wa",
        "1Dx10",
        "6000",
        "De+2",
        "Hi+2 Ri+3 Po+3",
    ],
    &[
        "52",
        "Textiles",
        "Ag Ni",
        "1Dx20",
        "3000",
        "Ag+7",
        "Hi+3 Na+2",
    ],
    &[
        "53",
        "Uncommon Ore",
        "As Ic",
        "1Dx20",
        "5000",
        "As+4",
        "In+3 Ni+1",
    ],
    &[
        "54",
        "Uncommon Raw Materials",
        "Ag De Wa",
        "1Dx10",
        "20000",
        "Ag+2 Wa+1",
        "In+2 Ht+1",
    ],
    &["55", "Wood", "Ag Ga", "1Dx20", "1000", "Ag+6", "Ri+2 In+1"],
    &[
        "56",
        "Vehicles",
        "In Ht",
        "1Dx10",
        "15000",
        "In+2 Ht+1",
        "Ni+2 Hi+1",
    ],
    // Illegal goods (61-65) - Contraband with high risk/reward
    &[
        "61",
        "Illegal Biochemicals",
        "Ag Wa",
        "1Dx5",
        "50000",
        "Wa+2",
        "In+6",
    ],
    &[
        "62",
        "Illegal Cybernetics",
        "Ht",
        "1Dx1",
        "250000",
        "Ht+1",
        "As+4 Ic+4 Ri+8 Az+6 Rz+6",
    ],
    &[
        "63",
        "Illegal Drugs",
        "As De Hi Wa",
        "1Dx1",
        "100000",
        "As+1 De+1 Ga+1 Wa+1",
        "Ri+6 Hi+6",
    ],
    &[
        "64",
        "Illegal Luxuries",
        "Ag Ga Wa",
        "1Dx1",
        "50000",
        "Ag+2 Wa+1",
        "Ri+6 Hi+4",
    ],
    &[
        "65",
        "Illegal Weapons",
        "Ht In",
        "1Dx5",
        "150000",
        "Ht+2",
        "Po+6 Az+8 Rz+10",
    ],
    // Exotics (66) - Special rare items
    &["66", "Exotics", "", "1Dx1", "1000000", "", ""],
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::TradeClass;

    #[test]
    fn test_standard_trade_table() {
        // Create a standard trade table
        let trade_table = TradeTable::default();

        // Verify the table is not empty
        assert!(!trade_table.is_empty());

        // Verify we have the expected number of entries (36 for the standard table)
        assert_eq!(trade_table.len(), 36);

        // Test a few specific entries to ensure they were parsed correctly

        // Common Electronics (11)
        let entry = trade_table.get(11).expect("Missing entry 11");
        assert_eq!(entry.index, 11);
        assert_eq!(entry.name, "Common Electronics");
        assert!(matches!(entry.availability, Availability::All));
        assert_eq!(entry.quantity.dice, 2);
        assert_eq!(entry.quantity.multiplier, 10);
        assert_eq!(entry.base_cost, 20000);

        // Check purchase DMs
        assert_eq!(entry.purchase_dm.get(&TradeClass::Industrial), Some(&2));
        assert_eq!(entry.purchase_dm.get(&TradeClass::HighTech), Some(&3));
        assert_eq!(entry.purchase_dm.get(&TradeClass::Rich), Some(&1));

        // Check sale DMs
        assert_eq!(entry.sale_dm.get(&TradeClass::NonIndustrial), Some(&2));
        assert_eq!(entry.sale_dm.get(&TradeClass::LowTech), Some(&1));
        assert_eq!(entry.sale_dm.get(&TradeClass::Poor), Some(&1));

        // Advanced Weapons (24)
        let entry = trade_table.get(24).expect("Missing entry 24");
        assert_eq!(entry.index, 24);
        assert_eq!(entry.name, "Advanced Weapons");

        // Check availability
        if let Availability::List(classes) = &entry.availability {
            assert_eq!(classes.len(), 2);
            assert!(classes.contains(&TradeClass::HighTech));
            assert!(classes.contains(&TradeClass::Industrial));
        } else {
            panic!("Expected List availability for Advanced Weapons");
        }

        // Illegal Weapons (65)
        let entry = trade_table.get(65).expect("Missing entry 65");
        assert_eq!(entry.index, 65);
        assert_eq!(entry.name, "Illegal Weapons");
        assert_eq!(entry.base_cost, 150000);

        // Check sale DMs for illegal weapons
        assert_eq!(entry.sale_dm.get(&TradeClass::Poor), Some(&6));
        assert_eq!(entry.sale_dm.get(&TradeClass::AmberZone), Some(&8));
        assert_eq!(entry.sale_dm.get(&TradeClass::RedZone), Some(&10));

        // Exotics (66)
        let entry = trade_table.get(66).expect("Missing entry 66");
        assert_eq!(entry.index, 66);
        assert_eq!(entry.name, "Exotics");
        assert_eq!(entry.base_cost, 1000000);
        assert!(entry.purchase_dm.is_empty());
        assert!(entry.sale_dm.is_empty());
    }
}
