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
#   WORLDGEN_RENDER_THREADS=1 ./scripts/run-backend.sh   # serial planet renders
#
# WORLDGEN_RENDER_THREADS caps the workers the globe texture build splits
# across. Unset means one per available core, which is what you want almost
# always; set it to 1 to reproduce how a single-vCPU Cloud Run instance
# behaves, or to a small number to leave cores for the rest of your machine.
# Note this is a *native* setting: the in-browser render path is
# single-threaded regardless, since wasm has no threads here.
set -euo pipefail
cd "$(dirname "$0")/.."

export FIRESTORE_DATABASE_ID="${FIRESTORE_DATABASE_ID:-debug}"
export RUST_LOG="${RUST_LOG:-info,worldgen::simulator=debug,worldgen::backend=debug}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
export WS_HOST="${WS_HOST:-0.0.0.0}"
export WS_PORT="${WS_PORT:-8081}"
# Never write to the production GCS render cache from a local run, even if the
# shell has GCS_BUCKET set from deploy work: local renders come from whatever
# branch is checked out. To test caching deliberately, name a bucket with
# GCS_BUCKET_LOCAL=<bucket>.
export GCS_BUCKET="${GCS_BUCKET_LOCAL:-debug}"
# No browser caching either: the ETags identify a response by its inputs, not
# by the code that drew it, so a browser would keep showing what the previous
# build rendered after a restart.
export WORLDGEN_NO_HTTP_CACHE="${WORLDGEN_NO_HTTP_CACHE:-1}"
# Unset by default: the renderer then uses one worker per core.
if [ -n "${WORLDGEN_RENDER_THREADS:-}" ]; then
  export WORLDGEN_RENDER_THREADS
fi

# SENTRY_DSN is optional — set it to enable error reporting.
# If unset, the server starts without Sentry.

echo "▶ worldgen backend"
echo "  WS:        ws://${WS_HOST}:${WS_PORT}/ws/{trade,simulator}"
echo "  RUST_LOG:  ${RUST_LOG}"
echo "  Firestore: ${FIRESTORE_DATABASE_ID}"
echo "  GCS cache: ${GCS_BUCKET}   HTTP cache: $([ "$WORLDGEN_NO_HTTP_CACHE" = 1 ] && echo off || echo on)"
echo "  Sentry:    ${SENTRY_DSN:+enabled}${SENTRY_DSN:-disabled (set SENTRY_DSN to enable)}"
echo "  Render:    ${WORLDGEN_RENDER_THREADS:-all cores} thread(s)"
echo

exec cargo run --bin server --features backend "$@"
