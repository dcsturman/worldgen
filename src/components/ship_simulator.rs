//! Ship Simulator UI.
//!
//! A self-contained Leptos page that:
//! 1. Collects [`SimulationParams`] from a form.
//! 2. Opens a WebSocket to `/ws/simulator` and streams the simulation.
//! 3. Renders each [`SimulationStep`] as it arrives.
//! 4. Shows a final summary with a "Save as PDF" (browser print) button.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use log::{error, info};
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::components::traveller_map::WorldSearch;
use crate::simulator::protocol::{ClientMessage, ServerMessage};
use crate::simulator::types::{
    Action, Date, SimulationParams, SimulationResult, SimulationStep, WorldRef,
};
use crate::trade::ZoneClassification;

/// High-level run state of the simulator.
#[derive(Debug, Clone)]
enum RunState {
    /// No simulation has been started yet.
    Idle,
    /// WebSocket is opening / sending the run request.
    Connecting,
    /// Simulation is streaming. Tracks how many steps we've seen.
    Running { steps_seen: u32 },
    /// Simulation finished cleanly with a result.
    Done(SimulationResult),
    /// Simulation errored or the socket closed without a `Done`.
    Errored(String),
}

/// Get the WebSocket URL for the simulator endpoint.
///
/// Mirrors `bin/main.rs::get_ws_url` but for `/ws/simulator`.
fn get_ws_url() -> String {
    if let Some(window) = web_sys::window()
        && let Ok(location) = window.location().host()
    {
        let protocol = if window.location().protocol().unwrap_or_default() == "https:" {
            "wss"
        } else {
            "ws"
        };

        // Local development mode: connect directly to backend on 8081
        #[cfg(feature = "local-dev")]
        {
            if location.starts_with("localhost") {
                return "ws://localhost:8081/ws/simulator".to_string();
            }
        }

        // Docker/Production: connect to same host (nginx proxies /ws/* to backend)
        return format!("{}://{}/ws/simulator", protocol, location);
    }
    // Fallback
    "ws://localhost:8081/ws/simulator".to_string()
}

/// Lightweight per-run WebSocket client. The closures must be kept alive
/// for the lifetime of the WebSocket; storing them on the struct does that.
#[allow(dead_code)]
struct SimClient {
    ws: WebSocket,
    on_open: Closure<dyn FnMut()>,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_close: Closure<dyn FnMut(CloseEvent)>,
    on_error: Closure<dyn FnMut(ErrorEvent)>,
}

