//! System Generator page — constraint-driven UI.
//!
//! The user describes any number of bodies (a star, the main world,
//! ordinary planets, gas giants, moons) by adding rows to a table.
//! Each row's "Type" dropdown selects the kind of body; the inputs to
//! the right of the dropdown change to match. Generate builds a
//! `SystemConstraints` and calls `System::generate_from_constraints`;
//! errors render inline next to their row plus a summary by the button,
//! and Generate stays disabled while any error is unresolved.
//!
//! A Traveller Map autocomplete strip sits above the table — bound
//! one-way to whichever row currently has kind=MainWorld, so picking a
//! canonical world there fills that row's name and UWP.

use leptos::prelude::*;
use reactive_stores::Store;

use crate::INITIAL_NAME;
use crate::INITIAL_UWP;
use crate::components::system_view::SystemView;
use crate::components::traveller_map::WorldSearch;
use crate::systems::constraint::{Constraint, PartialUwp, SystemConstraints};
use crate::systems::gas_giant::GasGiantSize;
use crate::systems::system::{StarOrbit, StarSize, StarType, System};
use crate::systems::world::World;
use crate::trade::ZoneClassification;

/// Row dropdown options. `MainWorld` is just `Planet` with the
/// `is_mainworld` flag flipped; we surface it as its own dropdown
/// option because that's how the user thinks about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowKind {
    Star,
    MainWorld,
    Planet,
    Belt,
    GasGiant,
    Moon,
    Empty,
}

impl RowKind {
    const ALL: &'static [RowKind] = &[
        RowKind::Star,
        RowKind::MainWorld,
        RowKind::Planet,
        RowKind::Belt,
        RowKind::GasGiant,
        RowKind::Moon,
        RowKind::Empty,
    ];

    fn label(self) -> &'static str {
        match self {
            RowKind::Star => "Star",
            RowKind::MainWorld => "Main world",
            RowKind::Planet => "Planet",
            RowKind::Belt => "Belt",
            RowKind::GasGiant => "Gas giant",
            RowKind::Moon => "Moon",
            RowKind::Empty => "Empty orbit",
        }
    }

    fn from_label(s: &str) -> Self {
        Self::ALL
            .iter()
            .copied()
            .find(|k| k.label() == s)
            .unwrap_or(RowKind::Planet)
    }
}

/// All per-row inputs. Many are unused for any given kind — empty
/// string is the "unspecified" sentinel everywhere so we don't need a
/// per-kind struct.
#[derive(Clone, Copy)]
struct ConstraintRow {
    id: u32,
    kind: RwSignal<RowKind>,
    name: RwSignal<String>,
    /// Planet/GasGiant orbit, OR Star orbit (when kind=Star and orbit-kind=System).
    orbit: RwSignal<String>,
    uwp: RwSignal<String>,
    num_moons: RwSignal<String>,
    parent_orbit: RwSignal<String>,
    /// "Auto" (None — backend rolls), "Primary", or "System" (specific orbit number).
    star_orbit_kind: RwSignal<String>,
    star_type: RwSignal<String>, // "" for auto, else one of O/B/A/F/G/K/M
    star_subtype: RwSignal<String>, // "" for auto, else "0".."9"
    star_size: RwSignal<String>, // "" for auto, else Ia/Ib/II/III/IV/V/VI/D
    /// Gas-giant size dropdown: "Auto" / "Small" / "Large".
    gg_size: RwSignal<String>,
}

impl ConstraintRow {
    fn new(id: u32, kind: RowKind) -> Self {
        ConstraintRow {
            id,
            kind: RwSignal::new(kind),
            name: RwSignal::new(String::new()),
            orbit: RwSignal::new(String::new()),
            uwp: RwSignal::new(String::new()),
            num_moons: RwSignal::new(String::new()),
            parent_orbit: RwSignal::new(String::new()),
            star_orbit_kind: RwSignal::new("Primary".to_string()),
            star_type: RwSignal::new(String::new()),
            star_subtype: RwSignal::new(String::new()),
            star_size: RwSignal::new(String::new()),
            gg_size: RwSignal::new("Auto".to_string()),
        }
    }

