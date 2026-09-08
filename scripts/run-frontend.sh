#!/usr/bin/env bash
# Run the worldgen frontend (trunk dev server) locally.
#
# Uses --features local-dev so the WASM client opens its WebSocket directly
# on localhost:8081 instead of routing through nginx. Pair this with
# scripts/run-backend.sh in another terminal.
#
# Trunk serves on 127.0.0.1:8080 by default (see Trunk.toml).
#
# Note on WORLDGEN_RENDER_THREADS: it does nothing here, deliberately. The
# frontend builds its globe texture inside the browser, where we have no
# threads, so that path is single-threaded by construction — see
# `render_threads` in src/worldmap/globe.rs, which is hardcoded to 1 on wasm.
# The frontend also renders at TexSize::STANDARD (1024x512), a quarter of the
# texels the server uses, which is what keeps it tolerable. If you want to
# measure or tune render threading, use scripts/run-backend.sh and hit
# /api/world.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "▶ worldgen frontend (trunk serve --features local-dev)"
echo "  HTTP: http://127.0.0.1:8080/"
echo "  Tabs: /, /world, /trade, /simulator"
echo "  Make sure scripts/run-backend.sh is running for /trade and /simulator."
echo

exec trunk serve --features local-dev "$@"
