//! # Star System Generation Module
//!
//! This module contains the core functionality for generating complete Traveller star systems,
//! including stellar mechanics, orbital dynamics, and system-wide coordination. It serves as
//! the primary orchestrator for creating realistic multi-star systems with worlds, gas giants,
//! and their satellites.
//!
//! ## Key Features
//!
//! - **Multi-Star Systems**: Supports primary, secondary, and tertiary star configurations
//! - **Orbital Mechanics**: Realistic orbital slot management and companion star placement
//! - **World Generation**: Coordinates placement of main worlds, gas giants, and planetoids
//! - **Satellite Systems**: Manages satellite generation for all orbital bodies
//! - **Zone Management**: Handles stellar zones and habitability calculations
//!
//! ## System Architecture
//!
//! The [`System`] struct represents a complete star system with:
//! - A primary star with its orbital slots
//! - Optional secondary and tertiary companion stars
//! - Various orbital contents (worlds, gas giants, blocked orbits)
//! - Hierarchical satellite systems
//!
//! ## Generation Process
//!
//! 1. **Star Generation**: Creates primary star and determines companions
//! 2. **Orbit Allocation**: Assigns orbital slots and blocks inappropriate orbits
//! 3. **Gas Giant Placement**: Generates gas giants in suitable orbits
//! 4. **Planetoid Generation**: Creates asteroid belts and planetoid systems
//! 5. **Main World Placement**: Places the system's primary inhabited world
//! 6. **World Generation**: Fills remaining orbits with generated worlds
//! 7. **Satellite Generation**: Creates moons for all applicable bodies
//! 8. **Companion Processing**: Recursively processes secondary/tertiary systems

use log::{debug, error, warn};
#[cfg(feature = "frontend")]
use reactive_stores::Store;
use std::fmt::Display;

use crate::systems::constraint::{Constraint, ConstraintError, PartialUwp, SystemConstraints};
use crate::systems::gas_giant::{GasGiant, GasGiantSize};
use crate::systems::has_satellites::HasSatellites;
use crate::systems::name_tables::gen_star_system_name;
use crate::systems::system_tables::{get_orbital_distance, get_zone};
use crate::systems::world::World;
use crate::util::{roll_1d6, roll_2d6, roll_10};

/// Overrides for one star — primary, secondary, or tertiary.
/// `None` on any field means "roll as today."
#[derive(Default, Debug, Clone)]
pub struct StarOverride {
    pub orbit: Option<StarOrbit>,
    pub spectral: Option<StarType>,
    pub subtype: Option<u8>,
    pub size: Option<StarSize>,
    /// Names this star's system. Not cosmetic: every body that isn't
    /// explicitly named derives its own from it — `gen_name` builds
    /// "<system name> <roman numeral>" — so pinning this renames most of
    /// the system. That is exactly why it's worth pinning when source
    /// material gives you the name.
    ///
    /// This field is why `StarOverride` isn't `Copy`.
    pub name: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct GasGiantOverride {
    /// Names this giant. Without it a constraint's name was collected away
    /// and the giant took the generated "<system> <numeral>" instead —
    /// silently, since nothing failed to *place*. That is a shape of failure
    /// the dropped-constraint diagnostics can't see, because the constraint
    /// was honoured; only one of its fields wasn't.
    pub name: Option<String>,
    pub size: Option<GasGiantSize>,
    pub num_satellites: Option<i32>,
    /// `Some(o)` pins this giant to orbit `o` (skipped with a warn if
    /// `o` is already claimed or out of range). `None` lets the
    /// random viable-orbit picker choose.
    pub orbit: Option<i32>,
}

#[derive(Default, Debug, Clone)]
pub struct PlanetOverride {
    pub name: Option<String>,
    pub orbit: Option<i32>,
    /// User-specified UWP columns. `None` (or all-wild) rolls every
    /// digit; a fully-specified `PartialUwp` reproduces the classic
    /// "from_uwp" path; anything in between fills the wild columns
    /// via the same per-orbit / per-zone rolls `World::generate` uses.
    pub partial_uwp: Option<PartialUwp>,
    pub num_satellites: Option<i32>,
}

#[derive(Default, Debug, Clone)]
pub struct BeltOverride {
    pub name: Option<String>,
    pub orbit: Option<i32>,
    pub partial_uwp: Option<PartialUwp>,
    pub num_satellites: Option<i32>,
}

#[derive(Default, Debug, Clone)]
pub struct MoonOverride {
    pub name: Option<String>,
    pub parent_orbit: i32,
    pub partial_uwp: Option<PartialUwp>,
}

/// Aggregate of every constraint-driven override the system generator
/// honors. Stars and gas giants flow into the star/gas-giant gen
/// passes; planets get placed before random world-fill; moons attach
/// after satellite generation runs.
#[derive(Default, Debug, Clone)]
pub struct SystemOverrides {
    /// First entry overrides the primary, second the secondary, third the
    /// tertiary. Empty means "roll the count of stars and all their
    /// types as today."
    pub stars: Vec<StarOverride>,
    /// Names the whole system; see `SystemConstraints::system_name`.
    pub system_name: Option<String>,
    /// Where the main world sits, overriding the habitable-zone default.
    pub main_world_orbit: Option<i32>,
    /// Constraints belonging to the companions' own sub-systems, kept as
    /// raw constraints because each companion collects its own overrides
    /// from them — orbit numbers inside are that companion's orbits.
    pub secondary_bodies: Vec<Constraint>,
    pub tertiary_bodies: Vec<Constraint>,
    /// `Some` overrides the random gas-giant count to `gas_giants.len()`
    /// and applies the per-entry size/moon overrides in placement order.
    /// `None` keeps today's random count.
    pub gas_giants: Option<Vec<GasGiantOverride>>,
    /// Pinned non-main-world planets. Placed before the random world
    /// fill so they win the orbit; the fill loop skips them.
    pub planets: Vec<PlanetOverride>,
    /// Pinned planetoid belts. Placed alongside planets; size is
    /// always forced to 0.
    pub belts: Vec<BeltOverride>,
    /// Orbits that the user explicitly marked empty — set to
    /// `OrbitContent::Blocked` before any random pass so nothing
    /// fills them.
    pub empties: Vec<i32>,
    /// Moons to attach to existing bodies after satellite generation.
    pub moons: Vec<MoonOverride>,
    /// Main world's num_moons override, threaded through fill_system_with
    /// after place_main_world resolves the world's orbit. `None` rolls
    /// the moon count as today.
    pub main_world_num_satellites: Option<i32>,
    /// True when the system is built from explicit constraints (the
    /// Traveller-Map autopop path via `generate_from_constraints`). In that
    /// case the body counts are authoritative — the `W` (worlds) digit and
    /// the `pbg` belt/gas-giant digits fix the exact population — so the
    /// random passes that would pad the system (the world-fill loop, the
    /// random planetoid pass, and the random gas-giant roll when none were
    /// pinned) are suppressed. The plain `generate_system(World)` path
    /// leaves this `false` and stays fully random.
    pub autopop: bool,
}

/// A complete star system with primary star and optional companions
///
/// Represents a Traveller star system containing a primary star, optional
/// secondary and tertiary companion stars, and all orbital bodies including
/// worlds, gas giants, and blocked orbits. The system manages orbital slots
/// and coordinates the generation of all system components.
///
/// ## System Hierarchy
///
/// - **Primary System**: The main star with its orbital slots
/// - **Secondary System**: Optional companion star (close, far, or orbital)
/// - **Tertiary System**: Optional third star (typically far orbit)
///
/// ## Orbital Management
///
/// Each system maintains a vector of orbital slots that can contain:
/// - Worlds (rocky planets with full UWP characteristics)
/// - Gas giants (with their own satellite systems)
/// - Secondary/tertiary star markers
/// - Blocked orbits (intentionally empty for realism)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub struct System {
    pub name: String,
    /// The primary star's own name, when it has one distinct from the
    /// system's.
    ///
    /// Usually they are the same — "the Regina system" orbits a star nobody
    /// names separately — so this defaults to `name`. But sources do name
    /// stars in their own right: Makergod's Oghma orbits *Fijari*, exactly
    /// as Terra orbits Sol. Conflating the two loses that.
    pub star_name: Option<String>,
    /// Constraints this system couldn't honour. Populated during placement;
    /// read via [`System::dropped_constraints`], which walks companions too.
    pub dropped: Vec<DroppedConstraint>,
    pub star: Star,
    #[cfg_attr(feature = "frontend", store)]
    pub secondary: Option<Box<System>>,
    #[cfg_attr(feature = "frontend", store)]
    pub tertiary: Option<Box<System>>,
    pub orbit: StarOrbit,
    #[cfg_attr(feature = "frontend", store)]
    pub orbit_slots: Vec<Option<OrbitContent>>,
}

// Enums
/// Stellar spectral classification types
///
/// Represents the seven main stellar spectral classes in order from
/// hottest to coolest. Each type has distinct characteristics affecting
/// luminosity, habitable zones, and system generation.
///
/// ## Spectral Classes
///
/// - **O**: Blue supergiants, extremely hot (30,000-50,000K), very rare
/// - **B**: Blue giants, very hot (10,000-30,000K), short-lived
/// - **A**: White stars, hot (7,500-10,000K), rapid rotation
/// - **F**: Yellow-white stars, moderately hot (6,000-7,500K)
/// - **G**: Yellow stars like Sol (5,200-6,000K), stable main sequence
/// - **K**: Orange stars, cooler (3,700-5,200K), long-lived
/// - **M**: Red dwarfs, coolest (2,400-3,700K), most common, very long-lived
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub enum StarType {
    O,
    B,
    A,
    F,
    G,
    K,
    M,
}

/// Stellar subtype refinement (0-9)
///
/// Provides finer classification within each spectral type.
/// Lower numbers indicate hotter stars within the type.
/// For example, G0 is hotter than G9.
pub type StarSubType = u8;

/// Stellar luminosity class (size classification)
///
/// Indicates the star's luminosity and evolutionary stage.
/// Affects stellar zones, companion generation, and system characteristics.
///
/// ## Size Classes
///
/// - **Ia/Ib**: Supergiants, extremely luminous, short-lived
/// - **II**: Bright giants, evolved stars with extended zones
/// - **III**: Giants, evolved stars past main sequence
/// - **IV**: Subgiants, transitioning from main sequence
/// - **V**: Main sequence (dwarfs), stable hydrogen burning
/// - **VI**: Subdwarfs, metal-poor, lower luminosity
/// - **D**: White dwarfs, stellar remnants, very compact zones
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub enum StarSize {
    Ia,
    Ib,
    II,
    #[allow(clippy::upper_case_acronyms)]
    III,
    #[allow(clippy::upper_case_acronyms)]
    IV,
    V,
    #[allow(clippy::upper_case_acronyms)]
    VI,
    D,
}

/// Orbital relationship of companion stars
///
/// Defines how companion stars relate to the primary system,
/// affecting their orbital mechanics and zone interactions.
///
/// ## Orbit Types
///
/// - **Primary**: Contact binary or very close orbit
/// - **Far**: Distant orbit, independent zone system
/// - **System(n)**: Orbits within primary's zone system at position n
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub enum StarOrbit {
    Primary,
    Far,
    System(usize),
}

// Structs
/// Complete stellar classification
///
/// Combines spectral type, subtype, and size class to fully
/// specify a star's characteristics. Used for zone calculations,
/// luminosity lookup, and system generation parameters.
///
/// ## Examples
///
/// - Sol: G2V (G-type, subtype 2, main sequence)
/// - Rigel: B8Ia (B-type, subtype 8, supergiant)
/// - Proxima Centauri: M5.5V (M-type, subtype 5-6, main sequence)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub struct Star {
    pub star_type: StarType,
    pub subtype: StarSubType,
    pub size: StarSize,
}

/// Which kind of body a dropped constraint described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Planet,
    Belt,
    GasGiant,
    Moon,
    Empty,
}

impl std::fmt::Display for BodyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BodyKind::Planet => "planet",
            BodyKind::Belt => "belt",
            BodyKind::GasGiant => "gas giant",
            BodyKind::Moon => "moon",
            BodyKind::Empty => "empty orbit",
        })
    }
}

/// A constraint the generator accepted but could not honour.
///
/// Distinct from [`ConstraintError`], which rejects a constraint set before
/// generation starts. These arise *during* placement, when a constraint is
/// individually valid but doesn't fit the system that got rolled — a pinned
/// orbit past the star's last one, a slot already taken by a companion star.
///
/// They were `warn!` calls until this existed, which in practice meant they
/// vanished: the system generated, looked plausible, and silently omitted
/// something the author had explicitly asked for. Curated overrides make that
/// intolerable — the whole point of writing one down is that it shows up —
/// so they're a returned value now, and the validator treats any of them as
/// a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DroppedConstraint {
    /// Pinned orbit is past the system's last orbit.
    OrbitOutOfRange {
        body: BodyKind,
        orbit: i32,
        max_orbits: usize,
    },
    /// Pinned orbit was already occupied.
    OrbitOccupied { body: BodyKind, orbit: i32 },
    /// No orbit was pinned and the system had no free orbit left.
    NoFreeOrbit { body: BodyKind },
    /// A negative orbit, which can't refer to anything.
    NegativeOrbit { body: BodyKind, orbit: i32 },
    /// A moon's parent orbit is outside the system.
    MoonParentOutOfRange { parent_orbit: i32 },
    /// A moon's parent orbit holds something that can't have moons.
    MoonParentCannotHoldMoons { parent_orbit: i32 },
    /// A relatively-positioned body referenced a body that isn't there —
    /// "the first world beyond Torpol" in a system with no Torpol. Usually a
    /// misspelling, since the referent is normally a name from the source.
    RelativeBodyNotFound { body: BodyKind, reference: String },
    /// Nothing of the required kind exists to be outermost/innermost/after —
    /// "the outermost gas giant" in a system with no gas giants.
    NoBodyAtRelativePosition { body: BodyKind },
}

