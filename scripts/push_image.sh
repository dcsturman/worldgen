#!/bin/zsh

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

# Ensure we're using the docker-container builder (supports caching)
docker buildx create --name worldgen-builder --driver docker-container --use 2>/dev/null || \
  docker buildx use worldgen-builder

# Build for Cloud Run (linux/amd64) with remote caching for faster rebuilds
docker buildx build --platform linux/amd64 \
  --cache-from=type=registry,ref=gcr.io/$GCP_PROJECT/worldgen:buildcache \
  --cache-to=type=registry,ref=gcr.io/$GCP_PROJECT/worldgen:buildcache,mode=max \
  -t gcr.io/$GCP_PROJECT/worldgen \
  --push .

# Deploy to Cloud Run with environment variables. `--set-env-vars`
# replaces all existing vars, so every var the backend needs has to be
# named explicitly here — adding GCS_BUCKET to the existing trio.
gcloud run deploy worldgen \
  --image gcr.io/$GCP_PROJECT/worldgen \
  --region us-central1 \
  --platform managed \
  --allow-unauthenticated \
  --set-env-vars GCP_PROJECT=$GCP_PROJECT,FIRESTORE_DATABASE_ID=worldgen,GCS_BUCKET=$GCS_BUCKET,RUST_LOG=info
