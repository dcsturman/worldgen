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

use crate::comms::captains_log::{
    ClientMessage as LogClientMessage, ServerMessage as LogServerMessage,
};
use crate::components::captains_log_prompt::build_prompt;
use crate::components::help_tooltip::HelpTooltip;
use crate::components::tooltip_docs as docs;
use crate::components::traveller_map::WorldSearch;
use crate::simulator::economy::WEAPONS_MAX;
use crate::simulator::map_render::{MapWaypoint, build_plain_link_url, build_route_map_data};
use crate::simulator::protocol::{ClientMessage, ServerMessage};
use crate::simulator::types::{
    Action, Date, SimulationParams, SimulationResult, SimulationStep, WorldRef,
};
use crate::trade::Ship;
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

/// Get the WebSocket URL for the captain's-log endpoint.
///
/// Mirrors `get_ws_url` but for `/ws/captains-log`.
fn get_captains_log_ws_url() -> String {
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
                return "ws://localhost:8081/ws/captains-log".to_string();
            }
        }

        // Docker/Production: connect to same host (nginx proxies /ws/* to backend)
        return format!("{}://{}/ws/captains-log", protocol, location);
    }
    // Fallback
    "ws://localhost:8081/ws/captains-log".to_string()
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

/// State of the captain's-log generation flow. Independent of `RunState` —
/// the simulation can finish and the user may then choose to generate
/// (or regenerate) a captain's log on top of the same result.
#[derive(Debug, Clone)]
enum LogState {
    /// No generation in progress; ready (or never started).
    Idle,
    /// WebSocket open; deltas streaming in.
    Streaming,
    /// Server sent `Done`. Token counts and finish_reason are logged
    /// to the browser console at info level for diagnostics but not
    /// surfaced in the UI — they break the in-character feel.
    Done,
    /// Server (or transport) reported an error. String is the human banner.
    Errored(String),
}

/// One-shot WebSocket client for `/ws/captains-log`. Mirrors `SimClient`
/// in shape but specialised to the captain's-log lifecycle: open, send a
/// single `RunSummary`, stream deltas, terminate on `Done` or `Error`.
#[allow(dead_code)]
struct LogClient {
    ws: WebSocket,
    on_open: Closure<dyn FnMut()>,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_close: Closure<dyn FnMut(CloseEvent)>,
    on_error: Closure<dyn FnMut(ErrorEvent)>,
}