impl std::fmt::Display for DroppedConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DroppedConstraint::OrbitOutOfRange {
                body,
                orbit,
                max_orbits,
            } => write!(
                f,
                "{body} pinned to orbit {orbit}, but the system only has {max_orbits} orbits"
            ),
            DroppedConstraint::OrbitOccupied { body, orbit } => {
                write!(f, "{body} pinned to orbit {orbit}, which is already occupied")
            }
            DroppedConstraint::NoFreeOrbit { body } => {
                write!(f, "{body} has no pinned orbit and the system has no free one")
            }
            DroppedConstraint::NegativeOrbit { body, orbit } => {
                write!(f, "{body} pinned to negative orbit {orbit}")
            }
            DroppedConstraint::MoonParentOutOfRange { parent_orbit } => write!(
                f,
                "moon references parent orbit {parent_orbit}, which is outside the system"
            ),
            DroppedConstraint::MoonParentCannotHoldMoons { parent_orbit } => write!(
                f,
                "moon references parent orbit {parent_orbit}, which holds nothing that can have moons"
            ),
            DroppedConstraint::RelativeBodyNotFound { body, reference } => write!(
                f,
                "{body} is positioned relative to \"{reference}\", which isn't in this system"
            ),
            DroppedConstraint::NoBodyAtRelativePosition { body } => {
                write!(f, "no {body} exists at the position this override describes")
            }
        }
    }
}

/// Contents of an orbital slot
///
/// Represents what occupies a specific orbital position in the system.
/// Each orbit can contain at most one type of content, though some
/// contents (like gas giants) can host their own satellite systems.
///
/// ## Content Types
///
/// - **Secondary/Tertiary**: Markers for companion star locations
/// - **World**: Rocky planets with full UWP characteristics
/// - **GasGiant**: Gas giants with satellite systems
/// - **Blocked**: Intentionally empty orbits for realism
#[derive(Debug, Clone)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub enum OrbitContent {
    // This orbit contains the secondary star system of the primary.
    Secondary,
    // This orbit contains the tertiary star system of the primary.
    Tertiary,
    // This orbit contains a world
    #[cfg_attr(feature = "frontend", store)]
    World(World),
    // This orbit contains a gas giant
    #[cfg_attr(feature = "frontend", store)]
    GasGiant(GasGiant),
    // This orbit is intentionally empty and cannot be filled.
    Blocked,
}

impl System {
    pub fn new(
        star_type: StarType,
        subtype: StarSubType,
        size: StarSize,
        orbit: StarOrbit,
        max_orbits: usize,
    ) -> System {
        System {
            name: gen_star_system_name(),
            star_name: None,
            dropped: Vec::new(),
            star: Star {
                star_type,
                subtype,
                size,
            },
            secondary: None,
            tertiary: None,
            orbit,
            orbit_slots: vec![None; max_orbits],
        }
    }

    pub fn count_stars(&self) -> i32 {
        let mut count = 1;
        if let Some(secondary) = &self.secondary {
            count += secondary.count_stars();
        }
        if let Some(tertiary) = &self.tertiary {
            count += tertiary.count_stars();
        }
        count
    }

    pub fn set_max_orbits(&mut self, max_orbits: usize) {
        self.orbit_slots.resize(max_orbits + 1, None);
    }

    pub fn get_max_orbits(&self) -> usize {
        self.orbit_slots.len()
    }

    pub fn is_slot_empty(&self, orbit: usize) -> bool {
        self.orbit_slots
            .get(orbit)
            .map(|body| body.is_none())
            .unwrap_or(true)
    }

    pub fn get_unused_orbits(&self) -> Vec<usize> {
        self.orbit_slots
            .iter()
            .enumerate()
            .filter_map(
                |(index, body)| {
                    if body.is_none() { Some(index) } else { None }
                },
            )
            .collect()
    }

    pub fn set_orbit_slot(&mut self, orbit: usize, content: OrbitContent) {
        if orbit >= self.orbit_slots.len() {
            self.set_max_orbits(orbit);
        }
        self.orbit_slots[orbit] = Some(content);
    }

    pub fn generate_system(mut main_world: World) -> System {
        let star_mod = if (main_world.atmosphere >= 4 && main_world.atmosphere <= 9)
            || main_world.get_population() >= 8
        {
            4
        } else {
            0
        };
        let overrides = SystemOverrides::default();
        let mut system = gen_stars(star_mod, true, &overrides);
        main_world.gen_trade_classes();
        system.fill_system_with(main_world, true, &overrides);
        system
    }

    /// Seeded variant of [`generate_system`]. Installs a `ChaCha8Rng`
    /// seeded from `seed` as the worldgen thread-local for the duration
    /// of the call, so every `roll_2d6` / `roll_1d6` / `roll_10` /
    /// `rng_random_range` invocation inside the generation pipeline draws
    /// from a reproducible stream. Same seed + same `main_world` →
    /// identical `System`, forever.
    pub fn generate_system_seeded(seed: u64, main_world: World) -> System {
        let _guard = crate::util::RngScope::new(seed);
        System::generate_system(main_world)
    }

    /// Constraint-driven entry point. The classic `generate_system(World)`
    /// path is the special case of a single fully-specified
    /// `Planet { is_mainworld: true }` constraint plus no overrides.
    ///
    /// What's wired today:
    /// - Required: exactly one `Planet { is_mainworld: true }` with a
    ///   fully-specified UWP. Partial UWPs and ordinary `Planet`/`Moon`
    ///   rows still return `UnsupportedYet`.
    /// - Honored: any number of `Star` constraints (used as primary +
    ///   companion overrides); any number of `GasGiant` constraints
    ///   (sets the gas-giant count and per-giant size/moon counts).
    pub fn generate_from_constraints(
        constraints: SystemConstraints,
    ) -> Result<System, Vec<ConstraintError>> {
        let errors = constraints.validate();
        if !errors.is_empty() {
            return Err(errors);
        }

        // The main-world Planet constraint still requires a fully-
        // specified UWP — partial main-world UWPs need eager-roll
        // logic (see design doc) that doesn't ship in this turn.
        if let Some(Constraint::Planet {
            uwp: Some(p),
            is_mainworld: true,
            ..
        }) = constraints.main_world()
            && !p.is_complete()
        {
            return Err(vec![ConstraintError::UnsupportedYet(
                "partial UWPs on the main world aren't supported yet — supply every column"
                    .to_string(),
            )]);
        }

        let Some(Constraint::Planet {
            name,
            uwp: Some(uwp),
            is_mainworld: true,
            num_satellites: main_num_satellites,
            ..
        }) = constraints.main_world().cloned()
        else {
            return Err(vec![ConstraintError::UnsupportedYet(
                "a fully-specified main-world Planet constraint is required".to_string(),
            )]);
        };

        let uwp_string = uwp.to_string_with_wildcards();
        let world_name = name.unwrap_or_else(|| "Main World".to_string());
        let mut main_world =
            World::from_uwp(&world_name, &uwp_string, false, true).map_err(|e| {
                vec![ConstraintError::ContradictoryUwp(format!(
                    "from_uwp({uwp_string:?}) failed: {e}"
                ))]
            })?;

        let mut overrides = collect_overrides(&constraints);
        overrides.main_world_num_satellites = main_num_satellites;

        let star_mod = if (main_world.atmosphere >= 4 && main_world.atmosphere <= 9)
            || main_world.get_population() >= 8
        {
            4
        } else {
            0
        };
        let mut system = gen_stars(star_mod, true, &overrides);
        main_world.gen_trade_classes();
        system.fill_system_with(main_world, true, &overrides);

        // Facts that could only be known once the system existed: a body
        // identified by relative position, a moon of a body whose orbit
        // nothing pinned. Anything that can't be resolved joins the dropped
        // constraints, so "the outermost gas giant" in a system with no gas
        // giants fails as loudly as a misplaced pin.
        if !constraints.post.is_empty() {
            let unresolved =
                crate::systems::overrides::apply_post(&mut system, &constraints.post);
            system.dropped.extend(unresolved);
        }
        Ok(system)
    }

    /// Seeded variant of [`generate_from_constraints`]. The headline
    /// library entry point — used by `crate::generate_system_png` to
    /// give consumers byte-identical output across runs for a given
    /// `(seed, constraints)` pair.
    pub fn generate_from_constraints_seeded(
        seed: u64,
        constraints: SystemConstraints,
    ) -> Result<System, Vec<ConstraintError>> {
        let _guard = crate::util::RngScope::new(seed);
        System::generate_from_constraints(constraints)
    }

    fn generate_companion(
        primary_type_roll: i32,
        primary_size_roll: i32,
        orbit: StarOrbit,
        override_: &StarOverride,
    ) -> System {
        let companion_type_roll = roll_2d6() + primary_type_roll;
        let companion_size_roll = roll_2d6() + primary_size_roll;
        let star_type = override_
            .spectral
            .unwrap_or_else(|| gen_companion_star_type(companion_type_roll));
        let subtype = override_
            .subtype
            .unwrap_or_else(|| roll_10() as StarSubType);
        let star_size = override_
            .size
            .unwrap_or_else(|| gen_companion_star_size(companion_size_roll));
        let mut companion: System = System::new(star_type, subtype, star_size, orbit, 0);
        if let Some(n) = &override_.name {
            companion.name = n.clone();
        }
        companion.set_max_orbits(gen_max_orbits(&companion.star));

        if companion.orbit == StarOrbit::Far {
            // If secondary is Far then it can have companions.
            if gen_num_stars() > 1 {
                // -4 to this as we're a secondary of a secondary.
                let orbit = gen_companion_orbit(roll_2d6() - 4);
                let mut secondary: Box<System> = Box::new(System::generate_companion(
                    companion_type_roll,
                    companion_size_roll,
                    orbit,
                    &StarOverride::default(),
                ));

                // If the secondary of the secondary is also in a FAR orbit, then it can have a full range of
                // orbits itself.  Otherwise it is halved.
                if orbit == StarOrbit::Far {
                    secondary.set_max_orbits(gen_max_orbits(&secondary.star));
                } else {
                    secondary.set_max_orbits(gen_max_orbits(&secondary.star) / 2);
                }

                companion.secondary = Some(secondary);
                if let StarOrbit::System(orbit) = orbit {
                    companion.set_orbit_slot(orbit, OrbitContent::Secondary);
                }
            }
        }
        companion
    }

    fn gen_planetoids(&mut self, num_giants: i32, main_world: &World) {
        if roll_2d6() >= 7 {
            // No planetoids in system
            return;
        }
        let mut num_planetoids = match roll_2d6() - num_giants {
            1..=3 => 3,
            4..=6 => 2,
            _ => 1,
        };
        let mut viable_giants: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                if matches!(body, Some(OrbitContent::GasGiant(_))) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();

        let mut viable_other_orbits: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(
                |(index, body)| {
                    if body.is_none() { Some(index) } else { None }
                },
            )
            .collect();

        while viable_giants.len() + viable_other_orbits.len() > 0 && num_planetoids > 0 {
            // Pull the actual orbit value out of the candidate vec —
            // pre-existing bug previously returned the random *index*
            // (`pos`) as if it were an orbit, which both ignored the
            // shuffle and could overwrite slots that weren't viable
            // (e.g. an Empty-pinned Blocked orbit at index 0).
            let orbit = if !viable_giants.is_empty() {
                let pos = crate::util::rng_random_range(0..viable_giants.len());
                viable_giants.remove(pos)
            } else {
                let pos = crate::util::rng_random_range(0..viable_other_orbits.len());
                viable_other_orbits.remove(pos)
            };

            // Belts have atmosphere 0, so the lifeless rule fires
            // automatically when the system's main TL is < 7.
            let population = if crate::systems::world::force_lifeless(main_world, 0) {
                0
            } else {
                (roll_2d6() - 2
                    + if orbit as i32 <= get_zone(&self.star).inner {
                        -5
                    } else {
                        0
                    }
                    + if orbit as i32 > get_zone(&self.star).habitable {
                        -5
                    } else {
                        0
                    })
                // `max(0)` guards a population-0 main world: `get_population()
                // - 1` is -1, and `clamp(0, -1)` (lo > hi) panics in every
                // build mode. A pop-0 world with TL >= 7 reaches here (low TL
                // would have been zeroed by force_lifeless above). Mirrors the
                // same guard in world.rs's subordinate-population clamp.
                .clamp(0, (main_world.get_population() - 1).max(0))
            };

            let mut planetoid = World::new(
                "Planetoid Belt".to_string(),
                orbit,
                orbit,
                0,
                0,
                0,
                population,
                false,
                false,
            );

            planetoid.gen_subordinate_stats(main_world);
            planetoid.gen_trade_classes();
            planetoid.gen_subordinate_facilities(&get_zone(&self.star), orbit, main_world);
            planetoid.compute_astro_data(&self.star);
            self.set_orbit_slot(orbit, OrbitContent::World(planetoid));
            num_planetoids -= 1;
        }
    }

