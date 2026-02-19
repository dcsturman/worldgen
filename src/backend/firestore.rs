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
//! ## Database Structure
//!
//! Database: `worldgen`
//!
//! ```text
//! {session_id}/                  # Collection (e.g., "default" for shared state)
//!   └── state/                   # Document containing the trade state
//!       ├── version: u32         # Schema version for migrations
//!       ├── origin_world: World  # Origin world data
//!       ├── dest_world: World?   # Optional destination world
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

use firestore::{FirestoreDb, FirestoreDbOptions};

use log::{debug, error, info, warn};
use thiserror::Error;

use crate::comms::TradeState;

/// Document name for the trade state within each session collection
const STATE_DOCUMENT_NAME: &str = "state";

/// Default session ID for shared state (all users see the same state)
pub const DEFAULT_SESSION_ID: &str = "default";

/// Name for special case database that indicates no Firestore connection (for local debugging)
const NULL_DATABASE_NAME: &str = "debug";

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

    #[error("Schema mismatch (document has old format): {0}")]
    SchemaError(String),
}

pub async fn initialize_firestore() -> Result<Option<FirestoreDb>, FirestoreError> {
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

    // We use the special name in NULL_DATABASE to allow off-line debugging.  The code will just
    // not write anything to FireStore.
    if database_id.to_lowercase() == NULL_DATABASE_NAME {
        warn!("🔥 Initializing system with NULL FirestoreDb.");
        Ok(None)
    } else {
        let options = FirestoreDbOptions::new(project_id).with_database_id(database_id);

        let firestore_db = FirestoreDb::with_options(options).await.map_err(|e| {
            error!("❌ Firestore: Failed to initialize client: {}", e);
            error!("❌ Firestore: Error details: {:?}", e);
            FirestoreError::InitError(e.to_string())
        })?;
        info!("Firestore client initialized successfully");
        Ok(Some(firestore_db))
    }
}

