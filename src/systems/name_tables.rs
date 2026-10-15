//! # Name Generation Tables Module
//!
//! This module provides random name generation for various astronomical bodies
//! in the Traveller universe. It contains curated lists of thematic names and
//! functions to randomly select from them, ensuring consistent and immersive
//! naming throughout generated solar systems.
//!
//! ## Key Features
//!
//! - **Planet Names**: Science fiction themed names for worlds and gas giants
//! - **Star System Names**: Astronomical and mythological names for star systems
//! - **Moon Names**: Classical and mythological names for satellite bodies
//! - **Random Selection**: Thread-safe random generation using the `rand` crate
//!
//! ## Usage
//!
//! ```rust
//! # use worldgen::systems::name_tables::{gen_planet_name, gen_star_system_name, gen_moon_name};
//! let planet = gen_planet_name();        // "Kepler's Keep"
//! let system = gen_star_system_name();   // "Aegis Prime"
//! let moon = gen_moon_name();            // "Ganymede"
//! ```
//!
//! ## Thread Safety
//!
//! All generation functions use `rand::rng()` which provides thread-local
//! random number generation, making them safe for concurrent use across
//! multiple threads during system generation.

use crate::util::rng_random_range;

/// Generates a random moon name
///
/// Selects from a comprehensive list of classical mythology and real
/// astronomical body names. Includes names from Greek, Roman, Norse,
/// and other mythological traditions, as well as real moons from
/// our solar system.
///
/// # Returns
///
/// A randomly selected moon name as a `String`
///
/// # Examples
///
/// ```rust,ignore
/// # use worldgen::systems::name_tables::gen_moon_name;
/// let name = gen_moon_name();
/// // Possible results: "Luna", "Phobos", "Europa"
/// ```
pub fn gen_moon_name() -> String {
    MOON_NAMES[rng_random_range(0..MOON_NAMES.len())].to_string()
}

/// Generates a random planet name
///
/// Selects from a curated list of science fiction themed names suitable
/// for worlds and gas giants. Names evoke themes of exploration, technology,
/// and distant frontiers.
///
/// # Returns
///
/// A randomly selected planet name as a `String`
///
/// # Examples
///
/// ```rust,ignore
/// # use worldgen::systems::name_tables::gen_planet_name;
/// let name = gen_planet_name();
/// // Possible results: "Kepler's Keep", "Nova Nexus", "Quantum Quasar"
/// ```
pub fn gen_planet_name() -> String {
    PLANET_NAMES[rng_random_range(0..PLANET_NAMES.len())].to_string()
}

/// Generates a random star system name
///
/// Selects from a list of astronomical and mythological names that combine
/// stellar phenomena with evocative descriptors. Names typically follow
/// patterns like "Constellation Feature" or "Star Descriptor".
///
/// # Returns
///
/// A randomly selected star system name as a `String`
///
/// # Examples
///
/// ```rust,ignore
/// # use worldgen::systems::name_tables::gen_star_system_name;
/// let name = gen_star_system_name();
/// // Possible results: "Aegis Prime", "Orion's Forge", "Vega Void"
/// ```
pub fn gen_star_system_name() -> String {
    STAR_SYSTEM_NAMES[rng_random_range(0..STAR_SYSTEM_NAMES.len())].to_string()
}

