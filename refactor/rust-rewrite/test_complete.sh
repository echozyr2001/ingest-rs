#!/bin/bash

echo "🧪 Testing Inngest Rust Dev Server"
echo "=================================="

# Function to test endpoint with curl
test_endpoint() {
    local method=$1
    local url=$2
    local data=$3
    local description=$4
    
    echo -n "Testing $description... "
    
    if [ -n "$data" ]; then
        response=$(curl -s -X "$method" "$url" \
            -H "Content-Type: application/json" \
            -d "$data")
    else
        response=$(curl -s -X "$method" "$url")
    fi
    
    if [ $? -eq 0 ]; then
        echo "✅"
        echo "Response: $response" | head -c 200
        echo ""
        echo ""
    else
        echo "❌"
        echo "Failed to connect to $url"
        echo ""
    fi
}

# Start the dev server in background
echo "🚀 Starting dev server..."
cargo run -- dev --port 8288 > /dev/null 2>&1 &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"

# Wait for server to start
echo "⏳ Waiting for server to start..."
sleep 3

# Test if server is running
if ! curl -s http://127.0.0.1:8288/health > /dev/null; then
    echo "❌ Server failed to start or is not responding"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "✅ Server is running!"
echo ""

# Test endpoints
test_endpoint "GET" "http://127.0.0.1:8288/health" "" "Health Check"

test_endpoint "GET" "http://127.0.0.1:8288/" "" "Dashboard"

test_endpoint "GET" "http://127.0.0.1:8288/api/v1/functions" "" "List Functions (empty)"

# Register a test function
function_data='{
    "name": "test-function",
    "triggers": [{"event": "user.created"}],
    "slug": "test-function"
}'
test_endpoint "POST" "http://127.0.0.1:8288/api/v1/functions" "$function_data" "Register Function"

test_endpoint "GET" "http://127.0.0.1:8288/api/v1/functions" "" "List Functions (with registered function)"

# Send a test event
event_data='{
    "name": "user.created",
    "data": {"user_id": "123", "email": "test@example.com"},
    "user": {"id": "123"}
}'
test_endpoint "POST" "http://127.0.0.1:8288/e/test-key" "$event_data" "Send Event"

# Send an event that won't match any function
no_match_event='{
    "name": "order.created",
    "data": {"order_id": "456"}
}'
test_endpoint "POST" "http://127.0.0.1:8288/e/test-key" "$no_match_event" "Send Non-matching Event"

# Stop the dev server
echo "⏹️  Stopping server..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo ""
echo "🎉 Test completed!"
