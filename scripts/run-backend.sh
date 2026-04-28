#!/usr/bin/env bash
# Run the worldgen WebSocket backend (Trade + Simulator) locally.
#
# Defaults:
#   - Firestore disabled (FIRESTORE_DATABASE_ID=debug)   -> no GCP creds needed
#   - Logs at info, with simulator at debug
#   - Listens on 0.0.0.0:8081
#
# Override any env var on the command line, e.g.:
#   RUST_LOG=trace ./scripts/run-backend.sh
#   FIRESTORE_DATABASE_ID=worldgen ./scripts/run-backend.sh
set -euo pipefail
cd "$(dirname "$0")/.."

export FIRESTORE_DATABASE_ID="${FIRESTORE_DATABASE_ID:-debug}"
export RUST_LOG="${RUST_LOG:-info,worldgen::simulator=debug,worldgen::backend=debug}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
export WS_HOST="${WS_HOST:-0.0.0.0}"
export WS_PORT="${WS_PORT:-8081}"

# SENTRY_DSN is optional — set it to enable error reporting.
# If unset, the server starts without Sentry.

echo "▶ worldgen backend"
echo "  WS:        ws://${WS_HOST}:${WS_PORT}/ws/{trade,simulator}"
echo "  RUST_LOG:  ${RUST_LOG}"
echo "  Firestore: ${FIRESTORE_DATABASE_ID}"
echo "  Sentry:    ${SENTRY_DSN:+enabled}${SENTRY_DSN:-disabled (set SENTRY_DSN to enable)}"
echo

exec cargo run --bin server --features backend "$@"