/// Retrieves the trade state for a given session from Firestore
///
/// If the session doesn't exist, returns a default TradeState and creates
/// the document in Firestore for future updates.
///
/// # Arguments
///
/// * `db` - The Firestore database client
/// * `session_id` - The session identifier (use `DEFAULT_SESSION_ID` for shared state)
///
/// # Returns
///
/// The TradeState for the session, or a default state if not found
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn get_trade_state(
    db_option: &Option<FirestoreDb>,
    session_id: &str,
) -> Result<TradeState, FirestoreError> {
    debug!(
        "📖 Firestore: Fetching trade state for session: {}",
        session_id
    );

    match db_option {
        None => {
            warn!("🔥 Running without Firestore connection.");
            Ok(TradeState::default())
        }
        Some(db) => {
            // Try to get the document
            // Structure: {session_id}/{STATE_DOCUMENT_NAME}
            let result: Option<TradeState> = db
                .fluent()
                .select()
                .by_id_in(session_id)
                .obj()
                .one(STATE_DOCUMENT_NAME)
                .await
                .map_err(|e| {
                    // Check if this is a deserialization error (schema mismatch)
                    let error_str = e.to_string();
                    if error_str.contains("SerializationError") || error_str.contains("missing field") {
                        warn!("⚠️  Firestore: Document exists but has incompatible schema (likely old format): {}", e);
                        warn!("⚠️  Firestore: Will use default state and overwrite with new schema");
                        // Return a special error that we'll handle by using default state
                        FirestoreError::SchemaError(e.to_string())
                    } else {
                        error!("❌ Firestore: Failed to read trade state: {}", e);
                        error!("❌ Firestore: Error details: {:?}", e);
                        FirestoreError::ReadError(e.to_string())
                    }
                })?;

            match result {
                Some(state) => {
                    debug!(
                        "✅ Firestore: Found existing trade state for session: {}",
                        session_id
                    );
                    // Apply any necessary migrations
                    Ok(state)
                }
                None => {
                    info!(
                        "📝 Firestore: No trade state found for session: {}, creating default",
                        session_id
                    );
                    // Create default state and save it
                    let default_state = TradeState::default();
                    save_trade_state(db_option, session_id, &default_state).await?;
                    Ok(default_state)
                }
            }
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
/// * `db` - The Firestore database client
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
pub async fn save_trade_state(
    db_option: &Option<FirestoreDb>,
    session_id: &str,
    state: &TradeState,
) -> Result<(), FirestoreError> {
    info!("📝 Firestore: Starting save for session: {}", session_id);

    match db_option {
        None => {
            warn!("🔥 Saving state without Firestore connection.");
            Ok(())
        }
        Some(db) => {
            // Log what we're trying to serialize
            debug!("📝 Firestore: Serializing state: {:?}", state);

            // Try to serialize to JSON to see what it looks like
            match serde_json::to_string_pretty(state) {
                Ok(json) => {
                    debug!("📝 Firestore: State as JSON:\n{}", json);
                }
                Err(e) => {
                    error!(
                        "❌ Firestore: Failed to serialize state to JSON for logging: {}",
                        e
                    );
                }
            }

            // Upsert the document (create or update)
            // Structure: {session_id}/{STATE_DOCUMENT_NAME}
            let result = db
                .fluent()
                .update()
                .in_col(session_id)
                .document_id(STATE_DOCUMENT_NAME)
                .object(state)
                .execute::<()>()
                .await;

            match result {
                Ok(_) => {
                    info!(
                        "✅ Firestore: Successfully saved trade state for session: {}",
                        session_id
                    );
                    Ok(())
                }
                Err(e) => {
                    error!(
                        "❌ Firestore: Failed to save trade state for session {}: {}",
                        session_id, e
                    );
                    error!("❌ Firestore: Error details: {:?}", e);
                    Err(FirestoreError::WriteError(e.to_string()))
                }
            }
        }
    }
}

/// Deletes the trade state for a given session from Firestore
///
/// This is primarily used for testing or resetting state.
///
/// # Arguments
///
/// * `db` - The Firestore database client
/// * `session_id` - The session identifier to delete
///
/// # Returns
///
/// `Ok(())` on success (even if document didn't exist)
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn delete_trade_state(
    db_option: &Option<FirestoreDb>,
    session_id: &str,
) -> Result<(), FirestoreError> {
    debug!("Deleting trade state for session: {}", session_id);

    match db_option {
        None => {
            warn!("🔥 Deleting state without Firestore connection.");
            Ok(())
        }
        Some(db) => {
            // Structure: {session_id}/{STATE_DOCUMENT_NAME}
            db.fluent()
                .delete()
                .from(session_id)
                .document_id(STATE_DOCUMENT_NAME)
                .execute()
                .await
                .map_err(|e| {
                    error!("Failed to delete trade state: {}", e);
                    FirestoreError::WriteError(e.to_string())
                })?;

            info!("Deleted trade state for session: {}", session_id);
            Ok(())
        }
    }
}

/// Checks if a trade state exists for a given session
///
/// # Arguments
///
/// * `db` - The Firestore database client
/// * `session_id` - The session identifier to check
///
/// # Returns
///
/// `true` if the session exists, `false` otherwise
///
/// # Errors
///
/// Returns `FirestoreError` if the database operation fails
pub async fn trade_state_exists(
    db_option: &Option<FirestoreDb>,
    session_id: &str,
) -> Result<bool, FirestoreError> {
    match db_option {
        None => {
            debug!("🔥 Saving state without Firestore connection.");
            Ok(true)
        }
        Some(db) => {
            // Structure: {session_id}/{STATE_DOCUMENT_NAME}
            let result: Option<TradeState> = db
                .fluent()
                .select()
                .by_id_in(session_id)
                .obj()
                .one(STATE_DOCUMENT_NAME)
                .await
                .map_err(|e| FirestoreError::ReadError(e.to_string()))?;

            Ok(result.is_some())
        }
    }
}
