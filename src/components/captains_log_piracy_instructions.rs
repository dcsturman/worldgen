//! Tone-selected instruction header for the *pirate* captain's-log AI prompt.
//!
//! [`instructions`] assembles the static text that prefixes every pirate
//! captain's-log call to Vertex AI. The dynamic cruise data (raids, threat
//! evasions, fence deals, reputation) is appended *after* this by
//! [`crate::components::captains_log_piracy_prompt::build_piracy_prompt`].
//!
//! Three registers, chosen per "Generate" click (see [`LogTone`]):
//! - **Criminal Report** (~⅔) — a professional crook's flat, transactional log.
//! - **Criminal, Bloodthirsty** — the same crook, but savoring the violence;
//!   used when the voyage doctrine was Bloodthirsty.
//! - **Genteel Euphemism** (~⅓) — scrupulously polite, sanitized, never admits
//!   to a crime; the comedy is the gap between the prim words and the facts.
//!
//! Everything outside the ROLE & TONE block (canon, no-FTL comms, structure,
//! the marooned hook, hard rules) is shared across the three registers. Edit
//! freely; do **not** add `== CRUISE DATA ==` lines here.

/// Which tonal register the pirate's log is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTone {
    /// A professional crook's flat, unsentimental report.
    CriminalReport,
    /// The crook, but savage and relishing the violence (Bloodthirsty doctrine).
    CriminalBloodthirsty,
    /// Scrupulously polite, euphemistic — never admits to a crime.
    GenteelEuphemism,
}

/// Assemble the full instruction header for the given tonal register.
pub fn instructions(tone: LogTone) -> String {
    let role_tone = match tone {
        LogTone::CriminalReport => ROLE_TONE_CRIMINAL,
        LogTone::CriminalBloodthirsty => ROLE_TONE_BLOODTHIRSTY,
        LogTone::GenteelEuphemism => ROLE_TONE_GENTEEL,
    };
    format!("{INTRO}\n\n{role_tone}\n\n{REST}")
}

const INTRO: &str = r#"You are writing a Traveller RPG corsair-captain's log: a vivid, in-character narrative summary of one raiding cruise. Read the structured data below and then write 1 to 5 paragraphs (no more than 50 sentences total) in the captain's first-person voice."#;

const ROLE_TONE_CRIMINAL: &str = r#"== ROLE & TONE ==
- You captain an independent pirate ship. You're a criminal, plainly, and this log is your own private record of the work — what you hit, what it cost, what you cleared. Write it the way a professional crook talks when the marks aren't listening: flat, transactional, unsentimental.
- Tone: cold, clipped, businesslike. You don't glorify the work and you don't apologize for it — it's a living. Dry, dark humor is fine; theatrics are not. NEVER pirate-movie cliche: no "arr", "matey", "scurvy dogs", "blimey", "ye", "shiver me timbers".
- Criminal vernacular: a "score", a "job", a "mark", "the take", "the cut", "moving the goods", "the heat", "the buyer" (the fence), "the law" (the navy). Violence is stated plainly, never savored.
- Refer to yourself in the first person ("I", "we"). The crew is "we"; the ship is "she" or by name."#;

const ROLE_TONE_BLOODTHIRSTY: &str = r#"== ROLE & TONE ==
- You captain an independent pirate ship, and you LIKE the work. This log is your own private record — what you hit, what you broke, what you cleared — and you don't hide the relish. Write it raw and unguarded, the way a killer talks when no one's keeping score.
- Tone: savage, blunt, a little gleeful about the violence — a predator's log, not a professional's cold ledger. The menace is real, not camp: still NO pirate-movie cliche (no "arr", "matey", "scurvy dogs", "blimey", "shiver me timbers") and no cartoon mustache-twirling.
- Criminal vernacular, but you dwell on the fear you put in them and the wreckage you leave: a "score", "the take", "the heat", "the buyer", "the law". When a ship fought and you put her down, say so and savor it; when a crew folded, enjoy that they knew better than to make you work for it.
- Refer to yourself in the first person ("I", "we"). The crew is "we"; the ship is "she" or by name."#;

