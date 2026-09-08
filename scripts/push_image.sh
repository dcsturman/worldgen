#!/bin/zsh

# Abort on the first failure — most importantly a failed `docker buildx
# build`. Without this, a build error (e.g. the buildx builder container
# dying after a laptop sleep) fell through to `gcloud run deploy`, which
# re-deployed the previous :latest image as a brand-new revision: stale
# code wearing a successful-deploy costume. Seen in the wild 2026-07-21.
set -e
set -o pipefail

# GCS_BUCKET controls the /world endpoint's planet-PNG cache. Without
# it (or set to "debug"), every /world request regenerates the planet
# from scratch (~25 s); with it, subsequent calls are served from the
# cache in ~200 ms. Prompt now rather than discovering the missing
# cache only after the first slow request lands in production.
if [ -z "$GCS_BUCKET" ]; then
  echo ""
  echo "GCS_BUCKET is not set in your shell environment."
  echo "  This controls the /world planet-PNG cache. Without a real"
  echo "  bucket, every /world request regenerates from scratch (~25 s);"
  echo "  with one, subsequent calls hit the cache (~200 ms)."
  echo ""
  echo "  Enter a bucket name to enable caching (e.g. worldgen-cache),"
  echo "  or 'debug' / blank to deploy without caching."
  read "GCS_BUCKET?Bucket: "
  if [ -z "$GCS_BUCKET" ]; then
    GCS_BUCKET=debug
  fi
fi

# If a real bucket was specified, sanity-check that it exists and is
# accessible. A missing bucket isn't fatal — the backend falls through
# to regenerating on every request and logs warnings — but the user
# almost certainly meant for caching to work, so flag it loudly.
if [ "$GCS_BUCKET" != "debug" ]; then
  if ! gcloud storage buckets describe gs://$GCS_BUCKET >/dev/null 2>&1; then
    echo ""
    echo "WARNING: bucket gs://$GCS_BUCKET does not exist or is not"
    echo "  accessible to your account. /world will regenerate on every"
    echo "  request (PUTs will fail). To create:"
    echo ""
    echo "    gcloud storage buckets create gs://$GCS_BUCKET \\"
    echo "        --location=us-central1 --uniform-bucket-level-access"
    echo ""
    echo "  Then grant the Cloud Run service account access:"
    echo ""
    echo "    gcloud storage buckets add-iam-policy-binding gs://$GCS_BUCKET \\"
    echo "        --member='serviceAccount:<cloud-run-sa-email>' \\"
    echo "        --role='roles/storage.objectUser'"
    echo ""
    read "REPLY?Deploy anyway? (y/N): "
    if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
      echo "Aborted."
      exit 1
    fi
  fi
fi

# TRAVELLERMAP_URL is baked into both the WASM bundle and the native
# server binary at compile time (option_env! in src/util.rs). Unset =>
# defaults to https://travellermap.com. Prompt now so a future you (or
# anyone else who comes back to a private deploy after a while) doesn't
# silently re-bake the public URL into a private build.
DEFAULT_TRAVELLERMAP_URL=https://travellermap.com
if [ -z "$TRAVELLERMAP_URL" ]; then
  echo ""
  echo "TRAVELLERMAP_URL is not set in your shell environment."
  echo "  This is the upstream TravellerMap host the frontend and the"
  echo "  simulator hit for sector lookups, search, and tile rendering."
  echo "  It's baked into the build, so you have to pick now — there's"
  echo "  no runtime override."
  echo ""
  echo "  Enter a custom URL (e.g. https://my.tmap.local) or press"
  echo "  enter to use the default ($DEFAULT_TRAVELLERMAP_URL)."
  read "TRAVELLERMAP_URL?TravellerMap URL: "
  if [ -z "$TRAVELLERMAP_URL" ]; then
    TRAVELLERMAP_URL=$DEFAULT_TRAVELLERMAP_URL
    echo "  → using default: $TRAVELLERMAP_URL"
  fi
fi

# Echo the final settings so the user can confirm what's about to be
# baked into the image before the long docker build kicks off.
echo ""
echo "Build configuration:"
echo "  TRAVELLERMAP_URL = $TRAVELLERMAP_URL"
echo "  GCS_BUCKET       = $GCS_BUCKET"
echo ""

# Ensure we're using the docker-container builder (supports caching)
docker buildx create --name worldgen-builder --driver docker-container --use 2>/dev/null || \
  docker buildx use worldgen-builder

# Build for Cloud Run (linux/amd64) with remote caching for faster rebuilds.
#
# TRAVELLERMAP_URL is forwarded as a build arg so the value resolved
# by the prompt above gets baked into BOTH the WASM bundle and the
# native server binary.
docker buildx build --platform linux/amd64 \
  --cache-from=type=registry,ref=gcr.io/$GCP_PROJECT/worldgen:buildcache \
  --cache-to=type=registry,ref=gcr.io/$GCP_PROJECT/worldgen:buildcache,mode=max \
  --build-arg TRAVELLERMAP_URL=$TRAVELLERMAP_URL \
  -t gcr.io/$GCP_PROJECT/worldgen \
  --push .

# Deploy to Cloud Run with environment variables. `--set-env-vars`
# replaces all existing vars, so every var the backend needs has to be
# named explicitly here — adding GCS_BUCKET to the existing trio.
#
# --cpu/--memory are stated explicitly rather than left to whatever the
# service happens to have. The globe texture build splits across cores
# (see `band_rows` in src/worldmap/globe.rs), so the vCPU count is a
# performance parameter of the renderer, not just a capacity knob: at 1
# vCPU a cold 2048x1024 globe takes ~24 s, at 4 it takes ~9 s, which is
# what keeps it under the "up to ~15 seconds" the TravellerMap client
# promises the user while it spins. 4 vCPU requires at least 2 GiB on
# Cloud Run, hence the memory. Leaving these implicit would mean a future
# deploy from a fresh checkout silently reverting the fix.
#
# --max-instances is not a preference: us-central1 allows this project 200
# total vCPU, and 4 vCPU x the previous cap of 100 instances asked for 400.
# The deploy is rejected outright without this. If the CPU count ever goes
# up again, this has to come down to match, or the quota has to go up.
gcloud run deploy worldgen \
  --image gcr.io/$GCP_PROJECT/worldgen \
  --region us-central1 \
  --platform managed \
  --allow-unauthenticated \
  --cpu 4 \
  --memory 2Gi \
  --max-instances 50 \
  --set-env-vars GCP_PROJECT=$GCP_PROJECT,FIRESTORE_DATABASE_ID=worldgen,GCS_BUCKET=$GCS_BUCKET,RUST_LOG=info