    fn default_main_world(id: u32) -> Self {
        let row = Self::new(id, RowKind::MainWorld);
        row.name.set(INITIAL_NAME.to_string());
        row.uwp.set(INITIAL_UWP.to_string());
        row
    }
}

/// Convert a row's signals into a `Constraint`. Returns `Ok(None)` for
/// rows so empty they shouldn't appear in the constraints set; returns
/// `Err` with a human-readable message for rows that don't parse.
fn row_to_constraint(row: &ConstraintRow) -> Result<Option<Constraint>, String> {
    let kind = row.kind.get();
    match kind {
        RowKind::Star => {
            let orbit = match row.star_orbit_kind.get().as_str() {
                "Auto" => None,
                "Primary" => Some(StarOrbit::Primary),
                "Far" => Some(StarOrbit::Far),
                "System" => {
                    let s = row.orbit.get();
                    let n = s
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| format!("orbit must be a non-negative integer (got '{s}')"))?;
                    Some(StarOrbit::System(n))
                }
                other => return Err(format!("unknown star orbit kind '{other}'")),
            };
            let spectral = parse_star_type(&row.star_type.get())?;
            let size = parse_star_size(&row.star_size.get())?;
            let subtype = parse_star_subtype(&row.star_subtype.get())?;
            Ok(Some(Constraint::Star {
                orbit,
                spectral,
                subtype,
                size,
            }))
        }
        RowKind::MainWorld | RowKind::Planet => {
            let is_mainworld = matches!(kind, RowKind::MainWorld);
            let name = trim_to_opt(&row.name.get());
            let orbit = parse_opt_i32(&row.orbit.get(), "orbit")?;
            let uwp = parse_opt_uwp(&row.uwp.get())?;
            let num_satellites = parse_opt_i32(&row.num_moons.get(), "moons")?;

            // Skip rows with literally nothing in them.
            if name.is_none()
                && orbit.is_none()
                && uwp.is_none()
                && num_satellites.is_none()
                && !is_mainworld
            {
                return Ok(None);
            }
            Ok(Some(Constraint::Planet {
                name,
                orbit,
                uwp,
                num_satellites,
                is_mainworld,
            }))
        }
        RowKind::GasGiant => {
            let name = trim_to_opt(&row.name.get());
            let orbit = parse_opt_i32(&row.orbit.get(), "orbit")?;
            let size = match row.gg_size.get().as_str() {
                "Small" => Some(GasGiantSize::Small),
                "Large" => Some(GasGiantSize::Large),
                _ => None, // "Auto" or any unexpected value falls through to "roll it"
            };
            let num_satellites = parse_opt_i32(&row.num_moons.get(), "moons")?;
            Ok(Some(Constraint::GasGiant {
                name,
                orbit,
                size,
                num_satellites,
            }))
        }
        RowKind::Moon => {
            let name = trim_to_opt(&row.name.get());
            let parent_str = row.parent_orbit.get();
            let parent_orbit = parent_str.trim().parse::<i32>().map_err(|_| {
                format!("parent orbit is required and must be an integer (got '{parent_str}')")
            })?;
            if parent_orbit < 0 {
                return Err(format!(
                    "parent orbit must be non-negative (got {parent_orbit})"
                ));
            }
            let uwp = parse_opt_uwp(&row.uwp.get())?;
            Ok(Some(Constraint::Moon {
                name,
                parent_orbit,
                uwp,
            }))
        }
        RowKind::Belt => {
            // Belts always use the canonical "Planetoid Belt" name —
            // we ignore whatever text is sitting in row.name (the
            // name input is disabled in the UI).
            let orbit = parse_opt_i32(&row.orbit.get(), "orbit")?;
            let uwp = parse_opt_uwp(&row.uwp.get())?;
            let num_satellites = parse_opt_i32(&row.num_moons.get(), "moons")?;
            Ok(Some(Constraint::Belt {
                name: None,
                orbit,
                uwp,
                num_satellites,
            }))
        }
        RowKind::Empty => {
            let orbit_str = row.orbit.get();
            let orbit = orbit_str.trim().parse::<i32>().map_err(|_| {
                format!("orbit is required and must be an integer (got '{orbit_str}')")
            })?;
            if orbit < 0 {
                return Err(format!("orbit must be non-negative (got {orbit})"));
            }
            Ok(Some(Constraint::Empty { orbit }))
        }
    }
}

