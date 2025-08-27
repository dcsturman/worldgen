#!/bin/zsh

docker build --platform linux/amd64 -t gcr.io/$GCP_PROJECT/worldgen .
docker push gcr.io/$GCP_PROJECT/worldgen
gcloud run deploy worldgen --image gcr.io/$GCP_PROJECT/worldgen --region us-central1 --platform managed --allow-unauthenticated