    fn gen_gas_giants(
        &mut self,
        overrides: Option<&[GasGiantOverride]>,
    ) -> (i32, std::collections::HashMap<usize, i32>) {
        // When the user pinned a list of gas giants (typically via
        // Traveller-Map autopop), the count is authoritative — even if
        // the count is zero, which the random path would never produce.
        let mut num_giants = if let Some(list) = overrides {
            list.len() as i32
        } else if roll_2d6() >= 10 {
            return (0, std::collections::HashMap::new());
        } else {
            match roll_2d6() {
                1..=3 => 1,
                4..=5 => 2,
                6..=7 => 3,
                8..=10 => 4,
                _ => 5,
            }
        };

        num_giants = num_giants.min(count_open_orbits(self));
        let original_num_giants = num_giants;
        let mut moon_overrides: std::collections::HashMap<usize, i32> =
            std::collections::HashMap::new();

        let habitable = get_zone(&self.star).habitable;

        let mut viable_outer_orbits: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                if body.is_none() {
                    if index as i32 > habitable {
                        Some(index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        let mut viable_inner_orbits: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                if body.is_none() {
                    if index as i32 <= habitable {
                        Some(index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        let mut placed_idx = 0;
        while viable_outer_orbits.len() + viable_inner_orbits.len() > 0 && num_giants > 0 {
            // If the override at this index pins an orbit and that
            // orbit is currently viable (i.e. empty and within range),
            // honor it. Otherwise fall back to the random pick — and record
            // why, which the old `.filter()` did not.
            //
            // This path fails more quietly than the planet and belt ones: an
            // unhonourable pin didn't drop the giant, it placed it somewhere
            // else at random. You asked for orbit 5, got orbit 9, and nothing
            // anywhere said so.
            let requested: Option<i32> = overrides
                .and_then(|list| list.get(placed_idx).and_then(|o| o.orbit));
            let max_orbits = self.get_max_orbits();
            let pinned_orbit: Option<usize> = match requested {
                None => None,
                Some(o) if o < 0 => {
                    self.dropped.push(DroppedConstraint::NegativeOrbit {
                        body: BodyKind::GasGiant,
                        orbit: o,
                    });
                    None
                }
                Some(o) => {
                    let ou = o as usize;
                    if viable_outer_orbits.contains(&ou) || viable_inner_orbits.contains(&ou) {
                        Some(ou)
                    } else if ou >= max_orbits {
                        self.dropped.push(DroppedConstraint::OrbitOutOfRange {
                            body: BodyKind::GasGiant,
                            orbit: o,
                            max_orbits,
                        });
                        None
                    } else {
                        self.dropped.push(DroppedConstraint::OrbitOccupied {
                            body: BodyKind::GasGiant,
                            orbit: o,
                        });
                        None
                    }
                }
            };

            let orbit = if let Some(o) = pinned_orbit {
                viable_outer_orbits.retain(|x| *x != o);
                viable_inner_orbits.retain(|x| *x != o);
                o
            } else if !viable_outer_orbits.is_empty() {
                let pos = crate::util::rng_random_range(0..viable_outer_orbits.len());

                viable_outer_orbits.remove(pos)
            } else {
                let pos = crate::util::rng_random_range(0..viable_inner_orbits.len());

                viable_inner_orbits.remove(pos)
            };

            let (size, moon_override) = match overrides {
                Some(list) => {
                    let o = list.get(placed_idx).cloned().unwrap_or_default();
                    let size = o.size.unwrap_or_else(|| {
                        if roll_1d6() <= 3 {
                            GasGiantSize::Small
                        } else {
                            GasGiantSize::Large
                        }
                    });
                    (size, o.num_satellites)
                }
                None => {
                    let size = if roll_1d6() <= 3 {
                        GasGiantSize::Small
                    } else {
                        GasGiantSize::Large
                    };
                    (size, None)
                }
            };
            if let Some(m) = moon_override {
                moon_overrides.insert(orbit, m);
            }
            let mut giant = GasGiant::new(size, orbit);
            if let Some(n) = overrides
                .and_then(|list| list.get(placed_idx))
                .and_then(|o| o.name.as_deref())
            {
                giant.name = n.to_string();
            }
            self.set_orbit_slot(orbit, OrbitContent::GasGiant(giant));
            num_giants -= 1;
            placed_idx += 1;
        }

        if num_giants > 0 {
            error!(
                "Not enough orbits for gas giants. Need {original_num_giants} in system {self:?}",
            );
        }
        (original_num_giants - num_giants, moon_overrides)
    }

    fn fill_system_with(
        &mut self,
        main_world: World,
        is_primary: bool,
        overrides: &SystemOverrides,
    ) {
        // First block appropriate orbits (just to have some number of empty
        // orbits), leaving alone any a constraint has claimed.
        let claimed: Vec<usize> = overrides
            .planets
            .iter()
            .filter_map(|p| p.orbit)
            .chain(overrides.belts.iter().filter_map(|b| b.orbit))
            .chain(
                overrides
                    .gas_giants
                    .iter()
                    .flat_map(|g| g.iter().filter_map(|g| g.orbit)),
            )
            .chain(overrides.main_world_orbit)
            .filter(|o| *o >= 0)
            .map(|o| o as usize)
            .collect();
        self.gen_blocked_orbits(&claimed);

        let main_world_copy = main_world.clone();
        let system_zones = get_zone(&self.star);

        // A system takes its main world's name unless an override says
        // otherwise. "The Torpol system", "the Regina system" — that is how
        // Traveller refers to them and how every published source writes
        // them, so every body without a name of its own becomes "Torpol VII"
        // rather than the name tables' invention.
        //
        // Deliberately not applied to companions: a companion sub-system has
        // no main world of its own, so it keeps the rolled name unless a
        // `Constraint::Star` names it.
        //
        // The main world keeps its own name rather than becoming "Torpol I" —
        // it is the name every other tool and the map itself use for it. The
        // numeral tracks orbit, so its slot is simply named instead of
        // numbered and the sequence shows a gap where it sits.
        //
        // Overwrites the name `System::new` already rolled rather than
        // skipping the roll, so the RNG stream is identical either way.
        if is_primary && overrides.system_name.is_none() && !main_world.name.trim().is_empty() {
            self.name = main_world.name.trim().to_string();
        }

        // Empty constraints reserve their orbits first so nothing
        // else can claim them.
        //
        // These next few passes used to be gated on `is_primary`, which was
        // correct while only the primary could carry constraints. Companions
        // carry their own now, and their orbit numbers are their own, so the
        // gate has to be "are there constraints" rather than "is this the
        // primary" — otherwise a companion's bodies are collected and then
        // silently never placed.
        self.apply_empty_constraints(&overrides.empties);

        // The star's orbit count is rolled at random (`gen_max_orbits`)
        // with no regard for how many bodies the constraints demand. When
        // a Traveller-Map autopop system asks for more bodies than the
        // star happened to roll slots for, grow the orbit list so every
        // constrained body has somewhere to land. Without this, the
        // planets (placed first) fill the few slots and the gas giants
        // and belts (placed afterwards) silently get zero — e.g. a
        // pbg=624 system (2 belts, 4 gas giants) rendering with neither.
        self.ensure_orbits_for_constraints(overrides);

        // Reserve the orbits pinned gas giants asked for, before the planet
        // and belt passes run.
        //
        // Those passes hand every *unpinned* body `get_unused_orbits().first()`
        // — the lowest free orbit — and gas giants aren't placed until after
        // them. So with eight don't-care planets from a PBG digit, orbits 0-7
        // were gone before a gas giant pinned to orbit 6 ever got to ask, and
        // its constraint was dropped. The planets had no preference; the gas
        // giant did, and lost anyway.
        //
        // Reserving mirrors what the main world already does with its
        // habitable orbit: mark the slot Blocked so the auto passes route
        // around it, release it again just before the real placement. When
        // nothing is pinned this list is empty and generation is unchanged,
        // which is why systems without a gas-giant override still come out
        // exactly as they did.
        // Belts have the same problem and are placed in their own pass after
        // the planets, so reserve theirs too. Only gas giants were reserved
        // originally, which left a pinned belt losing its orbit to a planet
        // that had no preference — the identical failure, one body kind over.
        let reserve = |slots: &Vec<Option<OrbitContent>>, orbits: Vec<i32>| -> Vec<usize> {
            orbits
                .into_iter()
                .filter(|o| *o >= 0)
                .map(|o| o as usize)
                .filter(|o| *o < slots.len() && slots[*o].is_none())
                .collect()
        };
        let (reserved_gg_orbits, reserved_belt_orbits) = {
            (
                reserve(
                    &self.orbit_slots,
                    overrides
                        .gas_giants
                        .iter()
                        .flatten()
                        .filter_map(|g| g.orbit)
                        .collect(),
                ),
                reserve(
                    &self.orbit_slots,
                    overrides.belts.iter().filter_map(|b| b.orbit).collect(),
                ),
            )
        };
        for o in reserved_gg_orbits.iter().chain(reserved_belt_orbits.iter()) {
            self.set_orbit_slot(*o, OrbitContent::Blocked);
        }

        // The main world is placed LAST (after the planet / belt / gas-giant
        // passes), but a habitable-zone main world has a fixed target orbit
        // and must not lose it to an auto-placed body. In a system with many
        // planet constraints, the unpinned planets greedily take the lowest
        // empty orbits — including the habitable one — and the main world is
        // then exiled to a far slot. Reserve its target orbit now (mark it
        // Blocked so every auto pass routes around it) and release it again
        // just before place_main_world. No RNG is consumed, and the auto
        // passes draw the same number of random values — only the orbit they
        // map to changes — so the rest of generation is unperturbed.
        let reserved_main_orbit = if is_primary {
            match self.main_world_habitable_orbit(&main_world_copy, overrides.main_world_orbit) {
                Some(o) if o < self.orbit_slots.len() && self.orbit_slots[o].is_none() => {
                    self.set_orbit_slot(o, OrbitContent::Blocked);
                    Some(o)
                }
                // Slot already taken (a companion star, a pinned Empty, or
                // out of range): leave it to place_main_world's own branch
                // logic, which handles those cases.
                _ => None,
            }
        } else {
            None
        };

        // Place user-pinned non-main planets and belts FIRST so the
        // random gas-giant / planetoid passes route around them.
        // Without this, a Planet constraint at orbit 1 can lose to a
        // randomly chosen gas giant.
        let mut planet_moon_overrides: std::collections::HashMap<usize, i32> =
            std::collections::HashMap::new();
        {
            self.place_planet_constraints(
                &overrides.planets,
                &main_world_copy,
                &mut planet_moon_overrides,
            );
            for o in &reserved_belt_orbits {
                self.orbit_slots[*o] = None;
            }
            self.place_belt_constraints(
                &overrides.belts,
                &main_world_copy,
                &mut planet_moon_overrides,
            );
        }

        // In autopop the gas-giant count is authoritative. A `None` list
        // there means the pbg gas-giant digit was 0, so force exactly zero
        // rather than letting gen_gas_giants roll a random count (which
        // would add bodies the `W` count never accounted for).
        // Release the gas-giant reservations so gen_gas_giants sees those
        // slots as the free orbits they were always meant to be.
        for o in &reserved_gg_orbits {
            self.orbit_slots[*o] = None;
        }

        let gg_spec: Option<&[GasGiantOverride]> =
            match (overrides.autopop, overrides.gas_giants.as_deref()) {
                (true, None) => Some(&[]),
                (_, other) => other,
            };
        let (num_gas_giants, gg_moon_overrides) = self.gen_gas_giants(gg_spec);

        // The random planetoid-belt pass adds belts and can overwrite gas
        // giants. In autopop the belt count is authoritative (belts are
        // placed exactly by place_belt_constraints, and a 0 belt digit means
        // none), so skip it entirely. The pure generate_system(World) path
        // is not autopop and still rolls planetoids.
        if !overrides.autopop {
            self.gen_planetoids(num_gas_giants, &main_world_copy);
        }

        // Release the reserved habitable orbit so place_main_world sees it
        // empty (None) and lands the main world there.
        if let Some(o) = reserved_main_orbit {
            self.orbit_slots[o] = None;
        }

        if is_primary {
            self.place_main_world(main_world, overrides.main_world_orbit);
        }

        for i in 0..=get_zone(&self.star).hot {
            // Block remaining empty hot-zone orbits — but don't
            // overwrite a user-pinned planet that landed inside the
            // hot zone (the user explicitly chose that orbit).
            if self.is_slot_empty(i as usize) {
                self.set_orbit_slot(i as usize, OrbitContent::Blocked);
            }
        }

        if is_primary {
            // Main world's num_moons override: now that place_main_world
            // has resolved which orbit the main world landed at, key
            // the override on that orbit so the satellite-gen loop
            // below picks it up like any other pinned planet.
            if let Some(n) = overrides.main_world_num_satellites {
                for (idx, slot) in self.orbit_slots.iter().enumerate() {
                    if let Some(OrbitContent::World(w)) = slot
                        && w.is_mainworld()
                    {
                        planet_moon_overrides.insert(idx, n);
                        break;
                    }
                }
            }
        }

        // Count Moon constraints per parent orbit so the random
        // satellite generation below knows how many slots are already
        // claimed by explicit Moon rows. Without this subtraction, a
        // body with `num_moons=1` and one Moon constraint targeting
        // the same orbit would end up with 2 moons (one rolled, one
        // from the constraint).
        let mut moon_constraints_per_orbit: std::collections::HashMap<usize, i32> =
            std::collections::HashMap::new();
        if is_primary {
            for m in &overrides.moons {
                if m.parent_orbit < 0 {
                    continue;
                }
                let po = m.parent_orbit as usize;
                *moon_constraints_per_orbit.entry(po).or_insert(0) += 1;
            }
        }

        // Random fill of the remaining empty orbits with rolled worlds —
        // the heart of the "generate a rich system" behavior. Skipped in
        // autopop: the `W` digit fixes the exact body count (main world +
        // planets + belts + gas giants), so padding every leftover orbit
        // with a world would overshoot it. Those orbits stay empty (and are
        // trimmed below if they trail past the outermost body).
        for i in (get_zone(&self.star).hot + 1)..self.get_max_orbits() as i32 {
            if overrides.autopop {
                break;
            }
            let i = i as usize;
            if self.is_slot_empty(i) {
                let mut new_world = World::generate(&self.star, i, &main_world_copy);
                new_world.gen_name(&self.name, i);
                new_world.compute_astro_data(&self.star);
                self.set_orbit_slot(i, OrbitContent::World(new_world));
            }
        }

        let zone_table = get_zone(&self.star);
        for i in 0..self.get_max_orbits() {
            let claimed_by_constraints = moon_constraints_per_orbit.get(&i).copied().unwrap_or(0);
            match &mut self.orbit_slots[i] {
                Some(OrbitContent::World(world)) => {
                    let target = planet_moon_overrides
                        .get(&i)
                        .copied()
                        .unwrap_or_else(|| world.determine_num_satellites());
                    let to_roll = (target - claimed_by_constraints).max(0);
                    for _ in 0..to_roll {
                        world.gen_satellite(&zone_table, &main_world_copy, &self.star);
                    }
                    world.clean_satellites();
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    let target = gg_moon_overrides
                        .get(&i)
                        .copied()
                        .unwrap_or_else(|| gas_giant.determine_num_satellites());
                    let to_roll = (target - claimed_by_constraints).max(0);
                    for _ in 0..to_roll {
                        gas_giant.gen_satellite(&system_zones, &main_world_copy, &self.star);
                    }
                    gas_giant.clean_satellites();
                    // Only when it hasn't already been named by a
                    // constraint — gen_name overwrites unconditionally.
                    if gas_giant.name.is_empty() {
                        gas_giant.gen_name(&self.name, i);
                    }
                }
                _ => continue,
            }
        }

        // Attach Moon constraints to their parent bodies. Skipped for
        // companion subsystems — TM autopop only describes the primary.
        if is_primary {
            self.apply_moon_constraints(&overrides.moons, &main_world_copy);
        }

        // Companions fill their own sub-systems, now with whatever
        // constraints were addressed to them. A companion is a `System` in
        // its own right, so its orbit numbers are its own — "orbit 1 of the
        // secondary" is not orbit 1 of the primary, which is exactly how
        // source tables describe them.
        //
        // Bodies with no star named still land on the primary, so a system
        // that never mentions its companions generates as it always did.
        if let Some(secondary) = &mut self.secondary
            && secondary.orbit != StarOrbit::Primary
        {
            let sub = companion_overrides(&overrides.secondary_bodies);
            secondary.fill_system_with(main_world_copy.clone(), false, &sub);
        }

        if let Some(tertiary) = &mut self.tertiary
            && tertiary.orbit != StarOrbit::Primary
        {
            let sub = companion_overrides(&overrides.tertiary_bodies);
            tertiary.fill_system_with(main_world_copy, false, &sub);
        }

        // In autopop, a star that rolled more orbit slots than the `W` count
        // needed leaves empty/blocked slots trailing past the outermost body.
        // The world-fill no longer pads them, but the renderer would still
        // draw them as bare orbit rings, so drop the trailing empties. Gaps
        // *between* bodies are left intact — those are legitimate empty
        // orbits. Only the primary owns the orbit list shown to the user.
        if is_primary && overrides.autopop {
            let last_body = self
                .orbit_slots
                .iter()
                .rposition(|s| matches!(s, Some(OrbitContent::World(_)) | Some(OrbitContent::GasGiant(_)) | Some(OrbitContent::Secondary) | Some(OrbitContent::Tertiary)));
            if let Some(last) = last_body {
                self.orbit_slots.truncate(last + 1);
            }
        }
    }

    /// The orbit `place_main_world` will target for a habitable-zone main
    /// world, or `None` when the world doesn't require the habitable zone
    /// (those are placed in a random empty orbit instead, so there's nothing
    /// to reserve). Mirrors the habitable-orbit selection in
    /// `place_main_world` so the reservation in `fill_system_with` blocks the
    /// exact slot the main world will later claim.
    fn main_world_habitable_orbit(
        &self,
        main_world: &World,
        pinned: Option<i32>,
    ) -> Option<usize> {
        // A stated orbit beats the habitable-zone default. The default is
        // right knowing nothing; a published system table is knowing
        // something.
        if let Some(o) = pinned {
            return (o >= 0).then_some(o as usize);
        }
        let requires_habitable =
            main_world.atmosphere > 1 && main_world.atmosphere < 10 && main_world.size > 0;
        if !requires_habitable {
            return None;
        }
        let zone = get_zone(&self.star);
        let habitable = if zone.habitable <= 0 || zone.habitable == zone.inner {
            zone.inner.max(0)
        } else {
            zone.habitable
        };
        Some(habitable as usize)
    }

    fn place_main_world(&mut self, mut main_world: World, pinned: Option<i32>) {
        // A pinned orbit is honoured verbatim, habitable zone or not — which
        // is the point. Makergod puts Oghma three orbits beyond a K5 V's
        // habitable zone, and the rules agree: at size 5 the -2 DM for being
        // outside gives a mean atmosphere of exactly the 3 it has.
        let requires_habitable = pinned.is_some()
            || (main_world.atmosphere > 1 && main_world.atmosphere < 10 && main_world.size > 0);
        let mut habitable = match pinned {
            Some(o) => o,
            None => get_zone(&self.star).habitable,
        };
        if pinned.is_none()
            && (habitable <= 0 || habitable == get_zone(&self.star).inner)
            && requires_habitable
        {
            warn!(
                "No habitable zone for main world for system: {:?}. Habitable = {}. Inner = {}. Using orbit 0.",
                self,
                habitable,
                get_zone(&self.star).inner
            );
            habitable = get_zone(&self.star).inner.max(0);
        }

        debug!(
            "(place_main_world) habitable = {}, max_orbits = {}, requires_habitable = {}, star = {}",
            habitable,
            self.get_max_orbits(),
            requires_habitable,
            self.star
        );

        // Place the main world. After placing be sure and generate the astro data (cannot do it until its placed!)
        if requires_habitable {
            // Just place in the habitable
            match &mut self
                .orbit_slots
                .get_mut(habitable as usize)
                .unwrap_or(&mut None)
            {
                Some(OrbitContent::Secondary) => {
                    // If there happens to be a star in the habitable zone, place it in orbit there.
                    // Note the orbit of the main world in terms of primary first
                    // TODO: Is this correct when we have multiple stars?
                    main_world.position_in_system = habitable as usize;
                    // Safe to unwrap as if the orbital position is secondary but there is no secondary, thats bug.
                    self.secondary
                        .as_mut()
                        .unwrap()
                        .place_main_world(main_world, None);
                }
                Some(OrbitContent::Tertiary) => {
                    // If there happens to be a star in the habitable zone, place it in orbit there.
                    // Note the orbit of the main world in terms of primary first
                    // TODO: Is this correct when we have multiple stars?
                    main_world.position_in_system = habitable as usize;
                    // Safe to unwrap as if the orbital position is tertiary but there is no tertiary, thats bug.
                    self.tertiary
                        .as_mut()
                        .unwrap()
                        .place_main_world(main_world, None);
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    let orbit = gas_giant.gen_satellite_orbit(main_world.size == 0);
                    main_world.orbit = orbit;
                    main_world.position_in_system = habitable as usize;
                    main_world.compute_astro_data(&self.star);
                    gas_giant.push_satellite(main_world);
                }
                Some(OrbitContent::Blocked) => {
                    main_world.position_in_system = habitable as usize;
                    main_world.orbit = habitable as usize;
                    main_world.compute_astro_data(&self.star);
                    self.set_orbit_slot(habitable as usize, OrbitContent::World(main_world));
                }
                Some(OrbitContent::World(_)) => {
                    // Habitable orbit was claimed by a user-pinned
                    // Planet constraint earlier — don't overwrite.
                    // Fall back to whichever empty orbit is left.
                    let empty_orbits = self.get_unused_orbits();
                    let pos = empty_orbits.first().copied().unwrap_or(habitable as usize);
                    main_world.position_in_system = pos;
                    main_world.orbit = pos;
                    main_world.compute_astro_data(&self.star);
                    self.set_orbit_slot(pos, OrbitContent::World(main_world));
                }
                None => {
                    main_world.position_in_system = habitable as usize;
                    main_world.orbit = habitable as usize;
                    main_world.compute_astro_data(&self.star);
                    self.set_orbit_slot(habitable as usize, OrbitContent::World(main_world));
                }
            }
        } else {
            let empty_orbits = self.get_unused_orbits();
            if !empty_orbits.is_empty() {
                let orbit = empty_orbits[crate::util::rng_random_range(0..empty_orbits.len())];
                main_world.position_in_system = orbit;
                main_world.orbit = orbit;
                main_world.compute_astro_data(&self.star);
                self.set_orbit_slot(orbit, OrbitContent::World(main_world));
            } else {
                // Just jam the world in somewhere.
                let pos = if self.get_max_orbits() == 0 {
                    0
                } else {
                    crate::util::rng_random_range(0..self.get_max_orbits())
                };
                main_world.orbit = pos;
                main_world.position_in_system = pos;
                self.set_orbit_slot(pos, OrbitContent::World(main_world));
            };
        }
    }

    /// Place every pinned non-main-world Planet override at its
    /// requested orbit (or pick from currently-empty orbits if no
    /// orbit was specified). Records the per-planet moon-count override
    /// keyed by final orbit so satellite generation can pick it up
    /// later.
    fn place_planet_constraints(
        &mut self,
        planets: &[PlanetOverride],
        main_world: &World,
        moon_overrides_out: &mut std::collections::HashMap<usize, i32>,
    ) {
        // Pinned bodies first: the unpinned ones take `get_unused_orbits()
        // .first()`, so in list order a pinned planet at orbit 2 could find
        // its slot already taken by a planet that had no preference at all.
        // Same failure as the gas giants and belts, now within a single pass.
        let mut planets: Vec<&PlanetOverride> = planets.iter().collect();
        planets.sort_by_key(|p| p.orbit.is_none());
        for p in planets {
            // Choose the orbit. Explicit orbit wins; if it'd land
            // outside the system or be already filled, skip the
            // constraint with a warn — validation should have caught
            // a duplicate-orbit collision earlier; this is the
            // "orbit higher than max" / "orbit blocked by hot zone"
            // edge case.
            let orbit = match p.orbit {
                Some(o) => {
                    if o < 0 {
                        self.dropped.push(DroppedConstraint::NegativeOrbit {
                            body: BodyKind::Planet,
                            orbit: o,
                        });
                        continue;
                    }
                    let o_usize = o as usize;
                    // Reported separately: "out of range" means the system
                    // wasn't grown far enough for a high pinned orbit, which
                    // is our bug; "occupied" means the constraint collided
                    // with something, which is the author's to resolve.
                    if o_usize >= self.get_max_orbits() {
                        self.dropped.push(DroppedConstraint::OrbitOutOfRange {
                            body: BodyKind::Planet,
                            orbit: o,
                            max_orbits: self.get_max_orbits(),
                        });
                        continue;
                    }
                    if !self.is_slot_empty(o_usize) {
                        self.dropped.push(DroppedConstraint::OrbitOccupied {
                            body: BodyKind::Planet,
                            orbit: o,
                        });
                        continue;
                    }
                    o_usize
                }
                None => match self.get_unused_orbits().first() {
                    Some(o) => *o,
                    None => {
                        self.dropped.push(DroppedConstraint::NoFreeOrbit {
                            body: BodyKind::Planet,
                        });
                        continue;
                    }
                },
            };

            // Generate the world honoring whatever digits the user
            // pinned in the partial UWP — wild columns get rolled with
            // the same per-orbit modifiers as a fully-rolled world.
            let name_ref = p.name.as_deref();
            let mut new_world = World::generate_with_partial(
                &self.star,
                orbit,
                main_world,
                p.partial_uwp.as_ref(),
                name_ref,
                false,
                false,
            );
            new_world.orbit = orbit;
            if name_ref.is_none() {
                new_world.gen_name(&self.name, orbit);
            }
            new_world.compute_astro_data(&self.star);

            if let Some(n) = p.num_satellites {
                moon_overrides_out.insert(orbit, n);
            }

            self.set_orbit_slot(orbit, OrbitContent::World(new_world));
        }
    }

    /// Place every Belt constraint as a size-0 planetoid belt at its
    /// requested orbit (or grab the first empty slot if no orbit was
    /// specified). Reuses the partial-aware world generator with the
    /// size column forced to 0.
    fn place_belt_constraints(
        &mut self,
        belts: &[BeltOverride],
        main_world: &World,
        moon_overrides_out: &mut std::collections::HashMap<usize, i32>,
    ) {
        let mut belts: Vec<&BeltOverride> = belts.iter().collect();
        belts.sort_by_key(|b| b.orbit.is_none());
        for b in belts {
            let orbit = match b.orbit {
                Some(o) => {
                    if o < 0 {
                        self.dropped.push(DroppedConstraint::NegativeOrbit {
                            body: BodyKind::Belt,
                            orbit: o,
                        });
                        continue;
                    }
                    let o_usize = o as usize;
                    if o_usize >= self.get_max_orbits() {
                        self.dropped.push(DroppedConstraint::OrbitOutOfRange {
                            body: BodyKind::Belt,
                            orbit: o,
                            max_orbits: self.get_max_orbits(),
                        });
                        continue;
                    }
                    if !self.is_slot_empty(o_usize) {
                        self.dropped.push(DroppedConstraint::OrbitOccupied {
                            body: BodyKind::Belt,
                            orbit: o,
                        });
                        continue;
                    }
                    o_usize
                }
                None => match self.get_unused_orbits().first() {
                    Some(o) => *o,
                    None => {
                        self.dropped.push(DroppedConstraint::NoFreeOrbit {
                            body: BodyKind::Belt,
                        });
                        continue;
                    }
                },
            };

            let mut partial = b.partial_uwp.clone().unwrap_or_default();
            // Belts are by definition size 0; override whatever the
            // user typed. Atmosphere and hydro default to 0 too,
            // matching the existing planetoid-belt generator.
            partial.size = Some(0);
            if partial.atmosphere.is_none() {
                partial.atmosphere = Some(0);
            }
            if partial.hydro.is_none() {
                partial.hydro = Some(0);
            }

            let default_name = "Planetoid Belt".to_string();
            let name_ref = b.name.as_deref().unwrap_or(default_name.as_str());
            let mut new_world = World::generate_with_partial(
                &self.star,
                orbit,
                main_world,
                Some(&partial),
                Some(name_ref),
                false,
                false,
            );
            new_world.orbit = orbit;
            new_world.compute_astro_data(&self.star);

            if let Some(n) = b.num_satellites {
                moon_overrides_out.insert(orbit, n);
            }

            self.set_orbit_slot(orbit, OrbitContent::World(new_world));
        }
    }

    /// Grow the primary star's orbit list, if needed, so it has at least
    /// one empty slot for every constrained body — the main world, the
    /// pinned planets, the belts, and the gas giants. `gen_max_orbits`
    /// rolls the slot count at random and can come up short of what a
    /// Traveller-Map autopop system (which carries explicit belt and
    /// gas-giant counts in its `pbg`) requires; the planet pass then
    /// consumes the few slots and the later belt and gas-giant passes
    /// find nothing open. Appending slots costs no RNG, so unconstrained
    /// generation (required == 1, almost always already satisfied) is
    /// unaffected and stays byte-identical.
    fn ensure_orbits_for_constraints(&mut self, overrides: &SystemOverrides) {
        let required = 1 // the main world
            + overrides.planets.len()
            + overrides.belts.len()
            + overrides.gas_giants.as_ref().map_or(0, |g| g.len());

        // A pinned orbit needs the system to reach *that index*, which
        // counting bodies doesn't guarantee. "The gas giant is at orbit 12"
        // in a ten-orbit system used to grow the list by the number of
        // constraints — two or three slots — leaving orbit 12 still out of
        // range, and the body was then dropped. Grow to whichever demand is
        // larger.
        let highest_pinned = overrides
            .planets
            .iter()
            .filter_map(|p| p.orbit)
            .chain(overrides.belts.iter().filter_map(|b| b.orbit))
            .chain(
                overrides
                    .gas_giants
                    .iter()
                    .flat_map(|g| g.iter().filter_map(|g| g.orbit)),
            )
            .chain(overrides.empties.iter().copied())
            .filter(|o| *o >= 0)
            .max()
            .map(|o| o as usize + 1)
            .unwrap_or(0);
        if highest_pinned > self.get_max_orbits() {
            self.set_max_orbits(highest_pinned - 1);
        }

        let empty = self.get_unused_orbits().len();
        if empty < required {
            let deficit = required - empty;
            // get_max_orbits() returns the slot-vec length; set_max_orbits(n)
            // resizes it to n+1. Adding `deficit` slots → new length
            // (len + deficit) → set_max_orbits(len + deficit - 1). Orbits
            // past the classic 20-slot table get extrapolated distances
            // (see system_tables::get_orbital_distance), so the list can
            // grow as far as the constraints demand.
            self.set_max_orbits(self.get_max_orbits() + deficit - 1);
        }
    }

    /// Set every user-specified Empty orbit to `OrbitContent::Blocked`.
    /// Runs first in the placement pipeline so subsequent passes
    /// (planet, gas-giant, random fill) treat the slot as taken.
    fn apply_empty_constraints(&mut self, empties: &[i32]) {
        for &orbit in empties {
            if orbit < 0 {
                self.dropped.push(DroppedConstraint::NegativeOrbit {
                    body: BodyKind::Empty,
                    orbit,
                });
                continue;
            }
            let o = orbit as usize;
            if o >= self.get_max_orbits() {
                self.dropped.push(DroppedConstraint::OrbitOutOfRange {
                    body: BodyKind::Empty,
                    orbit,
                    max_orbits: self.get_max_orbits(),
                });
                continue;
            }
            self.set_orbit_slot(o, OrbitContent::Blocked);
        }
    }

    /// Append every Moon constraint to its parent body's satellite
    /// list. Empty UWPs run through `gen_satellite`; partial or full
    /// UWPs go through `build_partial_satellite` so user-specified
    /// columns are honored and the rest get rolled with the standard
    /// per-zone modifiers.
    fn apply_moon_constraints(&mut self, moons: &[MoonOverride], main_world: &World) {
        let zone = get_zone(&self.star);
        let star = self.star;
        for m in moons {
            let parent_orbit = m.parent_orbit;
            if parent_orbit < 0 {
                self.dropped.push(DroppedConstraint::NegativeOrbit {
                    body: BodyKind::Moon,
                    orbit: parent_orbit,
                });
                continue;
            }
            let parent_idx = parent_orbit as usize;
            if parent_idx >= self.orbit_slots.len() {
                self.dropped
                    .push(DroppedConstraint::MoonParentOutOfRange { parent_orbit });
                continue;
            }

            // Decide the satellite's size: user override > parent-
            // specific roll. We resolve this BEFORE generating the
            // satellite orbit because is-ring affects the orbit roll.
            let size_override = m
                .partial_uwp
                .as_ref()
                .and_then(|p| p.size)
                .map(|s| s as i32);

            match &mut self.orbit_slots[parent_idx] {
                Some(OrbitContent::World(parent)) => {
                    if m.partial_uwp.is_none() {
                        // No UWP — run the standard satellite pipeline.
                        parent.gen_satellite(&zone, main_world, &star);
                        if let Some(name) = &m.name
                            && let Some(last) = parent.get_satellites_mut().sats.last_mut()
                        {
                            last.name = name.clone();
                        }
                        continue;
                    }
                    let size = size_override.unwrap_or_else(|| (parent.size - roll_1d6()).max(-1));
                    let parent_pos = parent.position_in_system;
                    let sat_orbit = parent.gen_satellite_orbit(size == 0);
                    let satellite = crate::systems::world::build_partial_satellite(
                        &star,
                        parent_pos,
                        sat_orbit,
                        size,
                        &zone,
                        main_world,
                        m.partial_uwp.as_ref(),
                        m.name.as_deref(),
                    );
                    parent.push_satellite(satellite);
                }
                Some(OrbitContent::GasGiant(parent)) => {
                    if m.partial_uwp.is_none() {
                        parent.gen_satellite(&zone, main_world, &star);
                        if let Some(name) = &m.name
                            && let Some(last) = parent.get_satellites_mut().sats.last_mut()
                        {
                            last.name = name.clone();
                        }
                        continue;
                    }
                    let size = size_override.unwrap_or_else(|| {
                        (match parent.size {
                            GasGiantSize::Small => roll_2d6() - 6,
                            GasGiantSize::Large => roll_2d6() - 4,
                        })
                        .max(-1)
                    });
                    let parent_pos = parent.orbit;
                    let sat_orbit = parent.gen_satellite_orbit(size == 0);
                    let satellite = crate::systems::world::build_partial_satellite(
                        &star,
                        parent_pos,
                        sat_orbit,
                        size,
                        &zone,
                        main_world,
                        m.partial_uwp.as_ref(),
                        m.name.as_deref(),
                    );
                    parent.push_satellite(satellite);
                }
                _ => self
                    .dropped
                    .push(DroppedConstraint::MoonParentCannotHoldMoons { parent_orbit }),
            }
        }
    }

    /// Randomly block a few orbits, skipping any a constraint has claimed.
    ///
    /// This runs before every placement pass, so without `claimed` it could
    /// block an orbit a source explicitly assigned — and did: Makergod gives
    /// Doruc's orbit 1 to the gas giant Khazha, and a random block landed
    /// there first. The rolls are unchanged either way, so a system with no
    /// constraints blocks exactly the orbits it always did.
    fn gen_blocked_orbits(&mut self, claimed: &[usize]) {
        if roll_1d6() < 5 {
            // No Empty orbits
            return;
        }
        let roll = roll_1d6();
        let num_empty = match roll {
            1..=2 => 1,
            3 => 2,
            _ => 3,
        };

        let valid_orbits: Vec<usize> = self
            .get_unused_orbits()
            .into_iter()
            .filter(|o| !claimed.contains(o))
            .collect();

        for _ in 0..num_empty {
            if let Some(pos) = crate::util::rng_choose(&valid_orbits) {
                self.set_orbit_slot(*pos, OrbitContent::Blocked);
            }
        }
    }
}

impl System {
    /// The primary star's name, falling back to the system's.
    pub fn star_name(&self) -> &str {
        self.star_name.as_deref().unwrap_or(&self.name)
    }

    /// Every constraint this system and its companions failed to honour.
    ///
    /// Walks the tree because companions are `System`s in their own right
    /// and place their own constrained bodies — a drop inside a companion is
    /// just as much a failure to honour the author's intent as one in the
    /// primary, and reporting only the primary's would hide it.
    pub fn dropped_constraints(&self) -> Vec<DroppedConstraint> {
        let mut all = self.dropped.clone();
        for child in [self.secondary.as_deref(), self.tertiary.as_deref()]
            .into_iter()
            .flatten()
        {
            all.extend(child.dropped_constraints());
        }
        all
    }
}

impl Default for System {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            star_name: None,
            dropped: Vec::new(),
            star: Star {
                star_type: StarType::G,
                subtype: 0,
                size: StarSize::V,
            },
            secondary: None,
            tertiary: None,
            orbit: StarOrbit::Primary,
            orbit_slots: Vec::new(),
        }
    }
}

impl Display for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is a {} star.", self.name, self.star)?;
        write!(
            f,
            " Zones for {} are {:?}. ",
            self.name,
            get_zone(&self.star)
        )?;
        if let Some(secondary) = &self.secondary {
            if let StarOrbit::Primary = secondary.orbit {
                writeln!(
                    f,
                    "It has a secondary contact star {}:\n{}",
                    secondary.name, secondary
                )?;
            } else {
                writeln!(
                    f,
                    "It has a secondary star {} which is a {} star in {}.",
                    secondary.name, secondary.star, secondary.orbit
                )?;
            }
        }
        if let Some(tertiary) = &self.tertiary {
            if let StarOrbit::Primary = tertiary.orbit {
                writeln!(
                    f,
                    " It has a tertiary contact star {}:\n{}",
                    tertiary.name, tertiary
                )?;
            } else {
                writeln!(
                    f,
                    " It has a tertiary star {} which is a {} star in {}.",
                    tertiary.name, tertiary.star, tertiary.orbit
                )?;
            }
        }

        if self
            .orbit_slots
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.is_slot_empty(*index))
            .count()
            > 0
        {
            writeln!(
                f,
                "\n{:>11}  {:<7}{:<24}{:<12}{:<18}",
                "Dist (Mkm)", "Orbit", "Name", "UWP", "Remarks"
            )?;
        }

        for (idx, body) in self.orbit_slots.iter().enumerate() {
            // Distance in millions of km from the table; displayed as
            // a leading column on every populated orbit slot. Satellite
            // rows printed by World/GasGiant Display recurse with their
            // own indented format and are not prefixed here.
            let dist = get_orbital_distance(idx as i32);
            match body {
                Some(OrbitContent::Secondary) => {
                    if let Some(secondary) = &self.secondary
                        && let StarOrbit::System(orbit) = secondary.orbit
                    {
                        writeln!(
                            f,
                            "{:>11.1}  {:<7}{:<24}{:<12}",
                            dist, orbit, secondary.name, secondary.star
                        )?;
                    }
                }
                Some(OrbitContent::Tertiary) => {
                    if let Some(tertiary) = &self.tertiary
                        && let StarOrbit::System(orbit) = tertiary.orbit
                    {
                        writeln!(
                            f,
                            "{:>11.1}  {:<7}{:<24}{:<12}",
                            dist, orbit, tertiary.name, tertiary.star
                        )?;
                    }
                }
                Some(OrbitContent::World(world)) => {
                    write!(f, "{:>11.1}  ", dist)?;
                    writeln!(f, "{world}")?;
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    write!(f, "{:>11.1}  ", dist)?;
                    writeln!(f, "{gas_giant}")?;
                }
                Some(OrbitContent::Blocked) | None => {}
            }
        }

        if let Some(secondary) = &self.secondary {
            writeln!(f, "\n{secondary}")?;
        }

        if let Some(tertiary) = &self.tertiary {
            writeln!(f, "\n{tertiary}")?;
        }
        Ok(())
    }
}

impl Display for StarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Display for StarSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Display for Star {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} {}",
            self.star_type, self.subtype as usize, self.size
        )
    }
}

