#!/bin/bash
set -e

# Worldgen Local Development Server Script
# This script builds the WASM frontend and runs the Axum server locally

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Worldgen Local Development Server ===${NC}"

# Check if GCP_PROJECT is set
if [ -z "$GCP_PROJECT" ]; then
    echo -e "${RED}Error: GCP_PROJECT environment variable is not set${NC}"
    echo "Please set it with: export GCP_PROJECT=your-project-id"
    exit 1
fi

# Check if GOOGLE_APPLICATION_CREDENTIALS is set
if [ -z "$GOOGLE_APPLICATION_CREDENTIALS" ]; then
    echo -e "${YELLOW}Warning: GOOGLE_APPLICATION_CREDENTIALS is not set${NC}"
    echo "Firestore authentication may fail. Set it with:"
    echo "  export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account-key.json"
    echo ""
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 0
    fi
fi

echo -e "${YELLOW}Project: $GCP_PROJECT${NC}"
echo -e "${YELLOW}Credentials: ${GOOGLE_APPLICATION_CREDENTIALS:-<not set>}${NC}"
echo ""

# Build the WASM frontend
echo -e "${GREEN}Building WASM frontend...${NC}"
trunk build --release

# Run the server
echo -e "${GREEN}Starting Axum server...${NC}"
echo -e "${YELLOW}Server will be available at: http://localhost:8080${NC}"
echo -e "${YELLOW}API endpoints at: http://localhost:8080/api/*${NC}"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""

cargo run --bin server --features ssr

