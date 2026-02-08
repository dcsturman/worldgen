//! # Firestore Integration Module
//!
//! This module provides server-side Firestore integration for the Worldgen trade computer.
//! It handles all communication with Google Cloud Firestore for persistent state storage.
//!
//! ## Architecture
//!
//! The module uses the `firestore` crate which communicates with Firestore via gRPC.
//! On Google Cloud Run, authentication is automatic via the service account.
//! For local development, you need to set up Application Default Credentials.
//!
//! ## Authentication
//!
//! The Firestore client automatically detects credentials in this order:
//! 1. `GOOGLE_APPLICATION_CREDENTIALS` environment variable pointing to a service account JSON
//! 2. Application Default Credentials (from `gcloud auth application-default login`)
//! 3. GCE/Cloud Run metadata service (automatic on GCP)
//!
//! ## Collection Structure
//!
//! ```text
//! trade_sessions/
//!   └── {session_id}/           # Document ID (e.g., "default" for shared state)
//!       ├── version: u32        # Schema version for migrations
//!       ├── origin_world: World # Origin world data
//!       ├── dest_world: World?  # Optional destination world
//!       ├── available_goods: AvailableGoodsTable
//!       ├── available_passengers: AvailablePassengers?
//!       ├── ship_manifest: ShipManifest
//!       ├── buyer_broker_skill: i16
//!       ├── seller_broker_skill: i16
//!       ├── steward_skill: i16
//!       └── illegal_goods: bool
//! ```
//!
//! ## Error Handling
//!
//! All Firestore operations return `Result<T, FirestoreError>` to allow proper
//! error handling in server functions. Errors are logged and converted to
//! appropriate HTTP responses.

use firestore::*;
use log::{debug, error, info};
use std::sync::OnceLock;
use thiserror::Error;

use crate::backend::state::TradeState;

/// The Firestore collection name for trade sessions
const COLLECTION_NAME: &str = "trade_sessions";

/// Default session ID for shared state (all users see the same state)
pub const DEFAULT_SESSION_ID: &str = "default";

/// Global Firestore database instance
///
/// Initialized once on first use via `get_db()`. Using OnceLock ensures
/// thread-safe lazy initialization without runtime overhead after first access.
static FIRESTORE_DB: OnceLock<FirestoreDb> = OnceLock::new();

/// Custom error type for Firestore operations
#[derive(Error, Debug)]
pub enum FirestoreError {
    #[error("Firestore initialization failed: {0}")]
    InitError(String),

    #[error("Firestore read error: {0}")]
    ReadError(String),

    #[error("Firestore write error: {0}")]
    WriteError(String),

    #[error("Document not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Initializes and returns the Firestore database client
///
/// This function lazily initializes the Firestore client on first call.
/// Subsequent calls return the cached instance. The client automatically
/// detects GCP credentials from the environment.
///
/// # Returns
///
/// A reference to the initialized FirestoreDb instance
///
/// # Panics
///
/// Panics if Firestore initialization fails. This is intentional as the
/// application cannot function without database access.
///
/// # Environment Variables
///
/// - `GCP_PROJECT` or `GOOGLE_CLOUD_PROJECT`: The GCP project ID (required)
/// - `GOOGLE_APPLICATION_CREDENTIALS`: Path to service account JSON (optional, for local dev)
pub async fn get_db() -> &'static FirestoreDb {
    // If already initialized, return the cached instance
    if let Some(db) = FIRESTORE_DB.get() {
        return db;
    }

    // Get project ID from environment
    let project_id = std::env::var("GCP_PROJECT")
        .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
        .expect("GCP_PROJECT or GOOGLE_CLOUD_PROJECT environment variable must be set");

    let database_id = std::env::var("FIRESTORE_DATABASE_ID")
        .expect("FIRESTORE_DATABASE_ID environment variable must be set");

    info!("Initializing Firestore client for project: {} database: {}", &project_id, &database_id);

    let options = FirestoreDbOptions::new(project_id)
        .with_database_id(database_id);