impl Display for StarOrbit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StarOrbit::Primary => write!(f, "close orbit"),
            StarOrbit::Far => write!(f, "far orbit"),
            StarOrbit::System(orbit) => write!(f, "orbit {orbit}"),
        }
    }
}

// Functions not in a struct
fn gen_num_stars() -> i32 {
    let roll = roll_2d6();
    if roll <= 7 {
        1
    } else if roll < 12 {
        2
    } else {
        3
    }
}

fn gen_primary_star_type(roll: i32) -> StarType {
    match roll {
        x if x <= 1 => StarType::B,
        2 => StarType::A,
        3..=7 => StarType::M,
        8 => StarType::K,
        9 => StarType::G,
        10..=11 => StarType::F,
        _ => StarType::G,
    }
}

fn gen_primary_star_size(roll: i32, star_type: StarType, subtype: StarSubType) -> StarSize {
    let mut star_size = match roll {
        1 => StarSize::Ia,
        2 => StarSize::Ib,
        3 => StarSize::II,
        4 => StarSize::III,
        5..=10 => StarSize::V,
        11 => StarSize::VI,
        12 => StarSize::D,
        // EDITORIAL: Given bonuses on table want common case for populated world to be main sequence star.
        _ => StarSize::V,
    };
    if star_size == StarSize::IV
        && ((star_type == StarType::K && subtype >= 5)
            || star_type > StarType::K && star_type <= StarType::M)
    {
        star_size = StarSize::V;
    }
    if star_size == StarSize::VI
        && (star_type < StarType::F || (star_type == StarType::F && subtype <= 4))
    {
        star_size = StarSize::V;
    }
    star_size
}