/// Star system names combining astronomical terms with evocative descriptors
///
/// A curated collection of 100 names that evoke the grandeur and mystery
/// of space exploration. Names often reference real constellations,
/// stellar phenomena, and astronomical concepts while maintaining
/// a sense of adventure and discovery.
///
/// ## Naming Patterns
///
/// - **Constellation + Feature**: "Orion's Forge", "Cassiopeia Cluster"
/// - **Star + Descriptor**: "Rigel Rift", "Vega Void"
/// - **Phenomenon + Location**: "Nova Nexus", "Quantum Quasar"
/// - **Mythological + Space**: "Aegis Prime", "Nemesis Nexus"
const STAR_SYSTEM_NAMES: [&str; 100] = [
    "Aegis Prime", "Avessa Abyss", "Andromeda Ardent",
    "Aquila Ascent", "Belvane Blaze", "Beryth Beacon",
    "Boötes Borderlands", "Canis Corridor", "Carina Crossroads",
    "Cassiopeia Crown", "Centaurus Candle", "Cepheus Cinders",
    "Cetus Confluence", "Chamaeleon Channel", "Columba Corona",
    "Corvus Crest", "Crater Cinder", "Crux Crucible",
    "Cygnus Reach", "Delphinus Delta", "Dravin Dusk",
    "Dorado Dawn", "Dorado Diadem", "Draco Drifts",
    "Draco's Claw", "Epsilon Enigma", "Equuleus Ember",
    "Eridanus Expanse", "Farrow Fringes", "Fornax Frontier",
    "Gemini Glint", "Glassenay", "Grus Glow",
    "Hercules Halo", "Hercules Hinterlands", "Horologium Halo",
    "Hydra Hearth", "Hydra's Heart", "Hydrus Horizon",
    "Indus Ignis", "Io Iris", "Ixion Corridor",
    "Juno's Veil", "Janus Jewel", "Kepler's Kindling",
    "Kessar Keystone", "Lacerta Lamp", "Leo's Lantern",
    "Lepus Leap", "Lupus Lodestar", "Lynx Lumen",
    "Lyra's Light", "Mensa Meridian", "Morvane Mists",
    "Microscopium Mote", "Mirell Mirage", "Monoceros Morn",
    "Musca Mists", "Nemesis Nexus", "Norma Noon",
    "Nova Nexus", "Oberon Opal", "Octans Odyssey",
    "Ophiuchus Oriel", "Orion's Flare", "Pavo Pyre",
    "Pegasus Passage", "Phoenix Frontier", "Pictor Pearl",
    "Pisces Prism", "Pellan Point", "Pravis Passage",
    "Quintus Quill", "Quintessa", "Rhosk Reach",
    "Reticulum Radiance", "Ravel Rift", "Sagitta Spark",
    "Scorpius Sear", "Sarell Sunfire", "Syrene Splendor",
    "Taurus Torch", "Theta Threshold", "Tucana Traverse",
    "Umbra Umber", "Ursa Ultima", "Ursa Umbra",
    "Velka Vesper", "Vela Voyage", "Virgo Venture",
    "Volans Vista", "Vulpecula Veil", "Wystan Wick",
    "Wolf's Wisp", "Xanadu Xenon", "Xena Crossroads",
    "Yakima Yellow", "Yggdrasil Yule", "Zenith Zeal",
    "Zarine Zephyr",
];

