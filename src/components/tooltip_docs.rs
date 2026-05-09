//! Hover-help text for every input in the Ship Simulator and Trade
//! Computer forms.
//!
//! Edit the strings below to update the in-app `?` tooltips — no other
//! changes are needed. An empty string suppresses the icon for that field.
//!
//! Constants in the "Ship", "Crew", and "Voyage" sections are shared
//! across both forms wherever the field is conceptually the same. The
//! "Trade Computer only" section holds the inputs that don't appear in
//! the simulator.

// ---- Ship ----
pub const CARGO_CAPACITY: &str = "Total cargo capacity on the ship.  Used to hold passenger cargo allotments, speculative trade goods, and chartered freight";
pub const CREW_STATEROOMS: &str = "Total number of standard staterooms used by the crew, used to determine monthly support costs.";
pub const PASSENGER_STATEROOMS: &str = "Total number of standard staterooms available for high, medium, and basic passengers, used both for availability \
                                        when soliciting passengers and for monthly support costs.";
pub const LOW_BERTHS: &str = "Total number of low berths available for low passengers, used both for availability when \
                              soliticting low passengers and for monthly support costs.";
pub const JUMP_RATING: &str = "Max jump of this ship. Jump rating is used when looking at available destinations for the next jump.";
pub const FUEL_COST_PER_PARSEC: &str = "Cost of fuel used per jump by the ship.  If the ship has no fuel processors enter Cr 1000/ton for refined fule. \
                                        If the ship has fuel processors, then enter Cr 500/ton for unrefined fuel. \
                                        If the ship also has fuel scoops, then wilderness refueling is possible so enter 0.";
pub const MAINTENANCE_PER_PERIOD: &str =
    "Ship maintenance cost (per the design) per monthly maintenance period.";
pub const SALARY_PER_PERIOD: &str =
    "Crew salaries (excluding profit shares) per monthly maintenance period.";
pub const MORTGAGE_PER_PERIOD: &str = "Mortage payment (if any) per monthly maintenance period.";
pub const CREW_PROFIT_SHARE: &str = "Fraction of profit allocated to captain and crew.  This is calculated after all expenses such as the cost of \
                                     speculative goods, life support, and ship maintenance.  It is calculated, however, before mortgage payments are \
                                     subtracted as such payments should come out of the owner's share as financing for the ship.";

// ---- Crew ----
pub const BROKER_SKILL: &str = "Broker skill of the ship's purser for calculating discounts or premium on speculative good trading.";
pub const STEWARD_SKILL: &str =
    "Steward skill of the ship's chief steward used to attract passengers for each jump.";
pub const LEADERSHIP: &str = "Leadership skill of captain. Higher leadership skill helps avoid \
     complications and mitigate their impact when they happen.";
pub const WEAPONS: &str = "The total number of weapons summed across all turrets on the ship. \
     The higher the number the lower the impact of pirate encounters.";
pub const CREW_SIZE: &str = "Total number of crew onboard. Used to calculate monthly life support \
     costs.";

// ---- Voyage ----
pub const STARTING_BUDGET: &str = "Capital provided by the owner to facilitate speculative trading.  This budget must be repaid before any crew \
                                   profit share is calculated.";
pub const START_DATE: &str = "Start date for the cruise.";
pub const TARGET_COMPLETION: &str = "A rough target completion date for the cruise. This is only a heuristic to tell the captain when to start \
                                     heading for home: when the cruise is half way towards the target completion date, a strong preference is given \
                                     for next works that take the ship back towards its homeworld.";
pub const ILLEGAL_GOODS: &str = "Is this ship willing to trade in illegal goods.";

// ---- Trade Computer only ----
pub const SHIP_NAME: &str = "Name of this ship.  Each unique ship is saved separately with all its current information, especially its ship \
                             stats and manifest.  By saving this information you can return to this information session after session.";
pub const DISTANCE: &str = "Distance from current world to desination world in parsecs.";
pub const SYSTEM_BROKER_SKILL: &str =
    "The (adversarial) broker skill of the current trading world.";
pub const EXECUTE_TRADES: &str = "Execute all trades at this world.  All purchased goods will have their cost deducted from profit and appear \
                                  in the manifest.  All sold goods in the manifest will add proceeeds to profit and be removed.  Passenger \
                                  fares will be added to the profit, as will profit for chartered freight cargo.  Monthly expenses are not \
                                  included with this button as they will only be applied once per maintenance period; use the \
                                  \"Apply monthly expenses\" button to apply those";