impl SimClient {
    /// Open a new WebSocket and send `RunSimulation(params)` once it opens.
    /// All received messages are dispatched to the provided signals.
    fn start(
        params: SimulationParams,
        run_state: RwSignal<RunState>,
        steps: RwSignal<Vec<SimulationStep>>,
    ) -> Result<Self, String> {
        let url = get_ws_url();
        info!("Simulator connecting to {}", url);
        let ws = WebSocket::new(&url).map_err(|e| format!("Failed to open WebSocket: {:?}", e))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Track whether we received a terminal message (Done/Error) so we can
        // distinguish a clean close from a premature one.
        let got_terminal: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        // ---- on_open: send the params as a RunSimulation message ----
        let ws_for_open = ws.clone();
        let params_for_open = params;
        let run_state_for_open = run_state;
        let on_open = Closure::<dyn FnMut()>::new(move || {
            let msg = ClientMessage::RunSimulation(params_for_open.clone());
            match serde_json::to_string(&msg) {
                Ok(json) => match ws_for_open.send_with_str(&json) {
                    Ok(_) => {
                        info!("Sent RunSimulation request");
                        run_state_for_open.set(RunState::Running { steps_seen: 0 });
                    }
                    Err(e) => {
                        error!("Failed to send RunSimulation: {:?}", e);
                        run_state_for_open.set(RunState::Errored(format!("Send failed: {:?}", e)));
                    }
                },
                Err(e) => {
                    error!("Failed to serialize RunSimulation: {}", e);
                    run_state_for_open.set(RunState::Errored(format!("Serialize failed: {}", e)));
                }
            }
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        // ---- on_message: parse ServerMessage and dispatch ----
        let got_terminal_for_msg = got_terminal.clone();
        let on_message = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            let Some(text) = e.data().as_string() else {
                error!("Received non-text WebSocket frame; ignoring");
                return;
            };
            match serde_json::from_str::<ServerMessage>(&text) {
                Ok(ServerMessage::Step(step)) => {
                    steps.update(|v| v.push(step));
                    run_state.update(|s| {
                        if let RunState::Running { steps_seen } = s {
                            *steps_seen += 1;
                        } else {
                            *s = RunState::Running { steps_seen: 1 };
                        }
                    });
                }
                Ok(ServerMessage::Done(result)) => {
                    *got_terminal_for_msg.borrow_mut() = true;
                    info!("Simulation done: {} jumps", result.jumps);
                    run_state.set(RunState::Done(result));
                }
                Ok(ServerMessage::Error { message }) => {
                    *got_terminal_for_msg.borrow_mut() = true;
                    error!("Simulation error: {}", message);
                    run_state.set(RunState::Errored(message));
                }
                Err(err) => {
                    error!("Failed to parse ServerMessage: {} (text: {})", err, text);
                }
            }
        });
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        // ---- on_close: if we never got a terminal message, surface as error ----
        let got_terminal_for_close = got_terminal.clone();
        let on_close = Closure::<dyn FnMut(_)>::new(move |e: CloseEvent| {
            info!(
                "Simulator WebSocket closed: code={}, reason={}",
                e.code(),
                e.reason()
            );
            if !*got_terminal_for_close.borrow() {
                let reason = if e.reason().is_empty() {
                    format!("Connection closed (code {})", e.code())
                } else {
                    format!("Connection closed: {} (code {})", e.reason(), e.code())
                };
                run_state.update(|s| {
                    // Don't clobber a Done/Errored that already came in.
                    if !matches!(s, RunState::Done(_) | RunState::Errored(_)) {
                        *s = RunState::Errored(reason.clone());
                    }
                });
            }
        });
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        // ---- on_error: log; the close handler will set the error state ----
        let on_error = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            error!("Simulator WebSocket error: {:?}", e.message());
        });
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        Ok(Self {
            ws,
            on_open,
            on_message,
            on_close,
            on_error,
        })
    }
}

/// Parse a `"DDD-YYYY"` date string. The day must be in 0..=364 and the
/// year must fit in `u16`. Returns `None` for any malformed input — the
/// form uses this both for validation and for the live "invalid" styling
/// on the input.
fn parse_ddd_yyyy(s: &str) -> Option<Date> {
    let (day_part, year_part) = s.trim().split_once('-')?;
    let day: u16 = day_part.trim().parse().ok()?;
    let year: u16 = year_part.trim().parse().ok()?;
    if day > 364 {
        return None;
    }
    Some(Date::new(day, year))
}

