#!/usr/bin/env bash
# Run the worldgen frontend (trunk dev server) locally.
#
# Uses --features local-dev so the WASM client opens its WebSocket directly
# on localhost:8081 instead of routing through nginx. Pair this with
# scripts/run-backend.sh in another terminal.
#
# Trunk serves on 127.0.0.1:8080 by default (see Trunk.toml).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "▶ worldgen frontend (trunk serve --features local-dev)"
echo "  HTTP: http://127.0.0.1:8080/"
echo "  Tabs: /, /world, /trade, /simulator"
echo "  Make sure scripts/run-backend.sh is running for /trade and /simulator."
echo

exec trunk serve --features local-dev "$@"
