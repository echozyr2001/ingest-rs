#!/bin/bash

echo "🧪 Testing Error Handling - Inngest Dev Server"
echo "==============================================="

# Start dev server in background
echo "🚀 Starting dev server..."
cargo run -- dev --port 8290 --verbose > /dev/null 2>&1 &
DEV_SERVER_PID=$!

# Wait for server to start
sleep 3

echo "✅ Dev server started (PID: $DEV_SERVER_PID)"

# Test invalid JSON
echo "🔍 Testing invalid JSON handling..."
RESPONSE=$(curl -s -X POST http://127.0.0.1:8290/api/v1/functions \
  -H "Content-Type: application/json" \
  -d '{invalid json}')

echo "Response: $RESPONSE"

if echo "$RESPONSE" | jq -e '.error and .success == false' > /dev/null 2>&1; then
    echo "✅ Error handling test PASSED"
else
    echo "❌ Error handling test FAILED"
fi

# Test valid function registration still works
echo "🔍 Testing valid function registration..."
VALID_RESPONSE=$(curl -s -X POST http://127.0.0.1:8290/api/v1/functions \
  -H "Content-Type: application/json" \
  -d '{"name": "test-function", "triggers": [{"event": "test.event"}], "slug": "test-function"}')

echo "Valid response: $VALID_RESPONSE"

if echo "$VALID_RESPONSE" | jq -e '.success == true' > /dev/null 2>&1; then
    echo "✅ Valid registration test PASSED"
else
    echo "❌ Valid registration test FAILED"
fi

# Stop the dev server
echo "⏹️  Stopping dev server..."
kill $DEV_SERVER_PID

echo "✅ Error handling tests completed!"