fn trim_to_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn parse_opt_i32(s: &str, label: &str) -> Result<Option<i32>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<i32>()
        .map(Some)
        .map_err(|_| format!("{label} must be an integer (got '{t}')"))
}

fn parse_opt_uwp(s: &str) -> Result<Option<PartialUwp>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    PartialUwp::parse(t).map(Some)
}

fn parse_star_type(s: &str) -> Result<Option<StarType>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    Ok(Some(match t {
        "O" => StarType::O,
        "B" => StarType::B,
        "A" => StarType::A,
        "F" => StarType::F,
        "G" => StarType::G,
        "K" => StarType::K,
        "M" => StarType::M,
        other => return Err(format!("unknown star type '{other}'")),
    }))
}

// Stellar-string parsing lives in `crate::api::parse_stellar` so the
// backend HTTP endpoint and library consumers can share it. We import
// it as `parse_stellar` for the call site below.
use crate::api::parse_stellar;

/// Parse a Traveller-Map "PBG" string (3 hex digits: population
/// multiplier, planetoid belts, gas giants). Returns `None` for
/// anything that doesn't fit.
fn parse_pbg(s: &str) -> Option<(u32, u32, u32)> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 3 {
        return None;
    }
    let pop = chars[0].to_digit(16)?;
    let belts = chars[1].to_digit(16)?;
    let giants = chars[2].to_digit(16)?;
    Some((pop, belts, giants))
}

fn parse_star_subtype(s: &str) -> Result<Option<u8>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let n = t
        .parse::<u8>()
        .map_err(|_| format!("subtype must be a digit 0-9 (got '{t}')"))?;
    if n > 9 {
        return Err(format!("subtype must be 0-9 (got {n})"));
    }
    Ok(Some(n))
}

fn parse_star_size(s: &str) -> Result<Option<StarSize>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    Ok(Some(match t {
        "Ia" => StarSize::Ia,
        "Ib" => StarSize::Ib,
        "II" => StarSize::II,
        "III" => StarSize::III,
        "IV" => StarSize::IV,
        "V" => StarSize::V,
        "VI" => StarSize::VI,
        "D" => StarSize::D,
        other => return Err(format!("unknown star size '{other}'")),
    }))
}