/// Top-level simulator page. Owns the form + log + summary state.
#[component]
pub fn ShipSimulator() -> impl IntoView {
    // ---- Form state ----
    // Ship
    let cargo_capacity = RwSignal::new(80i32);
    let staterooms = RwSignal::new(4i32);
    let low_berths = RwSignal::new(4i32);
    let jump = RwSignal::new(2i32);
    let fuel_cost_per_parsec = RwSignal::new(500i64);
    let maintenance_per_period = RwSignal::new(5_000i64);
    let crew_salary_per_period = RwSignal::new(12_000i64);
    let crew_profit_share = RwSignal::new(0.10f32);

    // Crew
    let buyer_broker_skill = RwSignal::new(1i16);
    let seller_broker_skill = RwSignal::new(1i16);
    let steward_skill = RwSignal::new(1i16);

    // Voyage
    let starting_budget = RwSignal::new(500_000i64);
    // Dates entered as "DDD-YYYY" strings (matching `Date::format`).
    let start_date_text = RwSignal::new("001-1105".to_string());
    let target_date_text = RwSignal::new("090-1105".to_string());
    let illegal_goods = RwSignal::new(false);

    // Home world. Populated by the TravellerMap autocomplete (WorldSearch).
    // We seed Regina/Spinward Marches as a sensible default so users can hit
    // Run immediately without having to search.
    let home_name = RwSignal::new("Regina".to_string());
    let home_sector = RwSignal::new("Spinward Marches".to_string());
    let home_coords = RwSignal::new(Some((19i32, 10i32)));
    let home_uwp = RwSignal::new("A788899-A".to_string());
    let home_zone = RwSignal::new(ZoneClassification::Green);

    // ---- Run state ----
    let run_state = RwSignal::new(RunState::Idle);
    let steps = RwSignal::new(Vec::<SimulationStep>::new());

    // Hold the live client so its closures stay alive across renders.
    let client_holder: Rc<RefCell<Option<SimClient>>> = Rc::new(RefCell::new(None));

    // ---- Validation ----
    let is_valid = Memo::new(move |_| {
        cargo_capacity.get() > 0
            && staterooms.get() >= 0
            && low_berths.get() >= 0
            && jump.get() > 0
            && fuel_cost_per_parsec.get() >= 0
            && maintenance_per_period.get() >= 0
            && crew_salary_per_period.get() >= 0
            && (0.0..=1.0).contains(&crew_profit_share.get())
            && (-3..=5).contains(&buyer_broker_skill.get())
            && (-3..=5).contains(&seller_broker_skill.get())
            && (-3..=5).contains(&steward_skill.get())
            && {
                match (
                    parse_ddd_yyyy(&start_date_text.read()),
                    parse_ddd_yyyy(&target_date_text.read()),
                ) {
                    (Some(start), Some(target)) => start.days_until(target) > 0,
                    _ => false,
                }
            }
            && !home_name.read().trim().is_empty()
            && !home_sector.read().trim().is_empty()
            && home_coords.read().is_some()
            && home_uwp.read().len() == 9
    });

    // ---- Run button handler ----
    let client_holder_for_run = client_holder.clone();
    let run = move |_| {
        if !is_valid.get_untracked() {
            return;
        }
        // Reset state for a fresh run.
        steps.set(Vec::new());
        run_state.set(RunState::Connecting);

        let params = SimulationParams {
            buyer_broker_skill: buyer_broker_skill.get_untracked(),
            seller_broker_skill: seller_broker_skill.get_untracked(),
            steward_skill: steward_skill.get_untracked(),
            cargo_capacity: cargo_capacity.get_untracked(),
            staterooms: staterooms.get_untracked(),
            low_berths: low_berths.get_untracked(),
            jump: jump.get_untracked(),
            maintenance_per_period: maintenance_per_period.get_untracked(),
            crew_salary_per_period: crew_salary_per_period.get_untracked(),
            fuel_cost_per_parsec: fuel_cost_per_parsec.get_untracked(),
            crew_profit_share: crew_profit_share.get_untracked(),
            starting_budget: starting_budget.get_untracked(),
            home_world: {
                let (hex_x, hex_y) = home_coords.get_untracked().unwrap_or((0, 0));
                WorldRef {
                    name: home_name.get_untracked(),
                    uwp: home_uwp.get_untracked(),
                    sector: home_sector.get_untracked(),
                    hex_x,
                    hex_y,
                    zone: home_zone.get_untracked(),
                }
            },
            start_date: parse_ddd_yyyy(&start_date_text.get_untracked())
                .unwrap_or_else(|| Date::new(1, 1105)),
            target_completion_date: parse_ddd_yyyy(&target_date_text.get_untracked())
                .unwrap_or_else(|| Date::new(90, 1105)),
            illegal_goods: illegal_goods.get_untracked(),
        };

        match SimClient::start(params, run_state, steps) {
            Ok(client) => {
                *client_holder_for_run.borrow_mut() = Some(client);
            }
            Err(e) => {
                error!("Failed to start sim client: {}", e);
                run_state.set(RunState::Errored(e));
            }
        }
    };

    view! {
        <div class:App>
            <h1 class="no-print">"Ship Simulator"</h1>

            <SimForm
                cargo_capacity=cargo_capacity
                staterooms=staterooms
                low_berths=low_berths
                jump=jump
                fuel_cost_per_parsec=fuel_cost_per_parsec
                maintenance_per_period=maintenance_per_period
                crew_salary_per_period=crew_salary_per_period
                crew_profit_share=crew_profit_share
                buyer_broker_skill=buyer_broker_skill
                seller_broker_skill=seller_broker_skill
                steward_skill=steward_skill
                starting_budget=starting_budget
                start_date_text=start_date_text
                target_date_text=target_date_text
                illegal_goods=illegal_goods
                home_name=home_name
                home_sector=home_sector
                home_coords=home_coords
                home_uwp=home_uwp
                home_zone=home_zone
            />

            <div class="sim-controls no-print">
                <button
                    class="blue-button"
                    prop:disabled=move || {
                        !is_valid.get() || matches!(run_state.get(), RunState::Connecting | RunState::Running { .. })
                    }
                    on:click=run
                >
                    {move || match run_state.get() {
                        RunState::Connecting => "Connecting...".to_string(),
                        RunState::Running { steps_seen } => format!("Running ({} steps)...", steps_seen),
                        _ => "Run Simulation".to_string(),
                    }}
                </button>
                <span class="sim-status">
                    {move || match run_state.get() {
                        RunState::Idle => String::new(),
                        RunState::Connecting => "Connecting to simulator backend...".to_string(),
                        RunState::Running { steps_seen } => format!("Streaming — {} step(s) received", steps_seen),
                        RunState::Done(_) => "Simulation complete.".to_string(),
                        RunState::Errored(ref msg) => format!("Error: {}", msg),
                    }}
                </span>
            </div>

            <SimLog steps=steps />

            <SimSummary run_state=run_state />
        </div>
    }
}