impl LogClient {
    /// Open a WebSocket and send a single `RunSummary` once it opens.
    /// Frames are dispatched into `log_text` / `log_state`.
    fn start(
        prompt: String,
        log_text: RwSignal<String>,
        log_state: RwSignal<LogState>,
    ) -> Result<Self, String> {
        let url = get_captains_log_ws_url();
        info!("Captain's log connecting to {}", url);
        let ws = WebSocket::new(&url).map_err(|e| format!("Failed to open WebSocket: {:?}", e))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let got_terminal: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        // ---- on_open: send the prompt ----
        let ws_for_open = ws.clone();
        let prompt_for_open = prompt;
        let log_state_for_open = log_state;
        let on_open = Closure::<dyn FnMut()>::new(move || {
            let msg = LogClientMessage::RunSummary {
                prompt: prompt_for_open.clone(),
            };
            match serde_json::to_string(&msg) {
                Ok(json) => match ws_for_open.send_with_str(&json) {
                    Ok(_) => {
                        info!("Sent captain's-log RunSummary request");
                    }
                    Err(e) => {
                        error!("Failed to send RunSummary: {:?}", e);
                        log_state_for_open.set(LogState::Errored(format!("Send failed: {:?}", e)));
                        let _ = ws_for_open.close();
                    }
                },
                Err(e) => {
                    error!("Failed to serialize RunSummary: {}", e);
                    log_state_for_open.set(LogState::Errored(format!("Serialize failed: {}", e)));
                    let _ = ws_for_open.close();
                }
            }
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        // ---- on_message: parse ServerMessage and dispatch ----
        let got_terminal_for_msg = got_terminal.clone();
        let ws_for_msg = ws.clone();
        let on_message = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            let Some(text) = e.data().as_string() else {
                error!("Received non-text WebSocket frame on captain's-log; ignoring");
                return;
            };
            match serde_json::from_str::<LogServerMessage>(&text) {
                Ok(LogServerMessage::Delta { text }) => {
                    log_text.update(|s| s.push_str(&text));
                }
                Ok(LogServerMessage::Done {
                    prompt_tokens,
                    output_tokens,
                    finish_reason,
                }) => {
                    *got_terminal_for_msg.borrow_mut() = true;
                    info!(
                        "Captain's log done: {} prompt / {} output tokens (finish_reason={:?})",
                        prompt_tokens, output_tokens, finish_reason
                    );
                    log_state.set(LogState::Done);
                    let _ = ws_for_msg.close();
                }
                Ok(LogServerMessage::Error {
                    code,
                    message,
                    vertex_status,
                    ..
                }) => {
                    *got_terminal_for_msg.borrow_mut() = true;
                    error!("Captain's log error: {} {}", code, message);
                    let suffix = vertex_status
                        .map(|s| format!(" (HTTP {})", s))
                        .unwrap_or_default();
                    log_state.set(LogState::Errored(format!(
                        "{}: {}{}",
                        code, message, suffix
                    )));
                    log_text.set(String::new());
                    let _ = ws_for_msg.close();
                }
                Err(err) => {
                    error!(
                        "Failed to parse captain's-log ServerMessage: {} (text: {})",
                        err, text
                    );
                }
            }
        });
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        // ---- on_close: surface premature close as an error ----
        let got_terminal_for_close = got_terminal.clone();
        let on_close = Closure::<dyn FnMut(_)>::new(move |e: CloseEvent| {
            info!(
                "Captain's-log WebSocket closed: code={}, reason={}",
                e.code(),
                e.reason()
            );
            if !*got_terminal_for_close.borrow() {
                let reason = if e.reason().is_empty() {
                    format!("Connection closed (code {})", e.code())
                } else {
                    format!("Connection closed: {} (code {})", e.reason(), e.code())
                };
                log_state.update(|s| {
                    if !matches!(s, LogState::Done | LogState::Errored(_)) {
                        *s = LogState::Errored(reason.clone());
                    }
                });
            }
        });
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        let on_error = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            error!("Captain's-log WebSocket error: {:?}", e.message());
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
    let ship_name = RwSignal::new(String::new());
    let cargo_capacity = RwSignal::new(80i32);
    let crew_staterooms = RwSignal::new(4i32);
    let passenger_staterooms = RwSignal::new(4i32);
    let low_berths = RwSignal::new(4i32);
    let jump_rating = RwSignal::new(2i16);
    let fuel_cost_per_parsec = RwSignal::new(500i64);
    let maintenance_per_period = RwSignal::new(5_000i64);
    let salary_per_period = RwSignal::new(12_000i64);
    let mortgage_per_period = RwSignal::new(0i64);
    let crew_profit_share = RwSignal::new(0.10f32);

    // Crew
    let broker_skill = RwSignal::new(1i16);
    let steward_skill = RwSignal::new(1i16);
    let leadership_skill = RwSignal::new(1i16);
    let weapons = RwSignal::new(2i16);
    let crew_size = RwSignal::new(4i32);

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
    // Cache the params used for the most recent run so the captain's-log
    // panel can build its prompt from the same inputs even if the form
    // has been edited since.
    let last_params = RwSignal::new(None::<SimulationParams>);

    // Hold the live client so its closures stay alive across renders.
    let client_holder: Rc<RefCell<Option<SimClient>>> = Rc::new(RefCell::new(None));

    // ---- Validation ----
    let is_valid = Memo::new(move |_| {
        cargo_capacity.get() > 0
            && crew_staterooms.get() >= 0
            && passenger_staterooms.get() >= 0
            && low_berths.get() >= 0
            && crew_size.get() >= 0
            && jump_rating.get() > 0
            && fuel_cost_per_parsec.get() >= 0
            && maintenance_per_period.get() >= 0
            && salary_per_period.get() >= 0
            && mortgage_per_period.get() >= 0
            && (0.0..=1.0).contains(&crew_profit_share.get())
            && (-3..=5).contains(&broker_skill.get())
            && (-3..=5).contains(&steward_skill.get())
            && (0..=5).contains(&leadership_skill.get())
            && (0..=24).contains(&weapons.get())
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
            ship: Ship {
                // Ship name is optional — used by the captain's-log
                // narrative; the simulator itself runs against
                // `home_world` for identity, not `ship.name`.
                name: ship_name.get_untracked().trim().to_string(),
                broker_skill: broker_skill.get_untracked(),
                steward_skill: steward_skill.get_untracked(),
                leadership_skill: leadership_skill.get_untracked(),
                weapons: weapons.get_untracked(),
                cargo_capacity: cargo_capacity.get_untracked(),
                passenger_staterooms: passenger_staterooms.get_untracked(),
                low_berths: low_berths.get_untracked(),
                crew_staterooms: crew_staterooms.get_untracked(),
                crew_size: crew_size.get_untracked(),
                jump_rating: jump_rating.get_untracked(),
                mortgage_per_period: mortgage_per_period.get_untracked(),
                maintenance_per_period: maintenance_per_period.get_untracked(),
                salary_per_period: salary_per_period.get_untracked(),
            },
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

        last_params.set(Some(params.clone()));

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
                ship_name=ship_name
                cargo_capacity=cargo_capacity
                crew_staterooms=crew_staterooms
                passenger_staterooms=passenger_staterooms
                low_berths=low_berths
                jump_rating=jump_rating
                fuel_cost_per_parsec=fuel_cost_per_parsec
                maintenance_per_period=maintenance_per_period
                salary_per_period=salary_per_period
                mortgage_per_period=mortgage_per_period
                crew_profit_share=crew_profit_share
                broker_skill=broker_skill
                steward_skill=steward_skill
                leadership_skill=leadership_skill
                weapons=weapons
                crew_size=crew_size
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

            <div class="sim-summary-pair">
                <SimSummary run_state=run_state last_params=last_params />
                // Only mount the captain's-log panel once a simulation
                // has actually completed — empty state has nothing to
                // narrate. Unmounting on non-Done also cleanly drops
                // any in-flight WS client between runs.
                {move || matches!(run_state.get(), RunState::Done(_)).then(|| view! {
                    <CaptainsLog
                        run_state=run_state
                        steps=steps
                        last_params=last_params
                    />
                })}
            </div>

            <SimLog steps=steps home_name=home_name />

            <RouteMap run_state=run_state steps=steps />
        </div>
    }
}

/// The simulation parameters form. All signals are owned by the parent.
#[component]
#[allow(clippy::too_many_arguments)]
fn SimForm(
    ship_name: RwSignal<String>,
    cargo_capacity: RwSignal<i32>,
    crew_staterooms: RwSignal<i32>,
    passenger_staterooms: RwSignal<i32>,
    low_berths: RwSignal<i32>,
    jump_rating: RwSignal<i16>,
    fuel_cost_per_parsec: RwSignal<i64>,
    maintenance_per_period: RwSignal<i64>,
    salary_per_period: RwSignal<i64>,
    mortgage_per_period: RwSignal<i64>,
    crew_profit_share: RwSignal<f32>,
    broker_skill: RwSignal<i16>,
    steward_skill: RwSignal<i16>,
    leadership_skill: RwSignal<i16>,
    weapons: RwSignal<i16>,
    crew_size: RwSignal<i32>,
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
                    <label>
                        <span class="sim-label-row">
                            "Ship name"
                        </span>
                        <input
                            type="text"
                            placeholder="(optional — invent one)"
                            bind:value=ship_name
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Cargo capacity (tons)"
                            <HelpTooltip text=docs::CARGO_CAPACITY />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Crew staterooms"
                            <HelpTooltip text=docs::CREW_STATEROOMS />
                        </span>
                        <input
                            type="number"
                            min="0"
                            prop:value=move || crew_staterooms.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    crew_staterooms.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Passenger staterooms"
                            <HelpTooltip text=docs::PASSENGER_STATEROOMS />
                        </span>
                        <input
                            type="number"
                            min="0"
                            prop:value=move || passenger_staterooms.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    passenger_staterooms.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Low berths"
                            <HelpTooltip text=docs::LOW_BERTHS />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Jump (parsecs)"
                            <HelpTooltip text=docs::JUMP_RATING />
                        </span>
                        <input
                            type="number"
                            min="1"
                            prop:value=move || jump_rating.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    jump_rating.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Fuel cost per parsec (Cr)"
                            <HelpTooltip text=docs::FUEL_COST_PER_PARSEC />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Maintenance / period (Cr)"
                            <HelpTooltip text=docs::MAINTENANCE_PER_PERIOD />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Crew salary / period (Cr)"
                            <HelpTooltip text=docs::SALARY_PER_PERIOD />
                        </span>
                        <input
                            type="number"
                            min="0"
                            prop:value=move || salary_per_period.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                    salary_per_period.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Mortgage / period (Cr)"
                            <HelpTooltip text=docs::MORTGAGE_PER_PERIOD />
                        </span>
                        <input
                            type="number"
                            min="0"
                            prop:value=move || mortgage_per_period.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i64>() {
                                    mortgage_per_period.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Crew profit share"
                            <HelpTooltip text=docs::CREW_PROFIT_SHARE />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Ship Broker skill"
                            <HelpTooltip text=docs::BROKER_SKILL />
                        </span>
                        <input
                            type="number"
                            min="-3"
                            max="5"
                            prop:value=move || broker_skill.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    broker_skill.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Steward skill"
                            <HelpTooltip text=docs::STEWARD_SKILL />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Leadership"
                            <HelpTooltip text=docs::LEADERSHIP />
                        </span>
                        <input
                            type="number"
                            min="0"
                            max="5"
                            prop:value=move || leadership_skill.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    leadership_skill.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Weapons"
                            <HelpTooltip text=docs::WEAPONS />
                        </span>
                        <input
                            type="number"
                            min="0"
                            max=WEAPONS_MAX
                            prop:value=move || weapons.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i16>() {
                                    weapons.set(v);
                                }
                            }
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Crew size"
                            <HelpTooltip text=docs::CREW_SIZE />
                        </span>
                        <input
                            type="number"
                            min="0"
                            prop:value=move || crew_size.get()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    crew_size.set(v);
                                }
                            }
                        />
                    </label>
                </div>
            </fieldset>

