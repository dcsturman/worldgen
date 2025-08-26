//! # Satellite Generation Trait Module
//! 
//! This module defines the `HasSatellites` trait, which provides a common interface
//! for any astronomical body that can host satellite worlds. This includes both
//! gas giants and regular worlds in the Traveller universe.
//! 
//! ## Key Features
//! 
//! - **Universal Interface**: Common methods for satellite management across different body types
//! - **Orbital Management**: Automatic orbit assignment and collision avoidance
//! - **Ring System Handling**: Special processing for planetary ring systems
//! - **Satellite Generation**: Complete world generation for satellite bodies
//! - **Sorting and Cleanup**: Automatic organization of satellite collections
//! 
//! ## Satellite Types
//! 
//! - **Regular Satellites**: Full worlds with complete UWP characteristics
//! - **Ring Systems**: Size 0 satellites representing planetary rings
//! - **Close Orbits**: Satellites in tight orbits around the parent body
//! - **Far Orbits**: Distant satellites with different environmental conditions
//! - **Extreme Orbits**: Very distant satellites (gas giants only)
//! 
//! ## Ring System Processing
//! 
//! The trait includes special handling for ring systems:
//! - Each world/gas giant can only have one ring system so only the first ring system is kept, others are removed
//! - Ring systems get minimal characteristics (Y-class starport, no population)
//! 
//! ## Usage
//! 
//! ```rust
//! use worldgen::systems::has_satellites::HasSatellites;
//! use worldgen::systems::gas_giant::GasGiant;
//! 
//! let mut gas_giant = GasGiant::new(GasGiantSize::Large, 5);
//! 
//! // Generate satellites
//! for _ in 0..gas_giant.determine_num_satellites() {
//!     gas_giant.gen_satellite(&system_zones, &main_world, &star);
//! }
//! 
//! // Clean up and organize
//! gas_giant.clean_satellites();
//! ```

use crate::systems::system::Star;
use crate::systems::system_tables::ZoneTable;
use crate::systems::world::{Satellites, World};

/// Trait for astronomical bodies that can host satellite worlds
/// 
/// Provides a common interface for satellite management, generation, and organization
/// across different types of parent bodies (gas giants, worlds, etc.). Implementors
/// must provide the core satellite access methods and generation logic, while the
/// trait provides common utilities for organization and cleanup.
pub trait HasSatellites {
    /// Returns the current number of satellites orbiting this body
    /// 
    /// # Returns
    /// 
    /// Count of satellites in the collection
    fn get_num_satellites(&self) -> usize;

    /// Retrieves a satellite at the specified orbital position
    /// 
    /// Searches the satellite collection for a world at the given orbit.
    /// Used for collision detection during orbit assignment.
    /// 
    /// # Arguments
    /// 
    /// * `orbit` - Orbital position to search for
    /// 
    /// # Returns
    /// 
    /// Reference to the satellite world if found, None otherwise
    fn get_satellite(&self, orbit: usize) -> Option<&World>;

    /// Returns a mutable reference to the satellite collection
    /// 
    /// Provides direct access to the underlying satellite storage for
    /// modification operations like sorting and cleanup.
    /// 
    /// # Returns
    /// 
    /// Mutable reference to the `Satellites` collection
    fn get_satellites_mut(&mut self) -> &mut Satellites;

    /// Adds a new satellite to this body's system
    /// 
    /// Appends a satellite world to the collection. The satellite should
    /// already have its orbital position and characteristics determined.
    /// 
    /// # Arguments
    /// 
    /// * `satellite` - World to add as a satellite
    fn push_satellite(&mut self, satellite: World);

    /// Sorts satellites by orbital position
    /// 
    /// Organizes the satellite collection in ascending order by orbit number.
    /// This ensures consistent display order and simplifies orbital mechanics
    /// calculations. Called automatically by `clean_satellites()`.
    fn sort_satellites(&mut self) {
        self.get_satellites_mut()
            .sats
            .sort_by(|a, b| a.orbit.cmp(&b.orbit));
    }

    /// Consolidates ring systems and organizes satellite collection
    /// 
    /// Performs comprehensive cleanup of the satellite system:
    /// 
    /// 1. **Sorting**: Orders satellites by orbital position
    /// 2. **Ring Detection**: Identifies all size 0 satellites (rings)
    /// 3. **Ring Consolidation**: Removes duplicate ring systems
    /// 4. **Ring Naming**: Renames the remaining ring to "Ring System"
    /// 
    /// ## Ring System Logic
    /// 
    /// Multiple ring systems around the same body are unrealistic, so this method:
    /// - Keeps only the first ring system (lowest orbit)
    /// - Removes all subsequent ring systems
    /// - Ensures the remaining ring has the standard name "Ring System"
    /// 
    /// ## Performance Note
    /// 
    /// Ring removal is done in reverse order to avoid index shifting issues
    /// when removing multiple elements from the vector.
    fn clean_satellites(&mut self) {
        self.sort_satellites();
        
        // Find all ring systems (size 0 satellites)
        let ring_indices: Vec<usize> = self
            .get_satellites_mut()
            .sats
            .iter()
            .enumerate()
            .filter(|(_, satellite)| satellite.size == 0)
            .map(|(index, _)| index)
            .collect();

        // If no rings found, nothing to clean
        if ring_indices.is_empty() {
            return;
        }

        // Remove all ring systems except the first one
        // Process in reverse order to avoid index shifting
        for ring in ring_indices.iter().skip(1).rev() {
            self.get_satellites_mut().sats.remove(*ring);
        }
        
        // Rename the remaining ring system
        self.get_satellites_mut().sats[ring_indices[0]].name = "Ring System".to_string();
    }

    /// Determines the number of satellites this body should generate
    /// 
    /// Implementors must provide logic for calculating how many satellites
    /// should be generated based on the body's characteristics (size, type, etc.).
    /// This is typically based on dice rolls with modifiers.
    /// 
    /// # Returns
    /// 
    /// Number of satellites to generate (can be 0)
    fn determine_num_satellites(&self) -> i32;

    /// Generates an orbital position for a new satellite
    /// 
    /// Implementors must provide logic for determining where a new satellite
    /// should be placed. This typically involves:
    /// - Rolling dice to determine orbit type (close, far, extreme)
    /// - Applying modifiers based on satellite characteristics
    /// - Checking for orbital collisions and adjusting as needed
    /// 
    /// # Arguments
    /// 
    /// * `is_ring` - Whether this satellite is a ring system (affects orbit selection)
    /// 
    /// # Returns
    /// 
    /// Available orbital position for the new satellite
    fn gen_satellite_orbit(&self, is_ring: bool) -> usize;

    /// Generates a complete satellite world
    /// 
    /// Implementors must provide logic for creating a fully detailed satellite
    /// with all UWP characteristics. This typically includes:
    /// - Size determination based on parent body characteristics
    /// - Orbital position assignment using `gen_satellite_orbit()`
    /// - Environmental calculations (atmosphere, hydrographics)
    /// - Population and infrastructure generation
    /// - Astronomical data computation
    /// 
    /// # Arguments
    /// 
    /// * `system_zones` - Zone boundaries for environmental calculations
    /// * `main_world` - Primary world for trade and facility generation
    /// * `star` - Primary star for astronomical calculations
    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World, star: &Star);
}