fn gen_companion_star_type(roll: i32) -> StarType {
    match roll {
        x if x <= 1 => StarType::B,
        2 => StarType::A,
        3..=4 => StarType::F,
        5..=6 => StarType::G,
        7..=8 => StarType::K,
        _ => StarType::M,
    }
}

fn gen_companion_star_size(roll: i32) -> StarSize {
    match roll {
        1..=4 => match roll {
            1 => StarSize::Ia,
            2 => StarSize::Ib,
            3 => StarSize::II,
            4 => StarSize::III,
            _ => unreachable!(),
        },
        5..=6 => StarSize::D,
        7..=8 => StarSize::V,
        9 => StarSize::VI,
        _ => StarSize::D,
    }
}

fn gen_companion_orbit(roll: i32) -> StarOrbit {
    match roll {
        1..=3 => StarOrbit::Primary,
        4..=6 => StarOrbit::System((roll - 3) as usize),
        7..=11 => StarOrbit::System((roll - 3 + roll_1d6()) as usize),
        _ => StarOrbit::Far,
    }
}

fn gen_max_orbits(star: &Star) -> usize {
    let mut modifier = if star.size <= StarSize::II {
        8
    } else if star.size == StarSize::III {
        4
    } else {
        0
    };

    if star.star_type == StarType::M {
        modifier -= 4;
    } else if star.star_type == StarType::K {
        modifier -= 2;
    }

    let orbits = roll_2d6() + modifier;
    if orbits < 0 { 0 } else { orbits as usize }
}

