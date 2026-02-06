# Testing Guide for Firestore Integration

This guide provides step-by-step instructions for testing the Firestore integration locally and in production.

## Quick Start - Local Testing

### 1. Set Environment Variables

```bash
export GCP_PROJECT="your-project-id"
export GOOGLE_APPLICATION_CREDENTIALS="$HOME/worldgen-dev-key.json"
```

### 2. Run the Local Server

```bash
./run_local.sh
```

Or manually:
```bash
trunk build --release
cargo run --bin server --features ssr
```

The server will start at `http://localhost:8080`

## Testing the API Endpoints

### Health Check

```bash
curl http://localhost:8080/api/health
# Expected: OK
```

### Get Default Session State

```bash
# First request creates default state
curl http://localhost:8080/api/state | jq

# Expected response:
# {
#   "success": true,
#   "data": {
#     "version": 1,
#     "origin_world": { ... },
#     "dest_world": null,
#     "available_goods": { ... },
#     ...
#   },
#   "error": null
# }
```

### Save State

```bash
# Save state for default session
curl -X POST http://localhost:8080/api/state \
  -H "Content-Type: application/json" \
  -d '{
    "state": {
      "version": 1,
      "origin_world": {
        "name": "Test World",
        "upp": "A123456-7",
        "starport": "A",
        "size": 1,
        "atmosphere": 2,
        "hydrographics": 3,
        "population": 4,
        "government": 5,
        "law_level": 6,
        "tech_level": 7,
        "trade_codes": [],
        "bases": [],
        "gas_giant": false,
        "mainworld": true
      },
      "dest_world": null,
      "available_goods": {
        "goods": [],
        "origin_world_name": "Test World"
      },
      "available_passengers": null,
      "ship_manifest": {
        "cargo": [],
        "passengers": [],
        "accumulated_profit": 0
      },
      "buyer_broker_skill": 2,
      "seller_broker_skill": 1,
      "steward_skill": 0,
      "illegal_goods": false
    }
  }' | jq
```

### Get Specific Session State

```bash
# Get state for a specific session
curl http://localhost:8080/api/state/my-session-123 | jq
```

### Save to Specific Session

```bash
curl -X POST http://localhost:8080/api/state/my-session-123 \
  -H "Content-Type: application/json" \
  -d '{ "state": { ... } }' | jq
```

### Delete Session State

```bash
curl -X DELETE http://localhost:8080/api/state/my-session-123 | jq
```

## Verify in Firestore Console

1. Go to https://console.cloud.google.com/firestore
2. Select your project
3. Navigate to the `trade_sessions` collection
4. You should see documents with IDs like `default`, `my-session-123`, etc.
5. Click on a document to see the stored state

## Testing the Frontend Integration

### 1. Open the Application

Navigate to `http://localhost:8080` in your browser

### 2. Open Browser Console

Press F12 to open developer tools

### 3. Test State Persistence

1. Go to the Trade Computer page
2. Make some changes (e.g., change broker skill, add cargo)
3. Check the Network tab - you should see POST requests to `/api/state`
4. Refresh the page
5. Your changes should persist (loaded from Firestore)

### 4. Test Multi-Session

Open two browser windows:
1. Window 1: `http://localhost:8080/trade.html?session=session1`
2. Window 2: `http://localhost:8080/trade.html?session=session2`

Changes in each window should be independent and persist separately.

## Common Issues and Solutions

### Issue: "Failed to initialize Firestore client"

**Solution**: Check that:
- `GCP_PROJECT` environment variable is set
- `GOOGLE_APPLICATION_CREDENTIALS` points to a valid service account key
- The service account has `roles/datastore.user` permission

```bash
# Verify environment variables
echo $GCP_PROJECT
echo $GOOGLE_APPLICATION_CREDENTIALS

# Check if file exists
ls -l $GOOGLE_APPLICATION_CREDENTIALS
```

### Issue: "Permission denied" errors

**Solution**: Grant Firestore permissions to your service account:

```bash
gcloud projects add-iam-policy-binding $GCP_PROJECT \
    --member="serviceAccount:worldgen-dev@$GCP_PROJECT.iam.gserviceaccount.com" \
    --role="roles/datastore.user"
```

### Issue: CORS errors in browser

**Solution**: The server has CORS enabled for all origins in development. If you still see errors:
1. Check that the server is running on the expected port
2. Verify the API URL in your frontend code
3. Check browser console for the exact error

### Issue: State not persisting

**Solution**: 
1. Check server logs for errors
2. Verify Firestore write succeeded in the console
3. Check that the session ID is consistent between saves and loads

## Production Testing (Cloud Run)

After deploying to Cloud Run:

```bash
# Get the service URL
SERVICE_URL=$(gcloud run services describe worldgen \
    --region us-central1 \
    --format='value(status.url)')

# Test health
curl $SERVICE_URL/api/health

# Test state endpoint
curl $SERVICE_URL/api/state | jq

# View logs
gcloud run services logs read worldgen --region us-central1 --limit 50
```

## Performance Testing

### Load Test with Apache Bench

```bash
# Install apache bench (if not already installed)
# macOS: brew install httpd
# Linux: apt-get install apache2-utils

# Test health endpoint
ab -n 1000 -c 10 http://localhost:8080/api/health

# Test state read endpoint
ab -n 100 -c 5 http://localhost:8080/api/state
```

### Monitor Firestore Usage

1. Go to https://console.cloud.google.com/firestore/usage
2. Check read/write operations
3. Monitor document count
4. Check storage usage

## Next Steps

Once local testing is successful:
1. Follow `SETUP_FIRESTORE.md` for Cloud Run deployment
2. Test the production deployment
3. Monitor logs and performance
4. Set up alerts for errors