#[component]
pub fn World() -> impl IntoView {
    provide_context(Store::new(
        World::from_uwp(INITIAL_NAME, INITIAL_UWP, false, true).unwrap(),
    ));
    provide_context(Store::new(System::default()));

    let system = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();

    let next_id = RwSignal::new(1u32);
    let rows: RwSignal<Vec<ConstraintRow>> =
        RwSignal::new(vec![ConstraintRow::default_main_world(0)]);

    let row_errors = RwSignal::new(Vec::<(u32, String)>::new());
    let global_errors = RwSignal::new(Vec::<String>::new());

    // Traveller Map autocomplete signals — bound to WorldSearch above
    // the table. When the user picks a result, the Effect below pushes
    // the data into whichever row currently has kind=MainWorld.
    let tm_name = RwSignal::new(INITIAL_NAME.to_string());
    let tm_uwp = RwSignal::new(INITIAL_UWP.to_string());
    let tm_coords = RwSignal::new(None::<(i32, i32)>);
    let tm_zone = RwSignal::new(ZoneClassification::Green);
    let tm_stellar = RwSignal::new(None::<String>);
    let tm_pbg = RwSignal::new(None::<String>);

    // A successful Traveller-Map lookup is the source of truth: every
    // valid pick rebuilds the entire constraint table from scratch
    // (Main World + Stars + GasGiants). Any rows the user added by
    // hand are cleared too — picking a new world means starting over.
    // Fires when stellar OR pbg arrives, which is the signal that
    // fetch_data_world succeeded; partial-search edits (typing without
    // selecting a result) leave the table alone.
    Effect::new(move |_| {
        let stellar = tm_stellar.get();
        let pbg = tm_pbg.get();
        if stellar.is_none() && pbg.is_none() {
            return;
        }

        let stars = stellar.as_deref().map(parse_stellar).unwrap_or_default();
        let (_, _belts, giants) = pbg.as_deref().and_then(parse_pbg).unwrap_or((0, 0, 0));
        let n = tm_name.get_untracked();
        let u = tm_uwp.get_untracked();

        rows.update(|rs| {
            rs.clear();

            // Main world first.
            let id = next_id.get_untracked();
            next_id.set(id + 1);
            let mw = ConstraintRow::new(id, RowKind::MainWorld);
            mw.name.set(if n.is_empty() {
                INITIAL_NAME.to_string()
            } else {
                n
            });
            mw.uwp.set(if u.is_empty() {
                INITIAL_UWP.to_string()
            } else {
                u
            });
            rs.push(mw);

            // Then a Star row per parsed entry — first becomes the
            // primary the main world orbits; companions roll their
            // own placement.
            for (idx, parsed) in stars.iter().enumerate() {
                let id = next_id.get_untracked();
                next_id.set(id + 1);
                let row = ConstraintRow::new(id, RowKind::Star);
                row.star_orbit_kind.set(if idx == 0 {
                    "Primary".to_string()
                } else {
                    "Auto".to_string()
                });
                row.star_type.set(format!("{}", parsed.spectral));
                row.star_subtype
                    .set(parsed.subtype.map(|n| n.to_string()).unwrap_or_default());
                row.star_size.set(format!("{}", parsed.size));
                rs.push(row);
            }

            // Then one GasGiant row per gas giant in PBG. Size dropdown
            // stays at Auto — TM's pbg digit doesn't distinguish small
            // vs large, so we let the backend roll.
            for _ in 0..giants {
                let id = next_id.get_untracked();
                next_id.set(id + 1);
                rs.push(ConstraintRow::new(id, RowKind::GasGiant));
            }
        });
    });

    let has_main_world = Signal::derive(move || {
        rows.with(|rs| rs.iter().any(|r| r.kind.get() == RowKind::MainWorld))
    });

    let add_row = move |_| {
        let id = next_id.get();
        next_id.set(id + 1);
        rows.update(|rs| rs.push(ConstraintRow::new(id, RowKind::Planet)));
    };

    // Drop the per-row error for `id` (and the global "fix per-row
    // errors above" banner if no per-row errors remain). Used by
    // `remove_row` and by `revalidate_row` below when a row's inputs
    // become valid again.
    let clear_row_error = move |id: u32| {
        row_errors.update(|errs| errs.retain(|(rid, _)| *rid != id));
        if row_errors.with(Vec::is_empty) {
            global_errors.set(vec![]);
        }
    };

    // Re-validate one row after the user edits any of its inputs.
    //   - Valid now → clear the row's error (so Generate re-enables
    //     and the red message disappears).
    //   - Still invalid → if the row already had an error, refresh the
    //     message (so a partial fix shows a helpful new error). We
    //     deliberately *don't* surface a new error mid-typing if the
    //     row didn't have one yet; that would surprise the user.
    //     Generate still produces fresh errors as it always did.
    let revalidate_row = move |id: u32| {
        let row_opt = rows.with_untracked(|rs| rs.iter().find(|r| r.id == id).copied());
        let Some(row) = row_opt else {
            return;
        };
        match row_to_constraint(&row) {
            Ok(_) => clear_row_error(id),
            Err(msg) => {
                row_errors.update(|errs| {
                    if let Some(entry) = errs.iter_mut().find(|(rid, _)| *rid == id) {
                        entry.1 = msg;
                    }
                });
            }
        }
    };

    let remove_row = move |id: u32| {
        rows.update(|rs| rs.retain(|r| r.id != id));
        clear_row_error(id);
    };

    let do_generate = move || {
        let mut row_errs = Vec::new();
        let mut bodies = Vec::new();
        for row in rows.get_untracked().iter() {
            match row_to_constraint(row) {
                Ok(Some(c)) => bodies.push(c),
                Ok(None) => {}
                Err(msg) => row_errs.push((row.id, msg)),
            }
        }

        if !row_errs.is_empty() {
            row_errors.set(row_errs);
            global_errors.set(vec!["fix per-row errors above".to_string()]);
            return;
        }
        row_errors.set(vec![]);

        let constraints = SystemConstraints { bodies };
        match System::generate_from_constraints(constraints) {
            Ok(sys) => {
                global_errors.set(vec![]);
                if let Some(mw) = find_main_world(&sys) {
                    main_world.set(mw);
                }
                system.set(sys);
            }
            Err(errs) => {
                global_errors.set(errs.iter().map(|e| e.to_string()).collect());
            }
        }
    };

    // Cold-start render with the default constraint set so the page
    // doesn't land blank. Subsequent generations require the button.
    Effect::new(move |prev: Option<()>| {
        if prev.is_none() {
            do_generate();
        }
    });

    let on_generate = move |_| do_generate();

    // Generate stays disabled only while per-row inputs fail to parse
    // (a syntactic error the user must fix before the backend can
    // possibly succeed). Backend rejections (`ConstraintError`) are
    // shown next to the button but do NOT block — the user changes
    // inputs and clicks again.
    let any_row_errors = Signal::derive(move || !row_errors.get().is_empty());

    view! {
        <div class:App>
            <h1 class="d-print-none">"Solar System Generator"</h1>
            <p class="d-print-none generator-intro">
                "Load a system from TravellerMap below if you want to start from canon. \
                 Then add additional constraints about the system. The rest of the system \
                 will be generated based on those constraints. This generator uses \
                 Classic Traveller Book 6 methodology."
            </p>
            <Show when=move || has_main_world.get()>
                <div class="d-print-none key-region world-entry-form">
                    <WorldSearch
                        label="TravellerMap Lookup".to_string()
                        name=tm_name
                        uwp=tm_uwp
                        coords=tm_coords
                        zone=tm_zone
                        stellar=tm_stellar
                        pbg=tm_pbg
                        show_uwp=false
                    />
                </div>
            </Show>
            <div class="d-print-none constraint-panel">
                <div class="constraint-table">
                    <For
                        each=move || rows.get()
                        key=|r| r.id
                        let:row
                    >
                        <ConstraintRowView
                            row=row
                            row_errors=row_errors
                            on_remove=Callback::new(move |id: u32| remove_row(id))
                            on_edit=Callback::new(move |id: u32| revalidate_row(id))
                        />
                    </For>
                </div>
                <button class="add-row-button" on:click=add_row title="Add another constraint row">"+"</button>
                <div class="constraint-actions">
                    <button class="blue-button" on:click=on_generate prop:disabled=move || any_row_errors.get()>"Generate"</button>
                    <Show when=move || !global_errors.get().is_empty()>
                        <ul class="constraint-error-summary">
                            {move || global_errors.get().into_iter().map(|e| view! { <li>{e}</li> }).collect_view()}
                        </ul>
                    </Show>
                </div>
            </div>
            <SystemView />
        </div>
    }
}

