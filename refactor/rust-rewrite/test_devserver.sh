#!/bin/bash

# Test script for Inngest Rust Dev Server
echo "🧪 Testing Inngest Rust Dev Server"

# Start the dev server in background
echo "🚀 Starting dev server..."
cargo run -- dev --port 8288 &
DEV_SERVER_PID=$!

# Wait for server to start
sleep 3

# Test health endpoint
echo "🔍 Testing health endpoint..."
curl -s http://127.0.0.1:8288/health | jq .

# Test dashboard
echo "🖥️  Testing dashboard..."
curl -s http://127.0.0.1:8288/ | head -n 5

# Register a test function
echo "📝 Registering test function..."
curl -s -X POST http://127.0.0.1:8288/api/v1/functions \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-function",
    "triggers": [{"event": "user.created"}],
    "slug": "test-function"
  }' | jq .

# List functions
echo "📋 Listing functions..."
curl -s http://127.0.0.1:8288/api/v1/functions | jq .

# Send a test event
echo "📨 Sending test event..."
curl -s -X POST http://127.0.0.1:8288/e/test-key \
  -H "Content-Type: application/json" \
  -d '{
    "name": "user.created",
    "data": {"user_id": "123", "email": "test@example.com"},
    "user": {"id": "123"}
  }' | jq .

# Stop the dev server
echo "⏹️  Stopping dev server..."
kill $DEV_SERVER_PID

echo "✅ Tests completed!"