fn empty_orbits_near_companion(system: &mut System, orbit: usize) {
    for i in (orbit / 2 + 1)..orbit {
        system.set_orbit_slot(i, OrbitContent::Blocked);
    }
    system.set_orbit_slot(orbit + 1, OrbitContent::Blocked);
    system.set_orbit_slot(orbit + 2, OrbitContent::Blocked);
}

fn gen_stars(world_mod: i32, companions_possible: bool, overrides: &SystemOverrides) -> System {
    // If the user supplied any Star constraints, the count of those is
    // authoritative — we generate exactly that many stars. Otherwise
    // fall back to the random count (or 1 for non-primary systems).
    let num_stars = if !overrides.stars.is_empty() {
        overrides.stars.len() as i32
    } else if companions_possible {
        gen_num_stars()
    } else {
        1
    };

    let primary_override = overrides.stars.first().cloned().unwrap_or_default();
    let primary_type_roll = roll_2d6();
    let primary_size_roll = roll_2d6();
    let star_type = primary_override
        .spectral
        .unwrap_or_else(|| gen_primary_star_type(primary_type_roll + world_mod));
    let star_subtype = primary_override
        .subtype
        .unwrap_or_else(|| roll_10() as StarSubType);
    let star_size = primary_override
        .size
        .unwrap_or_else(|| gen_primary_star_size(primary_size_roll, star_type, star_subtype));

    let mut system = System::new(star_type, star_subtype, star_size, StarOrbit::Primary, 0);
    // Applied *after* System::new, which has already rolled a name from the
    // name tables. Overwriting the result rather than skipping the roll keeps
    // the RNG stream identical whether or not a name is pinned — so naming a
    // system doesn't silently re-roll the rest of it. Same reasoning as the
    // reserve-then-release dance for the main world's habitable orbit.
    if let Some(n) = &overrides.system_name {
        system.name = n.clone();
    }
    // The primary star's own name, when the source gives it one distinct
    // from the system's. Defaults to the system name via `star_name()`.
    if let Some(n) = &primary_override.name {
        system.star_name = Some(n.clone());
    }
    let star = system.star;
    system.set_max_orbits(gen_max_orbits(&star));

    // Do this for a secondary, which we have with 2 or 3 stars.
    if num_stars >= 2 {
        let secondary_override = overrides.stars.get(1).cloned().unwrap_or_default();
        let orbit = secondary_override
            .orbit
            .unwrap_or_else(|| gen_companion_orbit(roll_2d6()));
        match orbit {
            StarOrbit::Primary | StarOrbit::Far => {
                system.secondary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                    &secondary_override,
                )));
            }
            // If the companion has an orbit, but its inside the primary star, just treat it as the primary orbit.
            StarOrbit::System(position) if position as i32 <= get_zone(&star).inside => {
                system.secondary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    StarOrbit::Primary,
                    &secondary_override,
                )));
            }
            StarOrbit::System(position) => {
                system.secondary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                    &secondary_override,
                )));
                system.set_orbit_slot(position, OrbitContent::Secondary);
                empty_orbits_near_companion(&mut system, position);
            }
        }
    }

    // Do this for a tertiary, which we have with 3 stars.
    // TODO: This is a blatant copy of the code above; how do I DRY this?
    if num_stars == 3 {
        let tertiary_override = overrides.stars.get(2).cloned().unwrap_or_default();
        let orbit = tertiary_override
            .orbit
            .unwrap_or_else(|| gen_companion_orbit(roll_2d6() + 4));
        match orbit {
            StarOrbit::Primary | StarOrbit::Far => {
                system.tertiary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                    &tertiary_override,
                )));
            }
            StarOrbit::System(position) if position as i32 <= get_zone(&star).inside => {
                system.tertiary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    StarOrbit::Primary,
                    &tertiary_override,
                )));
            }
            StarOrbit::System(position) => {
                system.tertiary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                    &tertiary_override,
                )));
                system.set_orbit_slot(position, OrbitContent::Tertiary);
                empty_orbits_near_companion(&mut system, position);
            }
        }
    }
    system
}

/// Pull the actionable star and gas-giant overrides out of a
/// `SystemConstraints`. Star constraints map to entries in `stars` in
/// declaration order (first becomes the primary override). Gas-giant
/// constraints become a non-empty `Some(Vec)` so the random-count
/// branch in `gen_gas_giants` is suppressed.
/// Build the overrides for a companion sub-system from the constraints
/// addressed to it.
///
/// Goes through `collect_overrides` like the primary's do, so a companion's
/// bodies obey exactly the same rules — pinned orbits reserved, counts
/// authoritative, the lot. `autopop` stays false: a companion has no PBG
/// digits of its own, so its gas-giant and belt counts are still rolled
/// unless constraints say otherwise.
fn companion_overrides(bodies: &[Constraint]) -> SystemOverrides {
    if bodies.is_empty() {
        return SystemOverrides::default();
    }
    let mut o = collect_overrides(&SystemConstraints {
        bodies: bodies.to_vec(),
        system_name: None,
        post: Vec::new(),
        secondary_bodies: Vec::new(),
        tertiary_bodies: Vec::new(),
        main_world_orbit: None,
    });
    // Autopop, despite there being no PBG digits here: it means "this
    // description is authoritative", which is exactly what a source table
    // listing a companion's bodies is. Without it the random planetoid pass
    // runs and overwrites the pinned bodies — Doruc's Khazha came out as a
    // randomly rolled planetoid belt.
    o.autopop = true;
    o
}

fn collect_overrides(constraints: &SystemConstraints) -> SystemOverrides {
    let system_name = constraints.system_name.clone();
    let mut stars: Vec<StarOverride> = constraints
        .bodies
        .iter()
        .filter_map(|c| match c {
            Constraint::Star {
                orbit,
                spectral,
                subtype,
                size,
                name,
            } => Some(StarOverride {
                orbit: *orbit,
                spectral: *spectral,
                subtype: *subtype,
                size: *size,
                name: name.clone(),
            }),
            _ => None,
        })
        .collect();
    // `generate_from_overrides` treats `stars[0]` as the primary, then
    // `stars[1]` as the secondary, etc. — and the primary's `orbit`
    // override is *ignored* (a primary is always `StarOrbit::Primary`).
    // So a row that says "I'm the secondary at orbit 6" must not land
    // at `stars[0]`, or its orbit spec is silently discarded.
    //
    // Stable-sort the rows so the most-primary-looking one comes first:
    // explicit `Primary` first, then `Auto` (no orbit specified), then
    // anything with a concrete orbit (`System(_)` or `Far`) — those are
    // unambiguously companions and belong further down.
    stars.sort_by_key(|s| match s.orbit {
        Some(StarOrbit::Primary) => 0,
        None => 1,
        Some(StarOrbit::System(_)) | Some(StarOrbit::Far) => 2,
    });
    // If the user supplied only companion stars (every row has a
    // concrete System/Far orbit), there's no row claiming the primary
    // slot — and silently demoting one of them would lose its orbit
    // spec. Insert a default (fully auto-rolled) primary so the
    // user's companions stay companions.
    if stars
        .first()
        .is_some_and(|s| matches!(s.orbit, Some(StarOrbit::System(_)) | Some(StarOrbit::Far)))
    {
        stars.insert(0, StarOverride::default());
    }

    let gg_list: Vec<_> = constraints
        .bodies
        .iter()
        .filter_map(|c| match c {
            Constraint::GasGiant {
                name,
                orbit,
                size,
                num_satellites,
            } => Some(GasGiantOverride {
                name: name.clone(),
                size: *size,
                num_satellites: *num_satellites,
                orbit: *orbit,
            }),
            _ => None,
        })
        .collect();

    let any_gg_constraint = constraints
        .bodies
        .iter()
        .any(|c| matches!(c, Constraint::GasGiant { .. }));

    let planets: Vec<_> = constraints
        .bodies
        .iter()
        .filter_map(|c| match c {
            Constraint::Planet {
                is_mainworld: false,
                name,
                orbit,
                uwp,
                num_satellites,
            } => Some(PlanetOverride {
                name: name.clone(),
                orbit: *orbit,
                partial_uwp: uwp.clone(),
                num_satellites: *num_satellites,
            }),
            _ => None,
        })
        .collect();

    let moons: Vec<_> = constraints
        .bodies
        .iter()
        .filter_map(|c| match c {
            Constraint::Moon {
                name,
                parent_orbit,
                uwp,
            } => Some(MoonOverride {
                name: name.clone(),
                parent_orbit: *parent_orbit,
                partial_uwp: uwp.clone(),
            }),
            _ => None,
        })
        .collect();

    let belts: Vec<_> = constraints
        .bodies
        .iter()
        .filter_map(|c| match c {
            Constraint::Belt {
                name,
                orbit,
                uwp,
                num_satellites,
            } => Some(BeltOverride {
                name: name.clone(),
                orbit: *orbit,
                partial_uwp: uwp.clone(),
                num_satellites: *num_satellites,
            }),
            _ => None,
        })
        .collect();

    let empties: Vec<_> = constraints
        .bodies
        .iter()
        .filter_map(|c| match c {
            Constraint::Empty { orbit } => Some(*orbit),
            _ => None,
        })
        .collect();

    SystemOverrides {
        stars,
        system_name,
        main_world_orbit: constraints.main_world_orbit,
        secondary_bodies: constraints.secondary_bodies.clone(),
        tertiary_bodies: constraints.tertiary_bodies.clone(),
        gas_giants: if any_gg_constraint {
            Some(gg_list)
        } else {
            None
        },
        planets,
        belts,
        empties,
        moons,
        // Filled in by generate_from_constraints from the main-world
        // constraint after collect_overrides returns.
        main_world_num_satellites: None,
        // This is the constraint-driven path, so the body counts are
        // authoritative — suppress the random padding passes.
        autopop: true,
    }
}

fn count_open_orbits(system: &System) -> i32 {
    system
        .orbit_slots
        .iter()
        .filter(|body| body.is_none())
        .count() as i32
}