/// The simulation parameters form. All signals are owned by the parent.
#[component]
#[allow(clippy::too_many_arguments)]
fn SimForm(
    cargo_capacity: RwSignal<i32>,
    staterooms: RwSignal<i32>,
    low_berths: RwSignal<i32>,
    jump: RwSignal<i32>,
    fuel_cost_per_parsec: RwSignal<i64>,
    maintenance_per_period: RwSignal<i64>,
    crew_salary_per_period: RwSignal<i64>,
    crew_profit_share: RwSignal<f32>,
    buyer_broker_skill: RwSignal<i16>,
    seller_broker_skill: RwSignal<i16>,
    steward_skill: RwSignal<i16>,
    starting_budget: RwSignal<i64>,
    start_date_text: RwSignal<String>,
    target_date_text: RwSignal<String>,
    illegal_goods: RwSignal<bool>,
    home_name: RwSignal<String>,
    home_sector: RwSignal<String>,
    home_coords: RwSignal<Option<(i32, i32)>>,
    home_uwp: RwSignal<String>,
    home_zone: RwSignal<ZoneClassification>,
) -> impl IntoView {
    view! {
        <div class="sim-form no-print">
            <fieldset class="sim-fieldset">
                <legend>"Ship"</legend>
                <div class="sim-grid">
                    <label>"Cargo capacity (tons)"
                        <input
                            type="number"
                            min="1"
                            prop:value=move || cargo_capacity.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    cargo_capacity.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Staterooms"
                        <input
                            type="number"
                            min="0"
                            prop:value=move || staterooms.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    staterooms.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Low berths"
                        <input
                            type="number"
                            min="0"
                            prop:value=move || low_berths.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    low_berths.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Jump (parsecs)"
                        <input
                            type="number"
                            min="1"
                            prop:value=move || jump.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    jump.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Fuel cost per parsec (Cr)"
                        <input
                            type="number"
                            min="0"
                            prop:value=move || fuel_cost_per_parsec.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                    fuel_cost_per_parsec.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Maintenance / period (Cr)"
                        <input
                            type="number"
                            min="0"
                            prop:value=move || maintenance_per_period.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                    maintenance_per_period.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Crew salary / period (Cr)"
                        <input
                            type="number"
                            min="0"
                            prop:value=move || crew_salary_per_period.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                    crew_salary_per_period.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Crew profit share (0.0 - 1.0)"
                        <input
                            type="text"
                            inputmode="decimal"
                            value=move || format!("{}", crew_profit_share.get())
                            on:change=move |ev| {
                                let s = event_target_value(&ev);
                                if let Ok(v) = s.parse::<f32>()
                                    && (0.0..=1.0).contains(&v)
                                {
                                    crew_profit_share.set(v);
                                }
                            }
                        />
                    </label>
                </div>
            </fieldset>

            <fieldset class="sim-fieldset">
                <legend>"Crew"</legend>
                <div class="sim-grid">
                    <label>"Buyer broker skill"
                        <input
                            type="number"
                            min="-3"
                            max="5"
                            prop:value=move || buyer_broker_skill.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    buyer_broker_skill.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Seller broker skill"
                        <input
                            type="number"
                            min="-3"
                            max="5"
                            prop:value=move || seller_broker_skill.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    seller_broker_skill.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Steward skill"
                        <input
                            type="number"
                            min="-3"
                            max="5"
                            prop:value=move || steward_skill.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    steward_skill.set(v);
                                }
                            }
                        />
                    </label>
                </div>
            </fieldset>

            <fieldset class="sim-fieldset">
                <legend>"Voyage"</legend>
                <div class="sim-grid">
                    <label>"Starting budget (Cr)"
                        <input
                            type="number"
                            min="0"
                            prop:value=move || starting_budget.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                    starting_budget.set(v);
                                }
                            }
                        />
                    </label>
                    <label>"Start date (DDD-YYYY)"
                        <input
                            type="text"
                            class:sim-invalid=move || parse_ddd_yyyy(&start_date_text.read()).is_none()
                            bind:value=start_date_text
                        />
                    </label>
                    <label>"Target completion (DDD-YYYY)"
                        <input
                            type="text"
                            class:sim-invalid=move || parse_ddd_yyyy(&target_date_text.read()).is_none()
                            bind:value=target_date_text
                        />
                    </label>
                    <label>"Illegal goods"
                        <input
                            type="checkbox"
                            prop:checked=move || illegal_goods.get()
                            on:change=move |ev| {
                                illegal_goods.set(event_target_checked(&ev));
                            }
                        />
                    </label>
                </div>
            </fieldset>

            <fieldset class="sim-fieldset">
                <legend>"Home World"</legend>
                <WorldSearch
                    label="Home".to_string()
                    name=home_name
                    uwp=home_uwp
                    coords=home_coords
                    zone=home_zone
                    sector=home_sector
                    show_uwp=false
                />
                <div class="sim-home-summary">
                    {move || {
                        let coords = home_coords.get();
                        let sector = home_sector.get();
                        let uwp = home_uwp.get();
                        let zone = home_zone.get();
                        if coords.is_some() && !sector.is_empty() && uwp.len() == 9 {
                            let (hx, hy) = coords.unwrap();
                            view! {
                                <div class="sim-home-detail">
                                    <div>
                                        <strong>{sector}</strong>
                                        " · hex "
                                        <code>{format!("{:02}{:02}", hx, hy)}</code>
                                        " · UWP "
                                        <code>{uwp}</code>
                                    </div>
                                    <div>
                                        <span class={format!("sim-zone-{}", zone.to_string().to_lowercase())}>
                                            {zone.to_string()}
                                        </span>
                                        " zone"
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="sim-home-detail sim-home-empty">
                                    "Type to search TravellerMap for a world."
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </fieldset>
        </div>
    }
}

/// Per-step description used by the log view.
fn describe_action(action: &Action) -> (String, &'static str) {
    match action {
        Action::Arrive {
            from,
            distance,
            fuel_cost,
        } => (
            format!(
                "Arrived from {} ({} pc, fuel {} Cr)",
                from.name, distance, fuel_cost
            ),
            "sim-action sim-action-arrive",
        ),
        Action::SellGood {
            good,
            qty,
            sell_price,
            paid,
            profit,
        } => (
            format!("Sold {qty}t {good} @ {sell_price}/t (paid {paid}/t) → +{profit} Cr"),
            "sim-action sim-action-sell",
        ),
        Action::HoldGood {
            good,
            qty,
            would_sell_at,
            paid,
            reason,
        } => (
            format!("Held {qty}t {good} (would sell at {would_sell_at}, paid {paid} — {reason})"),
            "sim-action sim-action-hold",
        ),
        Action::BuyGood {
            good,
            qty,
            unit_cost,
            total_cost,
        } => (
            format!("Bought {qty}t {good} @ {unit_cost}/t = {total_cost} Cr"),
            "sim-action sim-action-buy",
        ),
        Action::LoadFreight {
            tons,
            lots,
            revenue_pending,
        } => (
            format!("Loaded {tons}t freight in {lots} lots — pending revenue {revenue_pending} Cr"),
            "sim-action sim-action-freight",
        ),
        Action::BoardPax {
            high,
            medium,
            basic,
            low,
            revenue_pending,
        } => (
            format!(
                "Boarded {high}H/{medium}M/{basic}B/{low}L pax — pending revenue {revenue_pending} Cr"
            ),
            "sim-action sim-action-pax",
        ),
        Action::PayLifeSupport {
            stateroom_cost,
            ls_cost,
            low_cost,
        } => (
            format!(
                "Paid life support: staterooms {stateroom_cost} + LS {ls_cost} + low {low_cost}"
            ),
            "sim-action sim-action-ls",
        ),
        Action::Jump {
            to,
            distance,
            fuel_cost,
        } => (
            format!("Jumped {distance} pc to {} — fuel {fuel_cost} Cr", to.name),
            "sim-action sim-action-jump",
        ),
        Action::PayPeriodic {
            maintenance,
            salary,
            period_index,
        } => (
            format!(
                "Month {}: maintenance {maintenance} Cr + crew salary {salary} Cr",
                period_index + 1
            ),
            "sim-action sim-action-periodic",
        ),
        Action::BudgetWarning { note } => (format!("⚠ {note}"), "sim-action sim-action-warning"),
        Action::NoCandidate { note } => (
            format!("No reachable destination — aborting ({note})"),
            "sim-action sim-action-warning",
        ),
        Action::AbortOverflow { days_past_target } => (
            format!("Aborted: {days_past_target} days past target"),
            "sim-action sim-action-warning",
        ),
    }
}

/// Renders the streaming log of simulation steps.
#[component]
fn SimLog(steps: RwSignal<Vec<SimulationStep>>) -> impl IntoView {
    view! {
        <div class="sim-log">
            <h2>"Simulation Log"</h2>
            <Show
                when=move || !steps.read().is_empty()
                fallback=|| view! { <p class="sim-log-empty">"No steps yet."</p> }
            >
                <div class="sim-step-list">
                    {move || {
                        steps.read()
                            .iter()
                            .enumerate()
                            .map(|(idx, step)| {
                                let date = step.date.format();
                                let location = step.location.name.clone();
                                let budget = step.budget_after;
                                let (text, class) = describe_action(&step.action);
                                view! {
                                    <div class=format!("sim-step {}", class) data-idx=idx>
                                        <span class="sim-step-date">{date}</span>
                                        <span class="sim-step-location">{location}</span>
                                        <span class="sim-step-text">{text}</span>
                                        <span class="sim-step-budget">{format!("{} Cr", budget)}</span>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </div>
            </Show>
        </div>
    }
}

/// Renders the final summary card, including the Save-as-PDF print button.
#[component]
fn SimSummary(run_state: RwSignal<RunState>) -> impl IntoView {
    let print_handler = move |_| {
        if let Some(window) = web_sys::window() {
            let _ = window.print();
        }
    };

    view! {
        {move || match run_state.get() {
            RunState::Done(result) => {
                let r = result;
                view! {
                    <div class="sim-summary">
                        <h2>"Summary"</h2>
                        <div class="sim-summary-grid">
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Jumps"</span>
                                <span class="sim-summary-value">{r.jumps}</span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"End date"</span>
                                <span class="sim-summary-value">{r.end_date.format()}</span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Final budget"</span>
                                <span class="sim-summary-value">{format!("{} Cr", r.final_budget)}</span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Gross profit"</span>
                                <span class="sim-summary-value">{format!("{} Cr", r.gross_profit)}</span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Crew share"</span>
                                <span class="sim-summary-value">{format!("{} Cr", r.crew_share)}</span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Owner profit"</span>
                                <span class="sim-summary-value sim-summary-value-strong">
                                    {format!("{} Cr", r.owner_profit)}
                                </span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Returned home?"</span>
                                <span class="sim-summary-value">
                                    {if r.returned_home { "Yes" } else { "No" }}
                                </span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Went negative?"</span>
                                <span class="sim-summary-value">
                                    {if r.went_negative { "Yes" } else { "No" }}
                                </span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Completed normally?"</span>
                                <span class="sim-summary-value">
                                    {if r.completed_normally { "Yes" } else { "No" }}
                                </span>
                            </div>
                        </div>
                        <button class="blue-button no-print" on:click=print_handler>
                            "Save as PDF"
                        </button>
                    </div>
                }.into_any()
            }
            RunState::Errored(msg) => {
                view! {
                    <div class="sim-summary sim-summary-error">
                        <h2>"Simulation failed"</h2>
                        <p>{msg}</p>
                    </div>
                }.into_any()
            }
            _ => view! { <div></div> }.into_any(),
        }}
    }
}