/// Pull the main world out of a generated system so the existing
/// `Store<World>` consumers (trade classes, etc.) keep working. Walks
/// the orbit slots top-down and returns the first World marked
/// main-world; falls back to whatever World we find first.
fn find_main_world(sys: &System) -> Option<World> {
    use crate::systems::system::OrbitContent;
    let mut fallback: Option<World> = None;
    for slot in sys.orbit_slots.iter().flatten() {
        if let OrbitContent::World(w) = slot {
            // is_mainworld is private — best we can do without exposing
            // it is to assume the first World in orbit order is it,
            // which matches today's single-mainworld flow.
            if fallback.is_none() {
                fallback = Some(w.clone());
            }
        }
    }
    fallback
}

#[component]
fn ConstraintRowView(
    row: ConstraintRow,
    row_errors: RwSignal<Vec<(u32, String)>>,
    on_remove: Callback<u32>,
    on_edit: Callback<u32>,
) -> impl IntoView {
    let kind = row.kind;
    let id = row.id;

    let remove = move |_| on_remove.run(id);

    // Re-validate the row whenever any of its inputs fire `input` or
    // `change`. Both events bubble in HTML, so a single pair of
    // listeners on the wrapper `<div>` catches edits from every text
    // input and dropdown rendered inside. We use this bubble-listener
    // instead of an Effect subscribed to row signals because the
    // signal-Effect approach ran in a microtask that could fire
    // *after* `do_generate` set the row's error, silently swallowing
    // it. Synchronous DOM-event handlers don't race with
    // `do_generate`.
    let trigger_revalidate = move |_: web_sys::Event| on_edit.run(id);

    let row_error_text = Signal::derive(move || {
        row_errors
            .get()
            .into_iter()
            .find(|(rid, _)| *rid == id)
            .map(|(_, msg)| msg)
    });

    let kind_select = view! {
        <select
            class="kind-select"
            on:change=move |ev| {
                kind.set(RowKind::from_label(&event_target_value(&ev)));
            }
        >
            {RowKind::ALL.iter().map(|k| {
                let label = k.label();
                let k_copy = *k;
                view! {
                    <option
                        value=label
                        selected=move || kind.get() == k_copy
                    >{label}</option>
                }
            }).collect_view()}
        </select>
    };

    view! {
        <div class="constraint-row-wrapper"
            on:input=trigger_revalidate
            on:change=trigger_revalidate>
            <div class="constraint-row">
                {kind_select}
                {move || render_kind_inputs(row)}
                <span class="row-actions">
                    <button
                        class="remove-row-button"
                        on:click=remove
                        title="Remove this row"
                    >"×"</button>
                </span>
            </div>
            {move || row_error_text.get().map(|msg| view! {
                <div class="constraint-row-error">{msg}</div>
            })}
        </div>
    }
}