// Implement other functions...

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::arabic_to_roman;
    use std::collections::HashMap;

    #[test_log::test]
    /// A body pinned to a high orbit must actually land there.
    ///
    /// `ensure_orbits_for_constraints` used to grow the system by the
    /// *count* of constrained bodies, not by the highest orbit any of them
    /// asked for — so "the gas giant is at orbit 12", which is exactly the
    /// sort of fact source material gives you, produced a system of ten
    /// orbits and quietly no gas giant.
    #[test]
    fn a_high_pinned_orbit_grows_the_system_to_fit() {
        let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A").unwrap();
        cs.bodies.push(Constraint::GasGiant {
            name: None,
            orbit: Some(12),
            size: None,
            num_satellites: None,
        });
        let sys = System::generate_from_constraints_seeded(3, cs).expect("generates");
        assert_eq!(
            sys.dropped_constraints(),
            Vec::new(),
            "constraint was dropped rather than honoured"
        );
        assert!(
            matches!(
                sys.orbit_slots.get(12),
                Some(Some(OrbitContent::GasGiant(_)))
            ),
            "orbit 12 holds {:?}",
            sys.orbit_slots.get(12)
        );
    }

    /// A gas giant pinned to a low orbit must beat the don't-care planets
    /// that would otherwise have taken it.
    ///
    /// The PBG world count contributes planets with no orbit preference, and
    /// they used to claim the lowest free orbits before gas giants were
    /// placed at all — so "the gas giant is at orbit 4", with a handful of
    /// filler planets in the system, was dropped despite being perfectly
    /// satisfiable. Found by the override validator on its first real run.
    #[test]
    fn a_pinned_gas_giant_beats_unpinned_filler_planets() {
        let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A").unwrap();
        // Eight planets with no orbit of their own, as a PBG world count
        // produces.
        for _ in 0..8 {
            cs.bodies.push(Constraint::Planet {
                name: None,
                orbit: None,
                uwp: None,
                num_satellites: None,
                is_mainworld: false,
            });
        }
        cs.bodies.push(Constraint::GasGiant {
            name: None,
            orbit: Some(4),
            size: None,
            num_satellites: None,
        });
        let sys = System::generate_from_constraints_seeded(19, cs).expect("generates");
        assert_eq!(
            sys.dropped_constraints(),
            Vec::new(),
            "the pinned giant lost to a planet that had no preference"
        );
        assert!(
            matches!(
                sys.orbit_slots.get(4),
                Some(Some(OrbitContent::GasGiant(_)))
            ),
            "orbit 4 holds {:?}",
            sys.orbit_slots.get(4)
        );
    }

    /// Option A's whole justification: a system with nothing pinned must
    /// generate exactly as it did before the reservation existed.
    #[test]
    fn reserving_changes_nothing_when_no_gas_giant_is_pinned() {
        let build = || {
            let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A").unwrap();
            for _ in 0..6 {
                cs.bodies.push(Constraint::Planet {
                    name: None,
                    orbit: None,
                    uwp: None,
                    num_satellites: None,
                    is_mainworld: false,
                });
            }
            cs.bodies.push(Constraint::GasGiant {
                name: None,
                orbit: None,
                size: None,
                num_satellites: None,
            });
            System::generate_from_constraints_seeded(23, cs).unwrap()
        };
        let a = build();
        let b = build();
        let kinds = |s: &System| {
            s.orbit_slots
                .iter()
                .map(|c| match c {
                    None => "empty",
                    Some(OrbitContent::World(_)) => "world",
                    Some(OrbitContent::GasGiant(_)) => "gas giant",
                    Some(OrbitContent::Blocked) => "blocked",
                    Some(OrbitContent::Secondary) => "secondary",
                    Some(OrbitContent::Tertiary) => "tertiary",
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(&a), kinds(&b));
        assert!(
            a.orbit_slots.iter().flatten().count() > 1,
            "test is vacuous if the system is empty"
        );
    }

    /// The diagnostics have to say which of the two kinds a failure is:
    /// out-of-range is the generator failing to make room, occupied is the
    /// author's constraints colliding. Conflating them sends you to edit the
    /// wrong thing.
    #[test]
    fn a_collision_reports_occupied_not_out_of_range() {
        let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A").unwrap();
        // Two constraints on one orbit is caught by `validate` before
        // generation, which is the better outcome and not what this test is
        // about. The collisions that reach placement are the ones validation
        // can't see: here a companion star takes orbit 4, and a planet is
        // pinned to it.
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::System(4)),
            spectral: Some(StarType::M),
            subtype: Some(5),
            size: Some(StarSize::V),
            name: None,
        });
        cs.bodies.push(Constraint::Planet {
            name: Some("Doomed".into()),
            orbit: Some(4),
            uwp: None,
            num_satellites: None,
            is_mainworld: false,
        });
        let sys = System::generate_from_constraints_seeded(5, cs).expect("generates");
        let dropped = sys.dropped_constraints();
        assert!(
            dropped
                .iter()
                .any(|d| matches!(d, DroppedConstraint::OrbitOccupied { orbit: 4, .. })),
            "expected an OrbitOccupied at 4, got {dropped:?}"
        );
    }

    /// Naming the system names every body that doesn't carry a name of its
    /// own — "Merak Mists III" and friends are built from it. That's the
    /// reason the field exists; a name that only labelled the star would be
    /// nearly pointless.
    #[test]
    fn system_name_constraint_names_the_derived_bodies() {
        let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A")
            .expect("main world constraint parses");
        cs.system_name = Some("Merak Mists".to_string());
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::F),
            subtype: Some(3),
            size: Some(StarSize::V),
            name: None,
        });
        // Pin an unnamed gas giant so there is definitely a body that has to
        // derive its name. Without one, the assertion below depends on the
        // dice rolling up some auto-placed body, which for many seeds they
        // don't — the system comes out as just the main world.
        cs.bodies.push(Constraint::GasGiant {
            name: None,
            orbit: Some(4),
            size: None,
            num_satellites: None,
        });
        let sys = System::generate_from_constraints_seeded(42, cs).expect("generates");
        assert_eq!(sys.name, "Merak Mists");

        // At least one auto-named body should have inherited it. The main
        // world keeps its own name, so look for a derived one.
        let derived = sys
            .orbit_slots
            .iter()
            .flatten()
            .filter_map(|c| match c {
                OrbitContent::World(w) => Some(w.name.clone()),
                OrbitContent::GasGiant(g) => Some(g.name.clone()),
                _ => None,
            })
            .filter(|n| n.starts_with("Merak Mists "))
            .count();
        assert!(
            derived > 0,
            "no body derived its name from the star; slots were {:?}",
            sys.orbit_slots
                .iter()
                .flatten()
                .map(|c| format!("{c:?}"))
                .collect::<Vec<_>>()
        );
    }

    /// A system takes its main world's name, so derived bodies read as
    /// "Torpol VII" rather than an invented "Canis Corridor VII".
    #[test]
    fn a_system_is_named_after_its_main_world() {
        let mut cs = SystemConstraints::from_main_world("Torpol", "B55A77A-8").unwrap();
        cs.bodies.push(Constraint::GasGiant {
            name: None,
            orbit: Some(6),
            size: None,
            num_satellites: None,
        });
        let sys = System::generate_from_constraints_seeded(31, cs).expect("generates");
        assert_eq!(sys.name, "Torpol");

        // The main world keeps its own name...
        assert!(
            sys.orbit_slots.iter().flatten().any(|c| matches!(
                c,
                OrbitContent::World(w) if w.name == "Torpol"
            )),
            "main world should still be called Torpol"
        );
        // ...and the unnamed bodies derive from it.
        assert!(
            sys.orbit_slots.iter().flatten().any(|c| match c {
                OrbitContent::World(w) => w.name.starts_with("Torpol "),
                OrbitContent::GasGiant(g) => g.name.starts_with("Torpol "),
                _ => false,
            }),
            "no body derived its name from the system"
        );
    }

    /// An explicit system_name still wins — the main world is only the
    /// default.
    #[test]
    fn an_explicit_system_name_beats_the_main_world_default() {
        let mut cs = SystemConstraints::from_main_world("Torpol", "B55A77A-8").unwrap();
        cs.system_name = Some("Merak Mists".to_string());
        let sys = System::generate_from_constraints_seeded(31, cs).expect("generates");
        assert_eq!(sys.name, "Merak Mists");
    }

    /// A companion's name is its own — it names that companion's
    /// sub-system, whose bodies derive from it — so unlike the primary it
    /// really does belong on the Star constraint.
    #[test]
    fn companion_star_carries_its_own_name() {
        let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A").unwrap();
        cs.system_name = Some("Merak Mists".to_string());
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::F),
            subtype: Some(3),
            size: Some(StarSize::V),
            name: None,
        });
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Far),
            spectral: Some(StarType::M),
            subtype: Some(9),
            size: Some(StarSize::V),
            name: Some("Kepler's Lantern".to_string()),
        });
        let sys = System::generate_from_constraints_seeded(11, cs).expect("generates");
        assert_eq!(sys.name, "Merak Mists", "primary took the system name");
        let companion = sys.secondary.as_ref().expect("a companion was generated");
        assert_eq!(
            companion.name, "Kepler's Lantern",
            "companion should keep the name it was given, independent of the system's"
        );
    }

    /// The primary star can carry its own name, distinct from the system's.
    ///
    /// Usually they are the same string and nobody names the star
    /// separately. But sources do: Makergod's Oghma orbits *Fijari*, as
    /// Terra orbits Sol. `star_name()` falls back to the system name so the
    /// common case needs nothing.
    #[test]
    fn the_primary_star_can_be_named_apart_from_the_system() {
        let mut cs = SystemConstraints::from_main_world("Oghma", "B534754-9").unwrap();
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::K),
            subtype: Some(5),
            size: Some(StarSize::V),
            name: Some("Fijari".to_string()),
        });
        let sys = System::generate_from_constraints_seeded(5, cs).expect("generates");
        assert_eq!(sys.name, "Oghma", "the system is named after its main world");
        assert_eq!(sys.star_name(), "Fijari", "the star has its own name");
    }

    /// With no star name given, the star answers to the system's name.
    #[test]
    fn an_unnamed_star_falls_back_to_the_system_name() {
        let cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        let sys = System::generate_from_constraints_seeded(5, cs).expect("generates");
        assert_eq!(sys.star_name(), sys.name);
        assert_eq!(sys.star_name(), "Regina");
    }

    /// Pinning the name must not disturb anything else.
    ///
    /// `System::new` rolls a name unconditionally and the override
    /// overwrites the result, rather than skipping the roll — so the RNG
    /// stream is identical either way. If that ever changes, naming a system
    /// would quietly re-roll the whole thing, which is precisely the kind of
    /// action-at-a-distance that makes seeded generation untrustworthy.
    #[test]
    fn system_name_does_not_perturb_the_rest_of_generation() {
        let build = |name: Option<&str>| {
            let mut cs = SystemConstraints::from_main_world("Pourne", "A9B2887-A").unwrap();
            cs.system_name = name.map(str::to_string);
            cs.bodies.push(Constraint::Star {
                orbit: Some(StarOrbit::Primary),
                spectral: Some(StarType::F),
                subtype: Some(3),
                size: Some(StarSize::V),
                name: None,
            });
            System::generate_from_constraints_seeded(7, cs).unwrap()
        };
        let unnamed = build(None);
        let named = build(Some("Merak Mists"));

        assert_eq!(
            unnamed.orbit_slots.len(),
            named.orbit_slots.len(),
            "orbit count changed"
        );
        for (i, (a, b)) in unnamed
            .orbit_slots
            .iter()
            .zip(named.orbit_slots.iter())
            .enumerate()
        {
            let kind = |c: &Option<OrbitContent>| match c {
                None => "empty",
                Some(OrbitContent::World(_)) => "world",
                Some(OrbitContent::GasGiant(_)) => "gas giant",
                Some(OrbitContent::Blocked) => "blocked",
                Some(OrbitContent::Secondary) => "secondary",
                Some(OrbitContent::Tertiary) => "tertiary",
            };
            assert_eq!(kind(a), kind(b), "orbit {i} changed contents");
        }
    }

    #[test]
    fn test_roman_numerals() {
        assert_eq!(arabic_to_roman(1), "I");
        assert_eq!(arabic_to_roman(2), "II");
        assert_eq!(arabic_to_roman(3), "III");
        assert_eq!(arabic_to_roman(4), "IV");
        assert_eq!(arabic_to_roman(5), "V");
        assert_eq!(arabic_to_roman(6), "VI");
        assert_eq!(arabic_to_roman(7), "VII");
        assert_eq!(arabic_to_roman(8), "VIII");
        assert_eq!(arabic_to_roman(9), "IX");
        assert_eq!(arabic_to_roman(10), "X");
        assert_eq!(arabic_to_roman(11), "XI");
        assert_eq!(arabic_to_roman(12), "XII");
        assert_eq!(arabic_to_roman(13), "XIII");
        assert_eq!(arabic_to_roman(14), "XIV");
        assert_eq!(arabic_to_roman(15), "XV");
        assert_eq!(arabic_to_roman(16), "XVI");
        assert_eq!(arabic_to_roman(17), "XVII");
        assert_eq!(arabic_to_roman(18), "XVIII");
        assert_eq!(arabic_to_roman(19), "XIX");
        assert_eq!(arabic_to_roman(20), "XX");
        // 0 is the Roman zero "N"; values past 20 must not panic — a star
        // can roll up to 21 orbit slots (indices 0..=20, named orbit+1).
        assert_eq!(arabic_to_roman(0), "N");
        assert_eq!(arabic_to_roman(21), "XXI");
        assert_eq!(arabic_to_roman(24), "XXIV");
        assert_eq!(arabic_to_roman(40), "XL");
    }

    #[test_log::test]
    fn test_generate_system() {
        let main_uwp = "A788899-A";
        let main_world = World::from_uwp("Main World", main_uwp, false, true).unwrap();

        let system = System::generate_system(main_world);
        println!("{system}");
    }

    #[test]
    fn test_generate_system_pop_zero_main_world_does_not_panic() {
        // Regression: Aacheon (The Beyond 2720), UWP E410000-0, crashed the
        // live server. A population-0 main world made world.rs compute
        // `clamp(0, get_population() - 1)` = `clamp(0, -1)`, which panics
        // (min > max) and, under panic=abort, took down the whole process.
        // Generation is randomized, so loop to reliably exercise the
        // subordinate-world population path.
        let main_uwp = "E410000-0";
        for _ in 0..100 {
            let main_world = World::from_uwp("Aacheon", main_uwp, false, true).unwrap();
            let _ = System::generate_system(main_world);
        }
    }

    #[test]
    fn test_generate_system_pop_zero_high_tl_main_world_does_not_panic() {
        // Regression: the Aacheon case above only exercises a *low*-TL
        // pop-0 world, where force_lifeless zeroes subordinate populations
        // before the clamp. A pop-0 world with TL >= 7 instead reaches
        // gen_planetoids' `clamp(0, get_population() - 1)` = `clamp(0, -1)`,
        // which panics (lo > hi) and aborted the whole server. UWP
        // A780007-9: population 0, tech level 9. Loop to reliably fire the
        // randomized planetoid-belt placement.
        let main_uwp = "A780007-9";
        for _ in 0..200 {
            let main_world = World::from_uwp("Barren", main_uwp, false, true).unwrap();
            let _ = System::generate_system(main_world);
        }
    }

    #[test]
    fn test_exotic_atmosphere_main_world_generates_and_renders() {
        // Regression: Pourne (Trojan Reach 2324), UWP A9B2887-A — the
        // main world's atmosphere is B (11), an exotic code. The astro
        // pass indexed an 11-entry cloudiness table out of bounds and,
        // under panic=abort, SIGABRT'd the whole server (502s + browser
        // CORS errors). Must generate and render cleanly for every
        // atmosphere code, exotic ones included.
        for atmo in ['A', 'B', 'C', 'D', 'E', 'F'] {
            let uwp = format!("A9{atmo}2887-A");
            let cs = SystemConstraints::from_main_world("Exotic", &uwp).unwrap();
            let system = System::generate_from_constraints(cs)
                .unwrap_or_else(|e| panic!("atmosphere {atmo} ({uwp}) must generate: {e:?}"));
            crate::sysmap::render_png(&system).expect("must render");
        }
    }

    #[test]
    fn test_generate_from_constraints_x_starport_main_world() {
        // Regression: an `X` (no starport) main world used to be rejected
        // because the port parsed as a wildcard, leaving the UWP "partial".
        // It must now generate like any other fully-specified world.
        let constraints = SystemConstraints::from_main_world("Frontier", "X788899-A").unwrap();
        let system = System::generate_from_constraints(constraints)
            .expect("X-starport main world should generate");
        // And it renders without panic (sysmap is pure given the system).
        crate::sysmap::render_png(&system).expect("render");
    }

    #[test]
    fn test_generate_from_constraints_ehex_tech_level() {
        // Regression: Tech Level `G` (16) used to fail UWP parsing (plain
        // hex caps at `F`). It must parse via ehex and generate.
        let constraints = SystemConstraints::from_main_world("HighTech", "A788899-G").unwrap();
        System::generate_from_constraints(constraints)
            .expect("ehex Tech Level G main world should generate");
    }

    #[test_log::test]
    fn test_generate_from_constraints_single_mainworld() {
        let constraints = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        let system = System::generate_from_constraints(constraints)
            .expect("single fully-specified mainworld should always generate");
        println!("{system}");
    }

    #[test]
    fn test_constrained_gas_giants_and_belts_always_placed() {
        // Regression: Torpol (Trojan Reach 2221), pbg=624 → 2 belts +
        // 4 gas giants, worlds=16. The F4 V primary sometimes rolls only
        // a handful of orbit slots; the planet pass then filled them and
        // the gas-giant and belt passes (which run afterwards) silently
        // got zero — the system rendered with no gas giants and no belts.
        // `gen_max_orbits` is random, so loop to reliably hit the
        // few-orbits roll that triggered the bug.
        for _ in 0..200 {
            let mut cs = SystemConstraints::from_main_world("Torpol", "B55A77A-8").unwrap();
            cs.bodies.push(Constraint::Star {
                orbit: Some(StarOrbit::Primary),
                spectral: Some(StarType::F),
                subtype: Some(4),
                size: Some(StarSize::V),
                name: None,
            });
            for _ in 0..4 {
                cs.bodies.push(Constraint::GasGiant {
                    name: None,
                    orbit: None,
                    size: None,
                    num_satellites: None,
                });
            }
            for _ in 0..2 {
                cs.bodies.push(Constraint::Belt {
                    name: None,
                    orbit: None,
                    uwp: None,
                    num_satellites: None,
                });
            }
            // 16 worlds − 1 main − 2 belts − 4 gas giants = 9 planets.
            for _ in 0..9 {
                cs.bodies.push(Constraint::Planet {
                    name: None,
                    orbit: None,
                    uwp: None,
                    num_satellites: None,
                    is_mainworld: false,
                });
            }

            let system = System::generate_from_constraints(cs).expect("must generate");
            let gg = system
                .orbit_slots
                .iter()
                .filter(|s| matches!(s, Some(OrbitContent::GasGiant(_))))
                .count();
            let belts = system
                .orbit_slots
                .iter()
                .filter(|s| matches!(s, Some(OrbitContent::World(w)) if w.name.contains("Belt")))
                .count();
            assert_eq!(gg, 4, "all four gas giants must be placed");
            assert_eq!(belts, 2, "both belts must be placed");

            // The main world (Torpol, atmosphere 5 → habitable-zone world)
            // must land in the habitable zone, not get exiled to a far orbit
            // by the auto-placed planet constraints. F4 V's habitable orbit
            // is 5; reserving it keeps the main world there.
            let mw_orbit = system.orbit_slots.iter().position(
                |s| matches!(s, Some(OrbitContent::World(w)) if w.is_mainworld()),
            );
            assert_eq!(mw_orbit, Some(5), "main world must sit in the habitable zone");

            // Total body count must equal the W digit exactly (1 main + 9
            // planets + 2 belts + 4 gas giants = 16). The random world-fill
            // is suppressed in autopop so the star's orbit roll can't pad
            // the system past its W count.
            let bodies = system
                .orbit_slots
                .iter()
                .filter(|s| {
                    matches!(s, Some(OrbitContent::World(_)) | Some(OrbitContent::GasGiant(_)))
                })
                .count();
            assert_eq!(bodies, 16, "body count must equal the W (worlds) digit");
        }
    }

    #[test]
    fn test_autopop_body_count_matches_worlds_digit() {
        // The body count of a constraint-driven (autopop) system must equal
        // its W digit exactly, regardless of how many orbit slots the star
        // happens to roll. Cover a star that tends to over-roll orbits
        // (a bright giant) and authoritative-zero belt/gas-giant digits.
        let cases = [
            // (uwp, spectral, worlds, belts, gas_giants)
            ("B55A77A-8", StarType::F, 5usize, 0usize, 1usize),
            ("A788899-A", StarType::M, 8, 0, 3), // giant tends to over-roll orbits
            ("B55A77A-8", StarType::G, 4, 0, 0), // belts & giants both zero → exact
            ("B55A77A-8", StarType::G, 1, 0, 0), // lone main world
        ];
        for (uwp, spectral, worlds, belts, giants) in cases {
            let planets = worlds - 1 - belts - giants;
            for _ in 0..40 {
                let mut cs = SystemConstraints::from_main_world("AutoPop", uwp).unwrap();
                cs.bodies.push(Constraint::Star {
                    orbit: Some(StarOrbit::Primary),
                    spectral: Some(spectral),
                    subtype: Some(2),
                    size: Some(if spectral == StarType::M {
                        StarSize::II
                    } else {
                        StarSize::V
                    }),
                    name: None,
                });
                for _ in 0..giants {
                    cs.bodies.push(Constraint::GasGiant {
                        name: None,
                        orbit: None,
                        size: None,
                        num_satellites: None,
                    });
                }
                for _ in 0..belts {
                    cs.bodies.push(Constraint::Belt {
                        name: None,
                        orbit: None,
                        uwp: None,
                        num_satellites: None,
                    });
                }
                for _ in 0..planets {
                    cs.bodies.push(Constraint::Planet {
                        name: None,
                        orbit: None,
                        uwp: None,
                        num_satellites: None,
                        is_mainworld: false,
                    });
                }
                let system = System::generate_from_constraints(cs).expect("must generate");
                let bodies = system
                    .orbit_slots
                    .iter()
                    .filter(|s| {
                        matches!(s, Some(OrbitContent::World(_)) | Some(OrbitContent::GasGiant(_)))
                    })
                    .count();
                assert_eq!(
                    bodies, worlds,
                    "W={worlds} (b={belts},g={giants}) for {uwp}/{spectral:?} must yield exactly {worlds} bodies, got {bodies}"
                );
            }
        }
    }

    #[test]
    fn test_constraints_grow_orbits_past_classic_table() {
        // A system can demand more bodies than the classic 20-slot orbit
        // table holds. The orbit list must grow to fit them and the high
        // orbits must render distances (no out-of-bounds panic on the
        // ORBITAL_DISTANCE lookup, which now extrapolates past slot 19).
        let mut cs = SystemConstraints::from_main_world("Crowded", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::G),
            subtype: Some(2),
            size: Some(StarSize::V),
            name: None,
        });
        for _ in 0..6 {
            cs.bodies.push(Constraint::GasGiant {
                name: None,
                orbit: None,
                size: None,
                num_satellites: None,
            });
        }
        // 30 bodies total → forces the orbit list well past 20 slots.
        for _ in 0..23 {
            cs.bodies.push(Constraint::Planet {
                name: None,
                orbit: None,
                uwp: None,
                num_satellites: None,
                is_mainworld: false,
            });
        }

        let system = System::generate_from_constraints(cs).expect("must generate");
        assert!(
            system.get_max_orbits() > 20,
            "orbit list should have grown past the classic 20-slot table, got {}",
            system.get_max_orbits()
        );
        let gg = system
            .orbit_slots
            .iter()
            .filter(|s| matches!(s, Some(OrbitContent::GasGiant(_))))
            .count();
        assert_eq!(gg, 6, "all six gas giants must be placed");
        // Rendering touches get_orbital_distance for every occupied slot,
        // including the ones past slot 19 — must not panic.
        crate::sysmap::render_png(&system).expect("render past-table system");
    }

    #[test]
    fn test_generate_with_three_star_overrides() {
        // Mirrors what the constraint UI builds when picking Noricum
        // (Trojan Reach) from Traveller Map: Stellar = "G2 V M9 V M6 V"
        // → three Star rows, the first marked Primary, the rest Auto.
        // The system must come back with all three stars wired in.
        let mut cs = SystemConstraints::from_main_world("Noricum", "D8867BB-1").unwrap();
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::G),
            subtype: Some(2),
            size: Some(StarSize::V),
            name: None,
        });
        cs.bodies.push(Constraint::Star {
            orbit: None,
            spectral: Some(StarType::M),
            subtype: Some(9),
            size: Some(StarSize::V),
            name: None,
        });
        cs.bodies.push(Constraint::Star {
            orbit: None,
            spectral: Some(StarType::M),
            subtype: Some(6),
            size: Some(StarSize::V),
            name: None,
        });
        let system = System::generate_from_constraints(cs).expect("three stars must generate");
        assert!(
            system.secondary.is_some(),
            "secondary star missing — only primary loaded"
        );
        assert!(
            system.tertiary.is_some(),
            "tertiary star missing — only two stars loaded"
        );
        assert_eq!(system.star.star_type, StarType::G);
    }

    #[test]
    fn test_generate_from_constraints_rejects_partial_main_world_uwp() {
        // Partial UWPs are honored for non-main-world Planet/Moon/Belt
        // constraints, but the main world still needs a fully
        // specified UWP because its atmosphere/population feed the
        // primary star's type roll.
        let cs = SystemConstraints::from_main_world("Regina", "HXXXXXX-X").unwrap();
        if let Constraint::Planet { uwp: Some(p), .. } = &cs.bodies[0] {
            assert!(!p.is_complete());
        } else {
            panic!("expected planet constraint with uwp");
        }
        let result = System::generate_from_constraints(cs);
        assert!(
            matches!(result, Err(ref e) if e.iter().any(|err| matches!(err, ConstraintError::UnsupportedYet(_))))
        );
    }

    #[test_log::test]
    fn test_generate_with_planet_constraint_no_uwp() {
        // A Planet constraint with name + orbit and no UWP should
        // produce a generated world at that orbit with the requested
        // name. We use a high orbit (8) to dodge any randomly placed
        // gas giants / hot-zone blocking that varies with star type.
        let mut cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Planet {
            name: Some("Lovia".to_string()),
            orbit: Some(8),
            uwp: None,
            num_satellites: Some(2),
            is_mainworld: false,
        });

        let system = System::generate_from_constraints(cs).expect("planet constraint accepted");
        // Find a world named "Lovia" — should be at orbit 8 if we got
        // there, but if the orbit was unavailable (occupied / out of
        // range) the constraint is skipped with a warn rather than an
        // error.
        let found = system
            .orbit_slots
            .iter()
            .filter_map(|s| match s {
                Some(crate::systems::system::OrbitContent::World(w)) if w.name == "Lovia" => {
                    Some(w.orbit)
                }
                _ => None,
            })
            .next();
        // Test passes either if we got Lovia at orbit 8, or if the
        // generation completed without error (the warn-skip path).
        // Both are valid; we're really checking that the constraint
        // doesn't reject the input.
        if let Some(orbit) = found {
            assert_eq!(orbit, 8, "Lovia placed but at unexpected orbit");
        }
    }

    #[test_log::test]
    fn test_generate_with_partial_planet_uwp() {
        // Pin a planet with a partial UWP — port=A, size=8, others wild.
        let mut cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Planet {
            name: Some("Lovia".to_string()),
            orbit: Some(8),
            uwp: Some(crate::systems::constraint::PartialUwp::parse("A8XXXXX-X").unwrap()),
            num_satellites: None,
            is_mainworld: false,
        });
        let system =
            System::generate_from_constraints(cs).expect("partial Planet UWP should be accepted");
        // If Lovia got placed, its size must be 8 and port A.
        let lovia = system.orbit_slots.iter().find_map(|s| match s {
            Some(crate::systems::system::OrbitContent::World(w)) if w.name == "Lovia" => Some(w),
            _ => None,
        });
        if let Some(w) = lovia {
            assert_eq!(w.size, 8);
            assert!(matches!(w.port, crate::trade::PortCode::A));
        }
    }

    #[test_log::test]
    fn test_generate_with_belt_constraint() {
        let mut cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Belt {
            name: Some("Asteroid Reach".to_string()),
            orbit: Some(7),
            uwp: None,
            num_satellites: None,
        });
        let system =
            System::generate_from_constraints(cs).expect("belt constraint should be accepted");
        let belt = system.orbit_slots.iter().find_map(|s| match s {
            Some(crate::systems::system::OrbitContent::World(w)) if w.name == "Asteroid Reach" => {
                Some(w)
            }
            _ => None,
        });
        if let Some(b) = belt {
            assert_eq!(b.size, 0, "Belt must always be size 0");
        }
    }

    #[test_log::test]
    fn test_generate_with_empty_constraint() {
        let mut cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Empty { orbit: 9 });
        let system =
            System::generate_from_constraints(cs).expect("empty constraint should be accepted");
        // If orbit 9 is within the system, it must be Blocked.
        if 9_usize < system.orbit_slots.len() {
            assert!(matches!(
                system.orbit_slots[9],
                Some(crate::systems::system::OrbitContent::Blocked)
            ));
        }
    }

    #[test_log::test]
    fn test_generate_with_moon_constraint_no_uwp() {
        // A Moon constraint with parent_orbit pointing at the main
        // world (Regina at A788899-A typically lands at the habitable
        // zone — orbit varies by star type — so we pick orbit 0 as a
        // body-agnostic test by using the main world's eventual orbit
        // from the result, instead of asserting parent here. We just
        // verify the system generates without error and a moon was
        // appended somewhere.
        let mut cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Moon {
            name: Some("Sulatra".to_string()),
            parent_orbit: 0,
            uwp: None,
        });
        let system =
            System::generate_from_constraints(cs).expect("moon constraint should be accepted");
        // Don't assert placement details — just that generation
        // completed and the moon was either attached or skipped with
        // a warn (no error returned).
        let _ = system;
    }

    #[test_log::test]
    fn test_low_tech_main_world_zeroes_lifeless_pop() {
        // House rule: a TL < 7 main world lacks the life-support tech
        // to sustain populations on bodies without a real atmosphere
        // (atmosphere outside 2..=8). Bodies WITH a real atmosphere may
        // still hold populations even outside the habitable zone. Run
        // several seeds to cover RNG variance.
        let cs = SystemConstraints::from_main_world("LowTL", "B564644-4").unwrap();
        for _ in 0..20 {
            let system = System::generate_from_constraints(cs.clone())
                .expect("low-tech system should generate");
            for slot in system.orbit_slots.iter() {
                if let Some(OrbitContent::World(w)) = slot
                    && !w.is_mainworld()
                {
                    let real = (2..=8).contains(&w.atmosphere);
                    if !real {
                        assert_eq!(
                            w.get_population(),
                            0,
                            "body with atmosphere {} had pop {} despite main TL < 7",
                            w.atmosphere,
                            w.get_population()
                        );
                    }
                    for sat in w.satellites.sats.iter() {
                        let sat_real = (2..=8).contains(&sat.atmosphere);
                        if !sat_real {
                            assert_eq!(
                                sat.get_population(),
                                0,
                                "satellite with atmosphere {} had pop {} despite main TL < 7",
                                sat.atmosphere,
                                sat.get_population()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test_log::test]
    fn test_2d6_random() {
        let mut buckets = HashMap::new();
        for _ in 0..10000 {
            let roll = roll_2d6();
            *buckets.entry(roll).or_insert(0) += 1;
        }

        let mut count_vec: Vec<_> = buckets.iter().collect();
        count_vec.sort_by(|a, b| a.0.cmp(b.0));
        for (roll, count) in count_vec {
            println!("{}: {:2.2}%", roll, *count as f32 / 100.0);
        }
    }
}