/// Moon and satellite names from classical mythology and astronomy
///
/// A comprehensive collection of 338+ names drawn from various sources:
/// - **Real Moons**: Names of actual moons in our solar system
/// - **Greek Mythology**: Classical names from Greek pantheon
/// - **Roman Mythology**: Roman equivalents and unique names
/// - **Norse Mythology**: Names from Scandinavian traditions
/// - **Astronomical Objects**: Names of asteroids, dwarf planets, and other bodies
///
/// ## Name Categories
///
/// - **Major Moons**: Well-known names like "Ganymede", "Titan", "Europa"
/// - **Minor Moons**: Lesser-known but real astronomical names
/// - **Mythological**: Classical names from various pantheons
/// - **Descriptive**: Names that evoke lunar or celestial qualities
///
/// Names are suitable for any type of satellite, from small rocky moons
/// to large satellite worlds with their own atmospheres and populations.
const MOON_NAMES: [&str; 338] = [
    "Acamas", "Actaea", "Adrastea", "Aegaeon",
    "Aegir", "Aeneas", "Aether", "Aitne",
    "Ajax", "Akycha", "Albiorix", "Alvaldi",
    "Amalthea", "Ananke", "Anchise", "Anchises",
    "Angrboda", "Antenor", "Anthe", "Antilochus",
    "Aoede", "Arche", "Ariel", "Asteria",
    "Astraeus", "Atlas", "Aurvandil", "Autonoe",
    "Baldr", "Banquo", "Bebhionn", "Beira",
    "Belenus", "Beli", "Belinda", "Bellona",
    "Bergelmir", "Bestla", "Bianca", "Boreas",
    "Bragi", "Buri", "Caliban", "Callirrhoe",
    "Callisto", "Calypso", "Carme", "Carpo",
    "Cassio", "Cernunnos", "Chaldene", "Charon",
    "Coeus", "Cordelia", "Cressida", "Crius",
    "Cupid", "Cycnus", "Cyllene", "Cymbeline",
    "Dactyl", "Daphnis", "Deimos", "Deiphobus",
    "Dellingr", "Demetrius", "Desdemona", "Despina",
    "Dia", "Diomedes", "Dione", "Dolon",
    "Duncan", "Dysnomia", "Echo", "Edmund",
    "Eggther", "Elara", "Emilia", "Enceladus",
    "Epimetheus", "Epona", "Erinome", "Erriapus",
    "Ersa", "Euanthe", "Eukelade", "Eupheme",
    "Euphorbus", "Euporie", "Europa", "Eurybates",
    "Eurybia", "Eurydome", "Eurypylus", "Farbauti",
    "Fenrir", "Feste", "Fjorgyn", "Fornjot",
    "Forseti", "Francisco", "Freyja", "Freyr",
    "Frigg", "Fjalar", "Fulla", "Galatea",
    "Ganymede", "Gaspra", "Gefjon", "Geirrod",
    "Gerd", "Glaucus", "Goneril", "Greip",
    "Gridr", "Gunnlod", "Halimede", "Harpalyke",
    "Hati", "Hecate", "Hegemone", "Heimdall",
    "Helena", "Helene", "Helenus", "Helike",
    "Helios", "Hermia", "Hermippe", "Hermod",
    "Herse", "Hiiaka", "Hiisi", "Himalia",
    "Hippocamp", "Hlin", "Hodr", "Hydra",
    "Hyperion", "Hyrrokkin", "Iapetus", "Idomeneus",
    "Idunn", "Igaluk", "Ijiraq", "Ilmare",
    "Imogen", "Io", "Iocaste", "Isonoe",
    "Janus", "Japet", "Jarnsaxa", "Jord",
    "Juliet", "Kale", "Kallichore", "Kalyke",
    "Kari", "Kerberos", "Kiviuq", "Kore",
    "Laertes", "Laomedeia", "Larissa", "Leda",
    "Lempo", "Leto", "Linus", "Lofn",
    "Loge", "Lugus", "Luna", "Lysander",
    "Lysithea", "Mab", "Macduff", "Magni",
    "Malvolio", "Margaret", "Megaclite", "Meili",
    "Memnon", "Menoetius", "Methone", "Metis",
    "Mimas", "Mimir", "Miranda", "Mneme",
    "Mnemosyne", "Modi", "Mundilfari", "Naiad",
    "Namaka", "Nanna", "Nantosuelta", "Nanuq",
    "Narvi", "Nereid", "Neso", "Nestor",
    "Nix", "Njord", "Nuliajuk", "Oberon",
    "Odin", "Ogmios", "Olivia", "Ophelia",
    "Ophion", "Orius", "Orsino", "Orthosie",
    "Paaliaq", "Pallas", "Pallene", "Pan",
    "Pandarus", "Pandia", "Pandora", "Pasiphae",
    "Pasithee", "Patroclus", "Penthesilea", "Perdita",
    "Perdix", "Perses", "Philophrosyne", "Phobos",
    "Phoebe", "Phoenix", "Pinga", "Podarces",
    "Polydeuces", "Polydorus", "Portia", "Praxidike",
    "Prometheus", "Prospero", "Protesilaus", "Proteus",
    "Psamathe", "Puck", "Qilin", "Quaoar",
    "Queta", "Quetzal", "Quincy", "Ravn",
    "Regan", "Remus", "Rhadamanthys", "Rhea",
    "Rhesus", "Romulus", "Rosalind", "Rosmerta",
    "Saga", "Sao", "Sarpedon", "Sedna",
    "Selene", "Sequana", "Setebos", "Siarnaq",
    "Sigyn", "Sinope", "Sjofn", "Skadi",
    "Skathi", "Skirnir", "Skoll", "Skrymir",
    "Snotra", "Sponde", "Stephano", "Styx",
    "Sucellus", "Surtur", "Suttungr", "Sycorax",
    "Taranis", "Tarqeq", "Tarvos", "Taygete",
    "Telesto", "Tethys", "Teutates", "Thalassa",
    "Thebe", "Theia", "Thelxinoe", "Themis",
    "Themisto", "Thiazzi", "Thrud", "Thrymr",
    "Thyone", "Titan", "Titania", "Torngarsoak",
    "Trinculo", "Triton", "Troilus", "Ull",
    "Ullr", "Umbiel", "Umbriel", "Urd",
    "Ursula", "Valeska", "Valetudo", "Vali",
    "Vanth", "Vesta", "Vidar", "Vidarr",
    "Vili", "Viola", "Waldron", "Wanda",
    "Weywot", "Wodan", "Wyvern", "Xanthus",
    "Xena", "Xerxes", "Xolotl", "Yalode",
    "Ymir", "Yvaga", "Zelinda", "Zephyr",
    "Zetes", "Zoe",
];