/// Render the kind-specific inputs for a row. Returns a fragment that
/// slots between the kind select and the action cluster on the right.
fn render_kind_inputs(row: ConstraintRow) -> impl IntoView {
    let kind = row.kind;
    move || match kind.get() {
        RowKind::Star => view! { <StarInputs row=row /> }.into_any(),
        RowKind::MainWorld | RowKind::Planet => view! { <PlanetInputs row=row /> }.into_any(),
        RowKind::Belt => view! { <BeltInputs row=row /> }.into_any(),
        RowKind::GasGiant => view! { <GasGiantInputs row=row /> }.into_any(),
        RowKind::Moon => view! { <MoonInputs row=row /> }.into_any(),
        RowKind::Empty => view! { <EmptyInputs row=row /> }.into_any(),
    }
}

#[component]
fn PlanetInputs(row: ConstraintRow) -> impl IntoView {
    view! {
        <input class="cell-orbit" type="text" placeholder="orbit"
            prop:value=move || row.orbit.get()
            on:input=move |ev| row.orbit.set(event_target_value(&ev)) />
        <input class="cell-name" type="text" placeholder="name"
            prop:value=move || row.name.get()
            on:input=move |ev| row.name.set(event_target_value(&ev)) />
        <input class="cell-uwp" type="text" placeholder="UWP (X for wild)"
            prop:value=move || row.uwp.get()
            on:input=move |ev| row.uwp.set(event_target_value(&ev)) />
        <input class="cell-moons" type="text" placeholder="moons"
            prop:value=move || row.num_moons.get()
            on:input=move |ev| row.num_moons.set(event_target_value(&ev)) />
    }
}

