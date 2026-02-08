//! # Worldgen Main Application Entry Point
//!
//! This is the main entry point for the Worldgen web application.
//! It sets up routing based on URL paths and renders the appropriate components.

use web_sys::js_sys::{Function, Object, Reflect};

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags};
use worldgen::components::app::App;

#[cfg(not(feature = "ssr"))]
use worldgen::logging;

use log::{debug, info, warn};

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
    pub use tracing_subscriber::{fmt, prelude::*, EnvFilter};
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

#[cfg(feature = "ssr")]
#[component]
fn AppShell() -> impl IntoView {
    let thread_id = std::thread::current().id();
    debug!(
        "Rendering Thread {:?}: Entering Shell component.",
        thread_id
    );

    if use_context::<leptos_meta::MetaContext>().is_none() {
        warn!(
            "Thread {:?}: Recovering missing MetaContext in Shell",
            thread_id
        );
        provide_meta_context();
    }

    let app_state =
        use_context::<AppState>().expect("AppState context should be available in Shell");
    let options = app_state.leptos_options.clone();

    // Provide WsSignals context - required by leptos_ws
    provide_context(app_state.ws_signals.clone());

    let site_root = options.site_root.clone();
    let pkg_path = options.site_pkg_dir.clone();
    let css_file = format!("{pkg_path}/{}.css", options.output_name);
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="theme-color" content="#000000" />
                <title>"Worldgen"</title>

                // Google Analytics (Standard Script Tags)
                <script async src="https://www.googletagmanager.com/gtag/js?id=G-L26P5SCYR2" />
                <script>
                    "window.dataLayer = window.dataLayer || [];
                    function gtag(){dataLayer.push(arguments);}
                    gtag('js', new Date());
                    gtag('config', 'G-L26P5SCYR2');"
                </script>

                // CSS Files (Note the leading slashes)
                <link rel="stylesheet" href="/css/bootstrap.min.css" />
                <link rel="stylesheet" href=css_file />
                <link rel="stylesheet" href="/css/modal.css" />

                // The Leptos "Magic" tags
                <HydrationScripts options=options.clone() />
                {move || view! { <MetaTags /> }}
            </head>
            <body>
                // In SSR, we don't use <div id="root">, we just render the App
                <App />
            </body>
        </html>
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
            // Provide WsSignals context - MUST be provided before any other context
            // This is required by leptos_ws WebSocket server function
            leptos::prelude::provide_context(state.ws_signals.clone());
            leptos::prelude::provide_context(state.leptos_options.clone());
            leptos::prelude::provide_context(state.firestore_db.clone());
            leptos::prelude::provide_context(state.session_cache.clone());
        },
        request,
    )
    .await
}

#[cfg(feature = "ssr")]
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};

#[cfg(feature = "ssr")]
use tower::util::ServiceExt;
#[cfg(feature = "ssr")]
use tower_http::services::ServeDir;

#[cfg(feature = "ssr")]
async fn get_static_file(
    uri: &Uri,
    root: &str,
    headers: &HeaderMap<HeaderValue>,
) -> Result<Response<Body>, (StatusCode, String)> {
    use axum::http::header::ACCEPT_ENCODING;

    let req = Request::builder().uri(uri);

    let req = match headers.get(ACCEPT_ENCODING) {
        Some(value) => req.header(ACCEPT_ENCODING, value),
        None => req,
    };

    let req = req.body(Body::empty()).unwrap();
    // `ServeDir` implements `tower::Service` so we can call it with `tower::ServiceExt::oneshot`
    // This path is relative to the cargo root
    match ServeDir::new(root)
        .precompressed_gzip()
        .precompressed_br()
        .oneshot(req)
        .await
    {
        Ok(res) => Ok(res.into_response()),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {err}"),
        )),
    }
}

#[cfg(feature = "ssr")]
pub async fn leptos_fallback(State(options): State<LeptosOptions>, req: Request<Body>) -> Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    match get_static_file(req.uri(), &options.site_root, req.headers()).await {
        Ok(res) => res.into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Route not found").into_response(),
    }
}

/// Start for the Axum server (backend - server side)
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use ssr_imports::*;

    // Tracing support
    let filter = EnvFilter::new("debug")
        .add_directive("leptos_meta=debug".parse().unwrap())
        .add_directive("leptos_axum=debug".parse().unwrap())
        .add_directive("leptos=debug".parse().unwrap())
        .add_directive("leptos_router=debug".parse().unwrap())
        .add_directive("tachys=debug".parse().unwrap());

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    debug!("TEST THAT DEBUG IS WORKING");
    leptos::logging::debug_warn!("IF YOU SEE THIS, LEPTOS CORE HAS TRACING");

    // 1. Check the compiler flag for meta specifically
    if cfg!(feature = "leptos_meta") {
        info!("CANARY: leptos_meta dependency is ACTIVE");
    }

    // 2. Try to trigger a manual trace from within the meta namespace
    // In Leptos 0.8, this should emit a trace if the feature is on.
    let _ = leptos_meta::MetaContext::default();
    info!("CANARY: Created a manual MetaContext without panicking.");

    // 3. Force a manual debug log from the leptos_meta crate's logic
    // We can't call their internal private macros, but we can see if
    // provide_meta_context runs without the "no global spawner" or "no head" error here.
    leptos_meta::provide_meta_context();
    info!("CANARY: provide_meta_context() executed in main successfully.");

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

    info!("========================================");
    info!("WORLDGEN SERVER STARTING - NEW BUILD");
    info!("========================================");
    info!("Generate the route list");
    let routes = generate_route_list(App);
    info!("Ready to set up router");
    let app = Router::new()
        .route("/api/{*fn_name}", post(server_fn_handler))
        .route("/api/{*fn_name}", get(server_fn_handler))
        .leptos_routes_with_context(&app_state, routes, {
            let app_state = app_state.clone();
            move || {
                provide_meta_context();
                provide_context(app_state.clone());
                // Provide WsSignals - needed for WebSocket server function
                provide_context(app_state.ws_signals.clone());
            }
        }, {
            let _app_state = app_state.clone();
            move || view! { <AppShell /> }
        })
        .fallback(leptos_fallback)
        .layer(cors)
        .with_state(app_state);
    info!("Router set up.");
    info!("Starting server on http://{}", addr);
    info!("Serving with site root : {}", leptos_options.site_root);
    info!("API endpoints available at /api/*");
    info!("WebSocket endpoint available at /api/leptos_ws_websocket");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