const ROLE_TONE_GENTEEL: &str = r#"== ROLE & TONE ==
- You captain an independent pirate ship and consider yourself, for the record, a perfectly respectable operator. Write this log in scrupulously polite, sanitized, corporate-genteel language that never once admits to a crime. The comedy is entirely in the gap between the prim words and the brutal facts.
- Tone: courteous, understated, almost bureaucratic — a maitre d' or compliance officer narrating armed robbery. Never coarse, not one oath. No pirate cliche.
- Euphemize relentlessly: you "invite a vessel to a voluntary redistribution of cargo"; a ship you crippled "experienced an unscheduled reduction in capability"; one you destroyed was "permanently retired from active service"; a crew you killed were "released from their obligations"; fencing is "liaising with a private acquisitions specialist"; a fence sting is "a regrettable lapse in our partner's discretion"; reputation is "our growing professional profile"; the navy is "the relevant authorities".
- Refer to yourself in the first person ("I", "we"), unfailingly gracious about all of it."#;

const REST: &str = r#"== INVENT YOUR CAPTAIN ONCE ==
At the very start of the log, decide your captain's name and use it consistently throughout. Roll once on this distribution and KEEP THE SAME NAME for the whole entry; do not rename mid-log.

- 40% Vilani (human, Imperial-core culture). Examples of style: Sumarrgha Lugaadiin, Khimkhi Naashar, Lugaadiin Kashagga. Tend to long, sibilant, doubled-consonant names.
- 40% Solomani (human, Earth-derived). Real Earth-style names from any culture: Sarah Chen, Marcus Beaumont, Aleksei Volkov, Adaeze Okonkwo, Yuki Tanaka, Diego Marquez, Fatima al-Rashid.
- 10% Aslan, FEMALE only (Aslan males traditionally do not run ships in Imperial space). Use feminine Aslan name forms with apostrophes/glottals optional: Ftahalr, Khusiyrkho, Aoiyeya'sa, Hraulaorre. The captain is unambiguously female.
- 10% Vargr (uplifted canine humanoids — a natural fit for a corsair). Short, growly, with consonant clusters: Gvaeknae, Roegz, Khaegzkae, Ngoelloegh.

You may also invent: crew member names (chief engineer, gunner, sensor op, quartermaster), names for any prey ship, defender, or fence NPC, and minor color details (a regular dockside dive, a corrupt portmaster, a fence with a temper). Be CONSISTENT: name a crew member once and keep the name. If someone is lost and replaced, say so explicitly.

== INVENT YOUR SHIP IF NEEDED ==
If the cruise data line "Ship:" reads "(unregistered ...)", pick a ship name in keeping with Traveller corsair naming and use it consistently throughout the log. Examples of the right register: Black Adder, Reaver's Due, Ghost of Glisten, Sword of Lir, Vargr's Grin, Quiet Knife. Same rule as the captain — roll once at the start, never rename mid-log.

== NAME EVERY SHIP ==
The cruise data names the *class* of every ship you meet — a "Far Trader", a "Subsidised Merchant", a "Patrol Corvette", and so on. Use that class, and also give each ship that gets its own beat a proper NAME, the way a captain would log it: "the Far Trader Gilded Khourra", "the subsidised liner Pride of Regina", "the patrol corvette Vigilant". Ship names suit their type — fat traders carry workaday or hopeful names, liners grand ones, navy ships martial ones, q-ships innocuous ones. Once you name a ship, keep that name if she comes up again. Don't rename the class the data gave you (a Far Trader stays a Far Trader), just christen the individual hull.

== TRAVELLER CANON ==
The setting is Marc Miller's Traveller / the Third Imperium. Use only canon technology and terminology:
- ALLOWED: jump drive, J-1 through J-6, jump-space / J-space, maneuver drive, gravitic plates, fuel scoops, fuel processors, life support, sandcasters, missile turrets, beam laser, pulse laser, fusion gun, particle accelerator, nuclear damper, meson screen, black globe, pop-up turret, vacc suit, low berth, System Defence Boat (SDB), q-ship, naval patrol, scout/courier, free trader, far trader, subsidized merchant, Imperium, sector, subsector, parsec, starport classes A-E, law level. For interstellar messaging: x-boat / express boat, courier drum, data crystal.
- FORBIDDEN: warp drive, hyperspace, deflector shields, phasers, photon torpedoes, transporters, force fields, lightsabers, "Federation". Anything from Star Trek, Star Wars, Mass Effect, etc. Specifically: NO FTL radio, NO subspace radio, NO ansible, NO instantaneous interstellar communication of any kind.
- Money is in credits (Cr) or kilocredits (KCr); large hauls are megacredits (MCr).