#[component]
fn GasGiantInputs(row: ConstraintRow) -> impl IntoView {
    let gg_size = row.gg_size;
    let moons_placeholder = move || match gg_size.get().as_str() {
        "Small" => "moons (typ. ~3)",
        "Large" => "moons (typ. ~7)",
        _ => "moons",
    };
    let moons_title = "Small giants: 2d6-4 moons (avg ~3, max 8). Large giants: 2d6 moons (avg ~7, max 12). Auto rolls size and uses size's distribution.";
    view! {
        <input class="cell-orbit" type="text" placeholder="orbit"
            prop:value=move || row.orbit.get()
            on:input=move |ev| row.orbit.set(event_target_value(&ev)) />
        <input class="cell-name" type="text" placeholder="name"
            prop:value=move || row.name.get()
            on:input=move |ev| row.name.set(event_target_value(&ev)) />
        <select class="cell-uwp"
            on:change=move |ev| gg_size.set(event_target_value(&ev))
        >
            <option value="Auto" selected=move || gg_size.get() == "Auto">"Auto"</option>
            <option value="Small" selected=move || gg_size.get() == "Small">"Small"</option>
            <option value="Large" selected=move || gg_size.get() == "Large">"Large"</option>
        </select>
        <input class="cell-moons" type="text"
            prop:placeholder=moons_placeholder
            title=moons_title
            prop:value=move || row.num_moons.get()
            on:input=move |ev| row.num_moons.set(event_target_value(&ev)) />
    }
}

#[component]
fn BeltInputs(row: ConstraintRow) -> impl IntoView {
    view! {
        <input class="cell-orbit" type="text" placeholder="orbit"
            prop:value=move || row.orbit.get()
            on:input=move |ev| row.orbit.set(event_target_value(&ev)) />
        // Belts use the canonical "Planetoid Belt" name — no custom
        // naming. Render a disabled placeholder so the table columns
        // still line up.
        <input class="cell-name" type="text"
            disabled
            placeholder="Planetoid Belt"
            title="Belts are always named 'Planetoid Belt' — they're not single objects with a single name." />
        <input class="cell-uwp" type="text"
            placeholder="UWP (size pinned 0)"
            title="Belts are always size 0; the size column of the UWP is overridden. Other columns work like a partial Planet UWP."
            prop:value=move || row.uwp.get()
            on:input=move |ev| row.uwp.set(event_target_value(&ev)) />
        <input class="cell-moons" type="text" placeholder="moons"
            prop:value=move || row.num_moons.get()
            on:input=move |ev| row.num_moons.set(event_target_value(&ev)) />
    }
}

#[component]
fn EmptyInputs(row: ConstraintRow) -> impl IntoView {
    view! {
        <input class="cell-orbit" type="text" placeholder="orbit"
            title="Required: which orbit number to leave deliberately empty."
            prop:value=move || row.orbit.get()
            on:input=move |ev| row.orbit.set(event_target_value(&ev)) />
        <span class="cell-name" />
        <span class="cell-uwp" />
        <span class="cell-moons" />
    }
}

#[component]
fn MoonInputs(row: ConstraintRow) -> impl IntoView {
    view! {
        <input class="cell-orbit" type="text" placeholder="parent orbit"
            prop:value=move || row.parent_orbit.get()
            on:input=move |ev| row.parent_orbit.set(event_target_value(&ev)) />
        <input class="cell-name" type="text" placeholder="name"
            prop:value=move || row.name.get()
            on:input=move |ev| row.name.set(event_target_value(&ev)) />
        <input class="cell-uwp" type="text" placeholder="UWP (X for wild)"
            prop:value=move || row.uwp.get()
            on:input=move |ev| row.uwp.set(event_target_value(&ev)) />
        <span class="cell-moons" />
    }
}