/// Planet names with science fiction and space exploration themes
///
/// A collection of evocative names suitable for worlds and gas giants.
/// Names are designed to sound futuristic and adventurous while
/// maintaining pronounceability and memorability.
///
/// ## Naming Themes
///
/// - **Astronomical**: References to stars, phenomena, and exploration
/// - **Technological**: Evokes advanced civilizations and space travel
/// - **Mythological**: Classical references adapted for space settings
/// - **Descriptive**: Names that suggest planetary characteristics
///
/// These names are used for significant worlds that warrant proper names
/// rather than systematic designations (e.g., populated gas giant systems).
const PLANET_NAMES: [&str; 400] = [
    "Aetheron",
    "Aetheronia",
    "Aetheronos",
    "Aethoria",
    "Aethoris",
    "Aethorix",
    "Aethorixia",
    "Aethorixon",
    "Aethorixos",
    "Aethoron",
    "Aethoronia",
    "Aethoronos",
    "Aethoros",
    "Aphelia",
    "Aphelion",
    "Apsida",
    "Apsidia",
    "Ardana",
    "Ardania",
    "Ardanis",
    "Ardanon",
    "Ardanos",
    "Astralia",
    "Astralis",
    "Astralon",
    "Astralos",
    "Auroria",
    "Auroris",
    "Auroron",
    "Auroros",
    "Barycena",
    "Barycentis",
    "Belora",
    "Beloria",
    "Beloris",
    "Beloron",
    "Beloros",
    "Boreala",
    "Borean",
    "Caeluma",
    "Caelumis",
    "Caldara",
    "Caldaris",
    "Caldaron",
    "Calena",
    "Calenia",
    "Calenis",
    "Calenon",
    "Calenos",
    "Celestara",
    "Celestaria",
    "Celestaron",
    "Celestaros",
    "Celestia",
    "Celestis",
    "Celeston",
    "Celestos",
    "Celestria",
    "Celestris",
    "Celestron",
    "Celestronia",
    "Celestronon",
    "Celestronos",
    "Cepheion",
    "Cerula",
    "Ceruleon",
    "Cerulis",
    "Chronara",
    "Chronaria",
    "Chronaron",
    "Chronaros",
    "Chronia",
    "Chronon",
    "Chronorix",
    "Chronorixia",
    "Chronorixon",
    "Chronorixos",
    "Chronos",
    "Chronosia",
    "Chronosion",
    "Chronosios",
    "Cinderia",
    "Cinderos",
    "Cindron",
    "Cosmora",
    "Cosmoria",
    "Cosmoron",
    "Cosmoros",
    "Cryona",
    "Cryonar",
    "Cryonis",
    "Declina",
    "Declinar",
    "Declinis",
    "Dolmena",
    "Dolmenar",
    "Dolmenis",
    "Dorana",
    "Dorania",
    "Doranis",
    "Doranon",
    "Doranos",
    "Dracona",
    "Draconar",
    "Draconis",
    "Eclipta",
    "Ecliptar",
    "Ecliptis",
    "Elaria",
    "Elaris",
    "Elaron",
    "Embera",
    "Emberis",
    "Emberon",
    "Equinoxa",
    "Equinoxis",
    "Ereba",
    "Erebis",
    "Erebon",
    "Eskera",
    "Eskeria",
    "Eskeris",
    "Eskeron",
    "Eskeros",
    "Farala",
    "Faralia",
    "Faralis",
    "Faralon",
    "Faralos",
    "Ferruma",
    "Ferrumis",
    "Fissura",
    "Fissuris",
    "Fissuron",
    "Fornacis",
    "Fornaxa",
    "Fornaxis",
    "Garana",
    "Garania",
    "Garanis",
    "Garanon",
    "Garanos",
    "Gemina",
    "Geminar",
    "Geminis",
    "Glacia",
    "Glacion",
    "Glacis",
    "Granita",
    "Granitis",
    "Graniton",
    "Halcyar",
    "Halcyona",
    "Halcyonis",
    "Halena",
    "Halenia",
    "Halenis",
    "Halenon",
    "Halenos",
    "Hallowa",
    "Hallowis",
    "Hallowon",
    "Heliosia",
    "Heliosion",
    "Heliosios",
    "Helixara",
    "Helixaria",
    "Helixaron",
    "Helixaros",
    "Helixia",
    "Helixion",
    "Helixionia",
    "Helixionon",
    "Helixionos",
    "Helixon",
    "Helixoria",
    "Helixoris",
    "Helixoron",
    "Helixos",
    "Hespera",
    "Hesperis",
    "Hesperon",
    "Iriana",
    "Iriania",
    "Irianis",
    "Irianon",
    "Irianos",
    "Izara",
    "Izaris",
    "Izaron",
    "Jorana",
    "Jorania",
    "Joranis",
    "Joranon",
    "Joranos",
    "Kairon",
    "Kairosa",
    "Kairosis",
    "Karsta",
    "Karstis",
    "Kelvina",
    "Kelvinar",
    "Kelvinis",
    "Kestra",
    "Kestria",
    "Kestris",
    "Kestron",
    "Kestros",
    "Lacerta",
    "Lacertis",
    "Lacerton",
    "Lacunia",
    "Lacunis",
    "Lethara",
    "Lethea",
    "Letheon",
    "Librata",
    "Libratis",
    "Libraton",
    "Lorana",
    "Lorania",
    "Loranis",
    "Loranon",
    "Loranos",
    "Lumina",
    "Luminara",
    "Luminaria",
    "Luminaris",
    "Luminaron",
    "Luminaros",
    "Luminon",
    "Luminos",
    "Lunara",
    "Lunaria",
    "Lunaron",
    "Lunaros",
    "Magneton",
    "Marena",
    "Marenia",
    "Marenis",
    "Marenon",
    "Marenos",
    "Mensia",
    "Mensis",
    "Menson",
    "Merida",
    "Meridis",
    "Miraga",
    "Miragis",
    "Miragon",
    "Nadira",
    "Nadiris",
    "Nadiron",
    "Nerala",
    "Neralia",
    "Neralis",
    "Neralon",
    "Neralos",
    "Noctilis",
    "Noctisa",
    "Normia",
    "Normis",
    "Obsida",
    "Obsidis",
    "Obsidon",
    "Occulta",
    "Occultis",
    "Occulton",
    "Octana",
    "Octanis",
    "Orana",
    "Orania",
    "Oranis",
    "Oranon",
    "Oranos",
    "Orlatha",
    "Orlathis",
    "Orlathon",
    "Orphara",
    "Orphea",
    "Orpheon",
    "Paralla",
    "Parallis",
    "Parallon",
    "Perana",
    "Perania",
    "Peranis",
    "Peranon",
    "Peranos",
    "Perihela",
    "Perihelis",
    "Perihelon",
    "Pyxida",
    "Pyxidis",
    "Pyxion",
    "Quantara",
    "Quantaria",
    "Quantaron",
    "Quantaros",
    "Quenara",
    "Quenaria",
    "Quenaris",
    "Quenaron",
    "Quenaros",
    "Quiesca",
    "Quiescis",
    "Radiana",
    "Radianis",
    "Radianor",
    "Regolia",
    "Regolis",
    "Reticula",
    "Reticulis",
    "Reticulon",
    "Rhalena",
    "Rhalenia",
    "Rhalenis",
    "Rhalenon",
    "Rhalenos",
    "Sculpta",
    "Sculptis",
    "Sculpton",
    "Sidera",
    "Sideris",
    "Sideron",
    "Siroca",
    "Sirocis",
    "Sirocon",
    "Solara",
    "Solaria",
    "Solaron",
    "Solaros",
    "Sorena",
    "Tamsina",
    "Tamsinis",
    "Tamsinon",
    "Tectonis",
    "Tectora",
    "Tectoris",
    "Tempesta",
    "Tempestis",
    "Tempeston",
    "Termina",
    "Terminar",
    "Terminis",
    "Thalassara",
    "Thalassaria",
    "Thalassaron",
    "Thalassaros",
    "Thalassia",
    "Thalassion",
    "Thalassionia",
    "Thalassionon",
    "Thalassionos",
    "Thalassos",
    "Tholina",
    "Tholinis",
    "Tholion",
    "Tucania",
    "Tucanis",
    "Tychona",
    "Tychonar",
    "Tychonis",
    "Umbrala",
    "Umbralis",
    "Umbralon",
    "Verdanta",
    "Verdantis",
    "Verdanton",
    "Vespera",
    "Vesperis",
    "Vesperon",
    "Vessenara",
    "Vessenaris",
    "Vessenaron",
    "Vortexara",
    "Vortexaria",
    "Vortexaris",
    "Vortexaron",
    "Vortexaros",
    "Vortexia",
    "Vortexion",
    "Vortexionia",
    "Vortexionon",
    "Vortexionos",
    "Vortexis",
    "Vortexon",
    "Vortexos",
    "Wolframa",
    "Wolframis",
    "Xantha",
    "Xanthis",
    "Xanthon",
    "Zenitha",
    "Zenithis",
    "Zephyria",
    "Zephyron",
    "Zephyros",
    "Zodiaca",
    "Zodiacis",
];