            <fieldset class="sim-fieldset">
                <legend>"Voyage"</legend>
                <div class="sim-grid">
                    <label>
                        <span class="sim-label-row">
                            "Starting budget (Cr)"
                            <HelpTooltip text=docs::STARTING_BUDGET />
                        </span>
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
                    <label>
                        <span class="sim-label-row">
                            "Start date (DDD-YYYY)"
                            <HelpTooltip text=docs::START_DATE />
                        </span>
                        <input
                            type="text"
                            class:sim-invalid=move || parse_ddd_yyyy(&start_date_text.read()).is_none()
                            bind:value=start_date_text
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Target completion"
                            <HelpTooltip text=docs::TARGET_COMPLETION />
                        </span>
                        <input
                            type="text"
                            class:sim-invalid=move || parse_ddd_yyyy(&target_date_text.read()).is_none()
                            bind:value=target_date_text
                        />
                    </label>
                    <label>
                        <span class="sim-label-row">
                            "Illegal goods"
                            <HelpTooltip text=docs::ILLEGAL_GOODS />
                        </span>
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
                        if let Some((hx, hy)) = coords && !sector.is_empty() && uwp.len() == 9 {
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
///
/// Returns `None` for actions that should not appear in the log (e.g.
/// `IncidentAvoided`). Callers must filter those out before rendering so
/// the row count and indices reflect actual entries.
fn describe_action(action: &Action, home_port: &str) -> Option<(String, &'static str)> {
    Some(match action {
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
            crew_cost,
        } => (
            format!(
                "Paid life support: staterooms {stateroom_cost} + LS {ls_cost} + low {low_cost} + crew {crew_cost}"
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
            mortgage,
            period_index,
        } => (
            format!(
                "Month {}: maintenance {maintenance} Cr + crew salary {salary} Cr + mortgage {mortgage} Cr",
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

        // Skip avoided-incident rows entirely; they're just analytics noise.
        Action::IncidentAvoided { .. } => return None,

        Action::IncidentPiracy {
            cargo_lost_tons,
            buy_cost_sunk,
            credits_lost,
            weeks_lost,
            weapons,
            avoidance_total,
            table_total,
            ..
        } => (
            format!(
                "Pirates! −{cargo_lost_tons}t cargo (−{buy_cost_sunk} Cr sunk), \
                 −{credits_lost} Cr in repairs, +{weeks_lost} weeks delay \
                 (weapons {weapons}, avoid={avoidance_total}, table={table_total})"
            ),
            "sim-action sim-action-incident sim-action-piracy",
        ),
        Action::IncidentTradeScam {
            credits_lost,
            weeks_lost,
            broker,
            avoidance_total,
            table_total,
            ..
        } => (
            format!(
                "Trade scam: −{credits_lost} Cr, +{weeks_lost} weeks \
                 (broker {broker}, avoid={avoidance_total}, table={table_total})"
            ),
            "sim-action sim-action-incident sim-action-scam",
        ),
        Action::IncidentCrewLoss {
            weeks_lost,
            leadership,
            avoidance_total,
            table_total,
            ..
        } => (
            format!(
                "Crew layover: +{weeks_lost} weeks \
                 (leadership {leadership}, avoid={avoidance_total}, table={table_total})"
            ),
            "sim-action sim-action-incident sim-action-crew",
        ),
        Action::IncidentAccident {
            repair_cost,
            avoidance_total,
            table_total,
            ..
        } => (
            format!(
                "Accident: −{repair_cost} Cr in repairs \
                 (avoid={avoidance_total}, table={table_total})"
            ),
            "sim-action sim-action-incident sim-action-accident",
        ),
        Action::IncidentGovernment {
            fine_credits,
            weeks_lost,
            avoidance_total,
            table_total,
            ..
        } => (
            format!(
                "Government complication: −{fine_credits} Cr fine, +{weeks_lost} weeks \
                 (avoid={avoidance_total}, table={table_total})"
            ),
            "sim-action sim-action-incident sim-action-government",
        ),
        Action::Marooned {
            budget,
            total_parsecs_jumped,
            rescue_eta_days,
            rescue_arrives_on,
        } => (
            format!(
                "MAROONED — budget {budget} Cr; mayday arrives at {home_port} on {} ({rescue_eta_days} days, {total_parsecs_jumped} pc travelled)",
                rescue_arrives_on.format()
            ),
            "sim-action sim-action-marooned",
        ),
    })
}

/// Renders the streaming log of simulation steps.
#[component]
fn SimLog(steps: RwSignal<Vec<SimulationStep>>, home_name: RwSignal<String>) -> impl IntoView {
    view! {
        <div class="sim-log">
            <h2>"Simulation Log"</h2>
            <Show
                when=move || !steps.read().is_empty()
                fallback=|| view! { <p class="sim-log-empty">"No steps yet."</p> }
            >
                <div class="sim-step-list">
                    {move || {
                        let home_port = home_name.read().clone();
                        steps.read()
                            .iter()
                            .filter_map(|step| {
                                describe_action(&step.action, &home_port).map(|(text, class)| (step, text, class))
                            })
                            .enumerate()
                            .map(|(idx, (step, text, class))| {
                                let date = step.date.format();
                                let location = step.location.name.clone();
                                let budget = step.budget_after;
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

/// Walk the step list and pull out the visit order — one entry per
/// distinct location, in the order they were first visited *within a
/// run of consecutive steps*. Returns `(map_waypoints, display_names)`
/// so we can render both the iframe overlay and the textual hop list
/// from a single pass.
fn extract_route(steps: &[SimulationStep]) -> (Vec<MapWaypoint>, Vec<String>) {
    let mut waypoints: Vec<MapWaypoint> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut last: Option<(String, i32, i32)> = None;
    for step in steps {
        let loc = &step.location;
        let key = (loc.sector.clone(), loc.hex_x, loc.hex_y);
        if Some(&key) != last.as_ref() {
            waypoints.push(MapWaypoint {
                sector: loc.sector.clone(),
                hex_x: loc.hex_x,
                hex_y: loc.hex_y,
                color: if waypoints.is_empty() {
                    "green"
                } else {
                    "blue"
                },
            });
            names.push(loc.name.clone());
            last = Some(key);
        }
    }
    (waypoints, names)
}

/// Renders a static TravellerMap image (Tile API) of the area the
/// route covered, with an SVG overlay drawing the visited worlds as
/// circles and the path between them as a polyline. Hidden until the
/// run is `Done`. The "Open on TravellerMap" link below the map goes
/// to the interactive site centred on the home world.
#[component]
fn RouteMap(run_state: RwSignal<RunState>, steps: RwSignal<Vec<SimulationStep>>) -> impl IntoView {
    view! {
        {move || {
            if !matches!(run_state.get(), RunState::Done(_)) {
                return view! { <div></div> }.into_any();
            }
            let all_steps = steps.read();
            if all_steps.is_empty() {
                return view! { <div></div> }.into_any();
            }
            let (waypoints, names) = extract_route(&all_steps);
            if waypoints.is_empty() {
                return view! { <div></div> }.into_any();
            }
            let map_data = build_route_map_data(&waypoints);
            let link_url = build_plain_link_url(&waypoints[0]);
            let path_text = names.join(" → ");
            view! {
                <div class="sim-route-map">
                    <h2>"Route Map"</h2>
                    {match map_data {
                        Some(data) => {
                            let view_box = format!("0 0 {} {}", data.width, data.height);
                            // Polyline through every waypoint that has a known pixel
                            // position. Drawn first so the circles render on top.
                            let polyline_points = data.waypoints_px.iter()
                                .filter_map(|p| p.map(|(x, y)| format!("{:.1},{:.1}", x, y)))
                                .collect::<Vec<_>>()
                                .join(" ");
                            // Circles per waypoint in input order — colour comes from
                            // the MapWaypoint (home is green, rest blue).
                            let circles = waypoints.iter().zip(data.waypoints_px.iter())
                                .filter_map(|(wp, p)| p.map(|(x, y)| (wp.color, x, y)))
                                .map(|(color, x, y)| view! {
                                    <circle
                                        cx=format!("{:.1}", x)
                                        cy=format!("{:.1}", y)
                                        r="7"
                                        fill=color
                                        stroke="black"
                                        stroke-width="1.5"
                                    />
                                })
                                .collect::<Vec<_>>();
                            view! {
                                // Layout-critical positioning is inlined so the overlay works
                                // even if the page's CSS bundle is stale (which trunk has been
                                // known to miss between hot-reloads).
                                <div
                                    class="sim-route-map-frame"
                                    style="position: relative; line-height: 0;"
                                >
                                    <img
                                        src=data.image_url
                                        alt="Route map"
                                        style="display: block; width: 100%; height: auto;"
                                    />
                                    <svg
                                        class="sim-route-overlay"
                                        viewBox=view_box
                                        preserveAspectRatio="none"
                                        style="position: absolute; top: 0; left: 0; width: 100%; height: 100%; pointer-events: none;"
                                    >
                                        <polyline
                                            points=polyline_points
                                            fill="none"
                                            stroke="#FFB000"
                                            stroke-width="3"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            opacity="0.9"
                                        />
                                        {circles}
                                    </svg>
                                </div>
                            }.into_any()
                        },
                        None => view! {
                            <div class="sim-route-map-empty">
                                "Map preview unavailable for this sector — see link below."
                            </div>
                        }.into_any(),
                    }}
                    <div class="sim-route-path">{path_text}</div>
                    <a class="sim-route-link no-print" href=link_url target="_blank" rel="noopener">
                        "Open on TravellerMap"
                    </a>
                </div>
            }.into_any()
        }}
    }
}

/// Renders the final summary card, including the Save-as-PDF print button.
#[component]
fn SimSummary(
    run_state: RwSignal<RunState>,
    last_params: RwSignal<Option<SimulationParams>>,
) -> impl IntoView {
    let print_handler = move |_| {
        if let Some(window) = web_sys::window() {
            let _ = window.print();
        }
    };

    view! {
        {move || match run_state.get() {
            RunState::Done(result) => {
                let r = result;
                let marooned_panel = if r.marooned {
                    let loc = r.marooned_at.as_ref().map(|w| w.name.clone()).unwrap_or_default();
                    let on_date = r.marooned_on.map(|d| d.format()).unwrap_or_default();
                    let signal_date = r.rescue_arrives_on.map(|d| d.format()).unwrap_or_default();
                    let home_name = last_params
                        .get()
                        .map(|p| p.home_world.name.clone())
                        .unwrap_or_else(|| "home".to_string());
                    view! {
                        <div class="sim-summary sim-summary-marooned">
                            <h2>"⚠ Marooned"</h2>
                            <p>"Marooned at "<strong>{loc}</strong>" on "<strong>{on_date}</strong>"."</p>
                            <p>"Distress signal received at "<strong>{home_name}</strong>" on "<strong>{signal_date}</strong>"."</p>
                        </div>
                    }.into_any()
                } else {
                    view! {<div></div>}.into_any()
                };
                let marooned = r.marooned;
                view! {
                    {marooned_panel}
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
                            {(!marooned).then(|| view! {
                                <div class="sim-summary-row">
                                    <span class="sim-summary-label">"Returned home?"</span>
                                    <span class="sim-summary-value">
                                        {if r.returned_home { "Yes" } else { "No" }}
                                    </span>
                                </div>
                            })}
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Went negative?"</span>
                                <span class="sim-summary-value">
                                    {if r.went_negative { "Yes" } else { "No" }}
                                </span>
                            </div>
                            <div class="sim-summary-row">
                                <span class="sim-summary-label">"Marooned?"</span>
                                <span class="sim-summary-value">
                                    {if marooned { "Yes" } else { "No" }}
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

/// AI-generated captain's-log narrative panel. Sits next to the
/// `SimSummary` panel and is a no-op until the simulation completes.
///
/// One-shot WebSocket flow per click: open `/ws/captains-log`, send the
/// assembled prompt, append `Delta` frames into `log_text`, terminate
/// on `Done` (records token usage) or `Error` (clears text and shows a
/// banner). Each subsequent click clears state and starts fresh.
#[component]
fn CaptainsLog(
    run_state: RwSignal<RunState>,
    steps: RwSignal<Vec<SimulationStep>>,
    last_params: RwSignal<Option<SimulationParams>>,
) -> impl IntoView {
    let log_text = RwSignal::new(String::new());
    let log_state = RwSignal::new(LogState::Idle);

    // Hold the live client so its closures stay alive across renders.
    let client_holder: Rc<RefCell<Option<LogClient>>> = Rc::new(RefCell::new(None));

    // Whenever the simulator kicks off a new run (Connecting / Running),
    // clear the captain's-log panel so the old voyage's narrative doesn't
    // linger next to fresh simulation data. Also drops any in-flight
    // generation client to abort a stale stream.
    {
        let client_holder = client_holder.clone();
        Effect::new(move |_| match run_state.get() {
            RunState::Connecting | RunState::Running { .. } => {
                log_text.set(String::new());
                log_state.set(LogState::Idle);
                *client_holder.borrow_mut() = None;
            }
            _ => {}
        });
    }

    let client_holder_for_click = client_holder.clone();
    let on_generate = move |_| {
        // Need a completed simulation and the params we ran with.
        let result = match run_state.get_untracked() {
            RunState::Done(r) => r,
            _ => return,
        };
        let Some(params) = last_params.get_untracked() else {
            log_state.set(LogState::Errored(
                "Internal error: simulation parameters missing.".to_string(),
            ));
            return;
        };
        // Reset for a fresh attempt (also clears any prior error banner).
        log_text.set(String::new());
        log_state.set(LogState::Streaming);

        let prompt = {
            let steps_ref = steps.read();
            build_prompt(&params.ship.name, &params, &steps_ref, &result)
        };

        match LogClient::start(prompt, log_text, log_state) {
            Ok(client) => {
                *client_holder_for_click.borrow_mut() = Some(client);
            }
            Err(e) => {
                error!("Failed to start captain's-log client: {}", e);
                log_state.set(LogState::Errored(e));
            }
        }
    };

    view! {
        <div class="sim-summary captains-log">
            <h2>"Captain's Log"</h2>
            <button
                class="blue-button no-print"
                prop:disabled=move || {
                    !matches!(run_state.get(), RunState::Done(_))
                        || matches!(log_state.get(), LogState::Streaming)
                }
                on:click=on_generate
            >
                {move || match log_state.get() {
                    LogState::Streaming => "Generating...".to_string(),
                    LogState::Done => "Regenerate Captain's Log".to_string(),
                    _ => "Generate Captain's Log".to_string(),
                }}
            </button>
            {move || match log_state.get() {
                LogState::Errored(msg) => view! {
                    <p class="captains-log-error">
                        {format!("Captain's log unavailable: {}", msg)}
                    </p>
                }.into_any(),
                _ => view! { <span></span> }.into_any(),
            }}
            // Token counts and finish_reason intentionally not rendered —
            // they break the in-character feel of the log. The data still
            // flows to the backend log via `vertex: stream complete — …`
            // for diagnostics.
            <pre class="captains-log-text">{move || log_text.get()}</pre>
        </div>
    }
}
