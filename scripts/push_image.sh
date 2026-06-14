#!/bin/zsh

# Ensure we're using the docker-container builder (supports caching)
docker buildx create --name worldgen-builder --driver docker-container --use 2>/dev/null || \
  docker buildx use worldgen-builder

# Build for Cloud Run (linux/amd64) with remote caching for faster rebuilds
docker buildx build --platform linux/amd64 \
  --cache-from=type=registry,ref=gcr.io/$GCP_PROJECT/worldgen:buildcache \
  --cache-to=type=registry,ref=gcr.io/$GCP_PROJECT/worldgen:buildcache,mode=max \
  -t gcr.io/$GCP_PROJECT/worldgen \
  --push .

# Deploy to Cloud Run with environment variables
gcloud run deploy worldgen \
  --image gcr.io/$GCP_PROJECT/worldgen \
  --region us-central1 \
  --platform managed \
  --allow-unauthenticated \
  --set-env-vars GCP_PROJECT=$GCP_PROJECT,FIRESTORE_DATABASE_ID=worldgen,RUST_LOG=info
