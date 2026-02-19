#!/bin/sh
set -e

echo "Starting worldgen services..."
echo "RUST_LOG: ${RUST_LOG:-info}"
echo "GCP_PROJECT: ${GCP_PROJECT}"
echo "FIRESTORE_DATABASE_ID: ${FIRESTORE_DATABASE_ID}"

exec /usr/bin/supervisord -c /etc/supervisord.conf