== INTERSTELLAR COMMUNICATION ==
In Traveller there is NO faster-than-light radio. Every message between star systems is physically carried by a jump-capable ship. A bounty on your head, word of your reputation, or a distress call from your own marooned ship all travel by x-boat or by whatever ship is heading the right way — a week per parsec. Reputation and heat spread at the speed of jump, not light. A distress call is RECORDED locally, then HANDED OFF to the next outbound vessel; the "received-by" date in the data is when the physical message is delivered, NOT a light-speed crossing. Right register: "we tucked the drum into the next outbound x-boat". Wrong register (never use): "we beamed a signal", "broadcast across the void", "the signal reached [another star]".

== STRUCTURE ==
- Open with one short paragraph that names the captain, the ship, the hideout / home base and sector, and sketches the cruise (how long, the doctrine you sailed under, the reputation you ended with).
- Then walk the cruise in chronological order. Give each raid that mattered its own beat:
  - Stamp dates where the data gives them.
  - Name the prey type (small/medium/heavy/rich freighter, liner, convoy) and her tonnage, and dramatize the hunt: spotting her on the scopes, the run-in, the demand to heave to.
  - How it resolved — MATCH THE OUTCOME IN THE DATA, do not invent one:
    - SURRENDERED: she folded to the threat without a shot; you took the cargo by extortion.
    - FOUGHT AND WON: she ran her guns out and you had to fight for it before boarding — note that it cost you repair damage (and any weeks lost).
    - BROKE OFF — OUTGUNNED: you sized her up and judged the fight not worth it; you let her go.
    - PREY JUMPED CLEAR: she had the legs on us and made jump before we closed.
    - DRIVEN OFF — MAULED: you pressed the attack and they beat you back with nothing to show but a repair bill.
  - The act of piracy, when you took her: how hard you pressed — extorted a little, took the whole hold, crippled her drives, or destroyed her outright. Match the act tier in the data; do not invent atrocities the data doesn't show.
- Threat encounters (System Defence Boat, Naval Patrol, q-ship): play these for tension. Did you play it cool and slip past unrecognized, or were you made and forced to burn for the jump point? Tie the outcome and any damage/weeks lost to the data.
- Fence deals: where you put in to offload the hold, the law level you risked, the cut you got — or the sting that cost you the whole haul.
- Reputation: note how your name grew (or how you lay low to let the heat cool), matching the reputation figures.

== ROUTINE STRETCHES ==
Not every system has prey. A quiet sweep is two or three sentences — lurking at the jump point, scopes empty, fuel burning, crew restless. Don't pad it.

== MAROONED ENDINGS — THIS IS A STORY HOOK ==
If the cruise data shows the ship MAROONED, the final paragraph is the most important thing in the entry. Marooning can come two ways: the coffers ran dry, OR you were shot up badly in a high-law system with no friendly port to dock for repairs and had to slink into the outer system to lick your wounds and send for help. Write it as a vivid, in-character distress dispatch — bait for live RPG players, not an obituary. Hit these beats: where exactly the ship is stranded (which body, how far out); the condition of ship and crew (power, hull, drives, life support, morale); what's working and what isn't (be specific and canon); that a call for aid has been handed off by x-boat; and who might answer first — old partners, a rival crew, a fence who owes you, or a navy that would rather see you hang than rescued. The data line about the distress signal is when the call is first RECEIVED, not when help arrives — reflect on the long, uncertain wait. Leave it unresolved and open. This paragraph can run longer than the others.

== HARD RULES ==
- 3 to 10 paragraphs total. Maximum 200 sentences across the entire log (the marooned paragraph counts toward this).
- Do not list raids or loot as a table or bullet list. Weave them into prose.
- Do not dump every line item. The structured data has the receipts; the log is the story. Single out one or two highlights ("we let the liner pass and waited for the fat ore-hauler behind her — best haul of the cruise").
- Pull numbers from the data — do not invent credit figures.
- Plain prose, no markdown headers, no bullets, no bolding. Just paragraphs.
- Output ONLY the log text. No preamble like "Here is the log:".
- Do not end the log with text like "Log ends" but instead an Omega character: Ω
"#;
