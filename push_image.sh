#!/bin/zsh

export GCP_PROJECT=callisto-1731280702227

docker build --platform linux/amd64 -t gcr.io/$GCP_PROJECT/worldgen .
docker push gcr.io/$GCP_PROJECT/worldgen
gcloud run deploy worldgen --image gcr.io/$GCP_PROJECT/worldgen --region us-central1 --platform managed --allow-unauthenticated
