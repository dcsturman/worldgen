//! # Worldgen Main Application Entry Point
//!
//! This is the main entry point for the Worldgen web application.
//! It sets up routing based on URL paths and renders the appropriate components.

use web_sys::js_sys::{Function, Object, Reflect};

use leptos::prelude::*;
use worldgen::components::selector::Selector;
use worldgen::components::system_generator::World;
use worldgen::components::trade_computer::Trade;

#[cfg(not(feature = "ssr"))]
use worldgen::logging;

#[cfg(feature = "ssr")]
use log::info;

#[cfg(feature = "ssr")]
mod ssr_imports {
    pub use axum::{
        routing::{get, post},
        Router,
    };
    pub use firestore::FirestoreDb;
    pub use leptos::config::get_configuration;
    pub use leptos::prelude::*;
    pub use leptos_axum::*;
    pub use leptos_ws::WsSignals;
    pub use std::collections::HashMap;
    pub use std::net::SocketAddr;
    pub use std::sync::{Arc, RwLock};
    pub use tower_http::cors::{Any, CorsLayer};
    pub use worldgen::backend::SessionCache;
}

const GA_MEASUREMENT_ID: &str = "G-L26P5SCYR2";
/// Track page view for analytics
fn track_page_view(_path: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(gtag) = Reflect::get(&window, &"gtag".into()) {
            let _ = Function::from(gtag).call3(
                &window,
                &"config".into(),
                &GA_MEASUREMENT_ID.into(),
                &Object::new(),
            );
        }
    }
}

/// Main application component that handles routing based on URL path
#[component]
fn App() -> impl IntoView {
    let path = web_sys::window()
        .unwrap()
        .location()
        .pathname()
        .unwrap_or_default();

    // Track the page view
    track_page_view(&path);

    if path.contains("world") {
        view! { <World /> }.into_any()
    } else if path.contains("trade") {
        view! { <Trade /> }.into_any()
    } else {
        view! { <Selector /> }.into_any()
    }
}

/// Application state for Axum, containing all server-side shared state
#[cfg(feature = "ssr")]
#[derive(Clone, axum::extract::FromRef)]
pub struct AppState {
    pub leptos_options: leptos::config::LeptosOptions,
    pub ws_signals: leptos_ws::WsSignals,
    /// Firestore database client - already handles internal sharing via Arc
    pub firestore_db: firestore::FirestoreDb,
    /// Session cache - maps session IDs to their cached TradeState
    /// Sessions are lazily loaded when clients call use_session()
    pub session_cache: worldgen::backend::SessionCache,
}

/// Handler for server functions with all app context
#[cfg(feature = "ssr")]
async fn server_fn_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    _path: axum::extract::Path<String>,
    request: axum::extract::Request,
) -> impl axum::response::IntoResponse {
    leptos_axum::handle_server_fns_with_context(
        move || {
            leptos::prelude::provide_context(state.leptos_options.clone());
            leptos::prelude::provide_context(state.ws_signals.clone());
            leptos::prelude::provide_context(state.firestore_db.clone());
            leptos::prelude::provide_context(state.session_cache.clone());
        },
        request,
    )
    .await
}

/// Start for the Axum server (backend - server side)
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use ssr_imports::*;

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get port from environment (Cloud Run sets PORT)
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a valid number");

    // CORS configuration for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let conf = get_configuration(None).unwrap();
    let mut leptos_options = conf.leptos_options;

    leptos_options.site_addr = addr;

    // Create WebSocket signals for server-to-client communication
    let ws_signals = WsSignals::new();
    info!("WebSocket signals initialized for server-to-client communication");

    // Initialize Firestore database
    let project_id = std::env::var("GCP_PROJECT")
        .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
        .expect("GCP_PROJECT or GOOGLE_CLOUD_PROJECT environment variable must be set");

    let database_id = std::env::var("FIRESTORE_DATABASE_ID")
        .expect("FIRESTORE_DATABASE_ID environment variable must be set");

    info!(
        "Initializing Firestore client for project: {} database: {}",
        &project_id, &database_id
    );

    let options = firestore::FirestoreDbOptions::new(project_id).with_database_id(database_id);

    let firestore_db = FirestoreDb::with_options(options)
        .await
        .expect("Failed to initialize Firestore client");
    info!("Firestore client initialized successfully");

    // Create empty session cache - sessions are loaded lazily when clients call use_session()
    let session_cache: SessionCache = Arc::new(RwLock::new(HashMap::new()));
    info!("Session cache initialized (empty - sessions loaded on demand)");

    // Create app state with all components
    let app_state = AppState {
        leptos_options: leptos_options.clone(),
        ws_signals: ws_signals.clone(),
        firestore_db,
        session_cache,
    };

    let routes = generate_route_list(App);
    let app = Router::new()
        .route("/api/{*fn_name}", post(server_fn_handler))
        .route("/api/{*fn_name}", get(server_fn_handler))
        .leptos_routes(&app_state, routes, {
            let app_state = app_state.clone();
            move || {
                provide_context(app_state.ws_signals.clone());
            }
        })
        .layer(cors)
        .with_state(app_state);

    info!("Starting server on http://{}", addr);
    info!("Serving with site root : {}", leptos_options.site_root);
    info!("API endpoints available at /api/*");
    info!("WebSocket endpoint available at /api/leptos_ws_websocket");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

/// Application entry point for the client-side
///
/// Sets up panic hooks, initializes logging from URL parameters,
/// and mounts the main App component to the document body.  App
/// simply provides a selector for the two main applications based on the
/// URL path.  See index.html for the entry point and routing to the appropriate
/// URLs.  This means if you go to the root URL, you will see the selector.  If you
/// go to the /world URL, you will see the world generator.  If you go to
/// the /trade URL, you will see the trade computer.
#[cfg(not(feature = "ssr"))]
fn main() {
    console_error_panic_hook::set_once();
    logging::init_from_url();
    mount_to_body(App);
}
