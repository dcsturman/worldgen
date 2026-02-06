#!/bin/bash
set -e

# Worldgen Cloud Run Deployment Script
# This script builds and deploys the Worldgen application to Google Cloud Run

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Worldgen Cloud Run Deployment ===${NC}"

# Check if GCP_PROJECT is set
if [ -z "$GCP_PROJECT" ]; then
    echo -e "${RED}Error: GCP_PROJECT environment variable is not set${NC}"
    echo "Please set it with: export GCP_PROJECT=your-project-id"
    exit 1
fi

echo -e "${YELLOW}Project: $GCP_PROJECT${NC}"
echo -e "${YELLOW}Region: us-central1${NC}"

# Confirm deployment
read -p "Deploy to Cloud Run? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment cancelled"
    exit 0
fi

# Configure Docker authentication
echo -e "${GREEN}Configuring Docker authentication...${NC}"
gcloud auth configure-docker --quiet

# Build the Docker image
echo -e "${GREEN}Building Docker image...${NC}"
echo ""

# Use BuildKit for better caching and progress display
DOCKER_BUILDKIT=1 docker build \
    -f Dockerfile.server \
    --platform linux/amd64 \
    -t gcr.io/$GCP_PROJECT/worldgen \
    --progress=auto \
    .

# Push to Google Container Registry
echo -e "${GREEN}Pushing image to GCR...${NC}"
docker push gcr.io/$GCP_PROJECT/worldgen

# Deploy to Cloud Run
echo -e "${GREEN}Deploying to Cloud Run...${NC}"
gcloud run deploy worldgen \
    --image gcr.io/$GCP_PROJECT/worldgen \
    --region us-central1 \
    --platform managed \
    --allow-unauthenticated \
    --set-env-vars GCP_PROJECT=$GCP_PROJECT \
    --port 8080 \
    --memory 512Mi \
    --cpu 1 \
    --timeout 300 \
    --max-instances 10

# Get the service URL
SERVICE_URL=$(gcloud run services describe worldgen \
    --region us-central1 \
    --format='value(status.url)')

echo -e "${GREEN}=== Deployment Complete ===${NC}"
echo -e "${GREEN}Service URL: $SERVICE_URL${NC}"
echo ""
echo "Test the deployment:"
echo "  Health check: curl $SERVICE_URL/api/health"
echo "  Get state:    curl $SERVICE_URL/api/state"
echo ""
echo "View logs:"
echo "  gcloud run services logs read worldgen --region us-central1"
