#!/bin/zsh

export GCP_PROJECT=callisto-1731280702227

docker build --platform linux/amd64 -t gcr.io/$GCP_PROJECT/worldgen .
docker push gcr.io/$GCP_PROJECT/worldgen
gcloud run deploy --image gcr.io/$GCP_PROJECT/worldgen