#[component]
fn StarInputs(row: ConstraintRow) -> impl IntoView {
    let orbit_kind = row.star_orbit_kind;
    view! {
        <select class="cell-orbit"
            on:change=move |ev| orbit_kind.set(event_target_value(&ev))
        >
            <option value="Auto" selected=move || orbit_kind.get() == "Auto">"Auto"</option>
            <option value="Primary" selected=move || orbit_kind.get() == "Primary">"Primary"</option>
            <option value="System" selected=move || orbit_kind.get() == "System">"Orbit #"</option>
            <option value="Far" selected=move || orbit_kind.get() == "Far">"Far"</option>
        </select>
        // Always reserve the orbit-number cell so star rows align with
        // planet rows; disable it when orbit-kind isn't a specific orbit.
        <input class="cell-name" type="text"
            placeholder="orbit number"
            prop:disabled=move || orbit_kind.get() != "System"
            prop:value=move || row.orbit.get()
            on:input=move |ev| row.orbit.set(event_target_value(&ev)) />
        <span class="cell-uwp star-class-group">
            <select class="star-type-select"
                on:change=move |ev| row.star_type.set(event_target_value(&ev))
            >
                {[("", "Type (auto)"), ("O","O"), ("B","B"), ("A","A"), ("F","F"), ("G","G"), ("K","K"), ("M","M")].iter().map(|(v,l)| {
                    let v = v.to_string();
                    let l = l.to_string();
                    let v_for_sel = v.clone();
                    view! { <option value=v.clone() selected=move || row.star_type.get() == v_for_sel>{l}</option> }
                }).collect_view()}
            </select>
            <input class="star-subtype-input" type="text"
                placeholder="0-9"
                title="Spectral subtype: 0 (hottest) through 9 (coolest) within the type. Leave blank to roll."
                prop:value=move || row.star_subtype.get()
                on:input=move |ev| row.star_subtype.set(event_target_value(&ev)) />
        </span>
        <select class="cell-moons"
            on:change=move |ev| row.star_size.set(event_target_value(&ev))
        >
            {[("", "Size (auto)"), ("Ia","Ia"), ("Ib","Ib"), ("II","II"), ("III","III"), ("IV","IV"), ("V","V"), ("VI","VI"), ("D","D")].iter().map(|(v,l)| {
                let v = v.to_string();
                let l = l.to_string();
                let v_for_sel = v.clone();
                view! { <option value=v.clone() selected=move || row.star_size.get() == v_for_sel>{l}</option> }
            }).collect_view()}
        </select>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `parse_stellar` itself is tested in `src/api.rs` (the shared
    // implementation). We keep one Noricum regression test here to
    // pin the call-site behaviour (the frontend consumes the parsed
    // stars via the `StarSpec` shape).
    #[test]
    fn parse_stellar_noricum_three_stars() {
        let s = parse_stellar("G2 V M9 V M6 V");
        assert_eq!(s.len(), 3);
        assert!(matches!(s[0].spectral, StarType::G));
        assert_eq!(s[0].subtype, Some(2));
        assert!(matches!(s[0].size, StarSize::V));
        assert!(matches!(s[1].spectral, StarType::M));
        assert_eq!(s[1].subtype, Some(9));
        assert!(matches!(s[2].spectral, StarType::M));
        assert_eq!(s[2].subtype, Some(6));
    }

    #[test]
    fn parse_pbg_extracts_three_digits() {
        let p = parse_pbg("503").unwrap();
        assert_eq!(p, (5, 0, 3));
    }

    #[test]
    fn parse_pbg_rejects_wrong_length() {
        assert!(parse_pbg("50").is_none());
        assert!(parse_pbg("5031").is_none());
    }

    #[test]
    fn parse_pbg_handles_hex() {
        // pop multiplier can be hex (B = 11)
        let p = parse_pbg("B23").unwrap();
        assert_eq!(p, (11, 2, 3));
    }
}