#[cfg(test)]
mod name_table_tests {
    use super::*;

    /// `PLANET_NAMES` must hold 400 *distinct* names.
    ///
    /// It used to hold 400 entries and 199 distinct ones — half the table
    /// was repeats, so `Stellaron` and `Quasaron` were drawn eight times
    /// as often as a name that appeared once, and a 15-body system like
    /// Thebus visibly reused names. Index-based selection
    /// (`PLANET_NAMES[rng_random_range(0..len)]`) weights by slot count,
    /// so a duplicate is not a harmless typo; it is a probability bug.
    ///
    /// `MOON_NAMES` had it worse: 338 entries and 143 distinct, 58%
    /// repeats, which put two moons named `Kiviuq` in one Thebus system
    /// at once. Both tables are asserted now.
    ///
    /// `STAR_SYSTEM_NAMES` was the last holdout at 100 entries and 97
    /// distinct (`Eridanus Expanse`, `Fornax Frontier` and `Indus Inlet`
    /// each appeared twice). All three tables are covered now.
    #[test]
    fn name_tables_have_no_duplicates() {
        for (label, table) in [
            ("PLANET_NAMES", &PLANET_NAMES[..]),
            ("MOON_NAMES", &MOON_NAMES[..]),
            ("STAR_SYSTEM_NAMES", &STAR_SYSTEM_NAMES[..]),
        ] {
            let mut seen = std::collections::HashSet::new();
            let mut dupes: Vec<&str> =
                table.iter().filter(|n| !seen.insert(**n)).copied().collect();
            dupes.sort_unstable();
            dupes.dedup();
            assert!(
                dupes.is_empty(),
                "{label} repeats {} name(s): {:?}",
                dupes.len(),
                dupes
            );
            assert_eq!(seen.len(), table.len(), "{label} length mismatch");
        }
    }

    /// No planet name may collide with a moon or system name.
    ///
    /// A world and one of its own moons sharing a name reads as a bug to
    /// anyone looking at the system map, and the tables are drawn from
    /// independently.
    #[test]
    fn planet_names_do_not_collide_with_other_tables() {
        let others: std::collections::HashSet<&str> = MOON_NAMES
            .iter()
            .chain(STAR_SYSTEM_NAMES.iter())
            .copied()
            .collect();
        let clashes: Vec<&str> = PLANET_NAMES
            .iter()
            .filter(|n| others.contains(**n))
            .copied()
            .collect();
        assert!(clashes.is_empty(), "shared with other tables: {clashes:?}");
    }
}