    // Create the Firestore client
    let db = FirestoreDb::with_options(options)
        .await
        .expect("Failed to initialize Firestore client");

    // Store in the global static
    let _ = FIRESTORE_DB.set(db);

    FIRESTORE_DB.get().expect("Firestore DB should be set")
}

/// Retrieves the trade state for a given session from Firestore
///
/// If the session doesn't exist, returns a default TradeState and creates
/// the document in Firestore for future updates.
///
/// # Arguments
///
/// * `session_id` - The session identifier (use `DEFAULT_SESSION_ID` for shared state)
///
/// # Returns
///
/// The TradeState for the session, or a default state if not found
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn get_trade_state(session_id: &str) -> Result<TradeState, FirestoreError> {
    let db = get_db().await;

    debug!("Fetching trade state for session: {}", session_id);

    // Try to get the document
    let result: Option<TradeState> = db
        .fluent()
        .select()
        .by_id_in(COLLECTION_NAME)
        .obj()
        .one(session_id)
        .await
        .map_err(|e| {
            error!("Failed to read trade state: {}", e);
            FirestoreError::ReadError(e.to_string())
        })?;

    match result {
        Some(state) => {
            debug!("Found existing trade state for session: {}", session_id);
            // Apply any necessary migrations
            Ok(state.migrate())
        }
        None => {
            info!(
                "No trade state found for session: {}, creating default",
                session_id
            );
            // Create default state and save it
            let default_state = TradeState::default();
            save_trade_state(session_id, &default_state).await?;
            Ok(default_state)
        }
    }
}

/// Saves the trade state for a given session to Firestore
///
/// This performs an upsert operation - creating the document if it doesn't
/// exist, or updating it if it does.
///
/// # Arguments
///
/// * `session_id` - The session identifier (use `DEFAULT_SESSION_ID` for shared state)
/// * `state` - The TradeState to save
///
/// # Returns
///
/// `Ok(())` on success
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn save_trade_state(session_id: &str, state: &TradeState) -> Result<(), FirestoreError> {
    let db = get_db().await;

    info!("📝 Firestore: Starting save for session: {}", session_id);

    // Upsert the document (create or update)
    let result = db.fluent()
        .update()
        .in_col(COLLECTION_NAME)
        .document_id(session_id)
        .object(state)
        .execute::<()>()
        .await;

    match result {
        Ok(_) => {
            info!("✅ Firestore: Successfully saved trade state for session: {}", session_id);
            Ok(())
        }
        Err(e) => {
            error!("❌ Firestore: Failed to save trade state for session {}: {}", session_id, e);
            Err(FirestoreError::WriteError(e.to_string()))
        }
    }
}

/// Deletes the trade state for a given session from Firestore
///
/// This is primarily used for testing or resetting state.
///
/// # Arguments
///
/// * `session_id` - The session identifier to delete
///
/// # Returns
///
/// `Ok(())` on success (even if document didn't exist)
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn delete_trade_state(session_id: &str) -> Result<(), FirestoreError> {
    let db = get_db().await;

    debug!("Deleting trade state for session: {}", session_id);

    db.fluent()
        .delete()
        .from(COLLECTION_NAME)
        .document_id(session_id)
        .execute()
        .await
        .map_err(|e| {
            error!("Failed to delete trade state: {}", e);
            FirestoreError::WriteError(e.to_string())
        })?;

    info!("Deleted trade state for session: {}", session_id);
    Ok(())
}

/// Checks if a trade state exists for a given session
///
/// # Arguments
///
/// * `session_id` - The session identifier to check
///
/// # Returns
///
/// `true` if the session exists, `false` otherwise
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn trade_state_exists(session_id: &str) -> Result<bool, FirestoreError> {
    let db = get_db().await;

    let result: Option<TradeState> = db
        .fluent()
        .select()
        .by_id_in(COLLECTION_NAME)
        .obj()
        .one(session_id)
        .await
        .map_err(|e| FirestoreError::ReadError(e.to_string()))?;

    Ok(result.is_some())
}
