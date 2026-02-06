//! # API Client Module
//!
//! This module provides client-side functions for communicating with the server API.
//! It handles fetching and saving trade state to/from the Firestore-backed server.
//!
//! ## Usage
//!
//! The API client is designed to work with Leptos signals and resources:
//!
//! ```rust
//! // Load state on component mount
//! let state_resource = create_resource(|| (), |_| async { get_state().await });
//!
//! // Save state when it changes
//! spawn_local(async move {
//!     save_state(&new_state).await;
//! });
//! ```
//!
//! ## Error Handling
//!
//! All API functions return `Result<T, ApiError>` to allow proper error handling.
//! Errors are logged to the console and can be displayed to the user.

use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::server::state::TradeState;

/// Base URL for API calls (relative to current origin)
const API_BASE: &str = "/api";

/// Default session ID for shared state
const DEFAULT_SESSION_ID: &str = "default";

/// API response wrapper matching server format
#[derive(Deserialize, Serialize, Debug)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

/// Custom error type for API operations
#[derive(Debug, Serialize, Clone)]
pub enum ApiError {
    /// Network or fetch error
    NetworkError(String),
    /// Server returned an error
    ServerError(String),
    /// Failed to parse response
    ParseError(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ApiError::ServerError(msg) => write!(f, "Server error: {}", msg),
            ApiError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

/// Fetches the trade state from the server
///
/// This retrieves the shared trade state for the default session.
/// If no state exists on the server, a default state is returned.
///
/// # Returns
///
/// The current TradeState from the server
///
/// # Errors
///
/// Returns `ApiError` if the request fails or response cannot be parsed
pub async fn get_state() -> Result<TradeState, ApiError> {
    get_state_for_session(DEFAULT_SESSION_ID).await
}

/// Fetches the trade state for a specific session
///
/// # Arguments
///
/// * `session_id` - The session identifier
///
/// # Returns
///
/// The TradeState for the specified session
pub async fn get_state_for_session(session_id: &str) -> Result<TradeState, ApiError> {
    let url = format!("{}/state/{}", API_BASE, session_id);
    debug!("Fetching state from: {}", url);

    let window = web_sys::window().ok_or_else(|| ApiError::NetworkError("No window".to_string()))?;

    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::SameOrigin);

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    request
        .headers()
        .set("Accept", "application/json")
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| ApiError::ParseError("Response is not a Response object".to_string()))?;

    if !resp.ok() {
        return Err(ApiError::ServerError(format!(
            "HTTP {}: {}",
            resp.status(),
            resp.status_text()
        )));
    }

    let json = JsFuture::from(
        resp.json()
            .map_err(|e| ApiError::ParseError(format!("{:?}", e)))?,
    )
    .await
    .map_err(|e| ApiError::ParseError(format!("{:?}", e)))?;

    let api_response: ApiResponse<TradeState> = serde_wasm_bindgen::from_value(json)
        .map_err(|e| ApiError::ParseError(format!("{:?}", e)))?;

    if api_response.success {
        api_response
            .data
            .ok_or_else(|| ApiError::ParseError("No data in response".to_string()))
    } else {
        Err(ApiError::ServerError(
            api_response
                .error
                .unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }
}

/// Saves the trade state to the server
///
/// This saves the shared trade state for the default session.
/// All connected users will see the updated state.
///
/// # Arguments
///
/// * `state` - The TradeState to save
///
/// # Returns
///
/// `Ok(())` on success
///
/// # Errors
///
/// Returns `ApiError` if the request fails
pub async fn save_state(state: &TradeState) -> Result<(), ApiError> {
    save_state_for_session(DEFAULT_SESSION_ID, state).await
}

/// Saves the trade state for a specific session
///
/// # Arguments
///
/// * `session_id` - The session identifier
/// * `state` - The TradeState to save
///
/// # Returns
///
/// `Ok(())` on success
pub async fn save_state_for_session(session_id: &str, state: &TradeState) -> Result<(), ApiError> {
    let url = format!("{}/state/{}", API_BASE, session_id);
    debug!("Saving state to: {}", url);

    let window = web_sys::window().ok_or_else(|| ApiError::NetworkError("No window".to_string()))?;

    // Serialize the request body
    #[derive(Serialize)]
    struct SaveRequest<'a> {
        state: &'a TradeState,
    }

    let body = serde_json::to_string(&SaveRequest { state })
        .map_err(|e| ApiError::ParseError(format!("Failed to serialize: {}", e)))?;

    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.mode(RequestMode::SameOrigin);
    opts.body(Some(&wasm_bindgen::JsValue::from_str(&body)));

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    request
        .headers()
        .set("Content-Type", "application/json")
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    request
        .headers()
        .set("Accept", "application/json")
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| ApiError::NetworkError(format!("{:?}", e)))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| ApiError::ParseError("Response is not a Response object".to_string()))?;

    if !resp.ok() {
        return Err(ApiError::ServerError(format!(
            "HTTP {}: {}",
            resp.status(),
            resp.status_text()
        )));
    }

    info!("State saved successfully");
    Ok(())
}

/// Checks if the API server is reachable
///
/// This can be used to verify connectivity before attempting other operations.
///
/// # Returns
///
/// `true` if the server is reachable, `false` otherwise
pub async fn health_check() -> bool {
    let url = format!("{}/health", API_BASE);

    let window = match web_sys::window() {
        Some(w) => w,
        None => return false,
    };

    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::SameOrigin);

    let request = match Request::new_with_str_and_init(&url, &opts) {
        Ok(r) => r,
        Err(_) => return false,
    };

    match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(resp_value) => {
            if let Ok(resp) = resp_value.dyn_into::<Response>() {
                resp.ok()
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
    .await
    .map_err(|e| ApiError::ParseError(format!("{:?}", e)))?;

    let api_response: ApiResponse<TradeState> = serde_wasm_bindgen::from_value(json)
        .map_err(|e| ApiError::ParseError(format!("{:?}", e)))?;

    if api_response.success {
        api_response
            .data
            .ok_or_else(|| ApiError::ParseError("No data in response".to_string()))
    } else {
        Err(ApiError::ServerError(
            api_response.error.unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }
}
