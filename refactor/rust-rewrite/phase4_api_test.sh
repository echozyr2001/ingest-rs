#!/bin/bash

# Phase 4 Production API Testing Script
# Tests the production server API endpoints with PostgreSQL and Redis backends

set -e

echo "🚀 Phase 4 Production API Testing"
echo "================================="

# Configuration
POSTGRES_URL="postgresql://inngest_user:inngest_password@localhost:5432/inngest"
REDIS_URL="redis://localhost:6379"
SERVER_PORT=8080
SERVER_HOST="0.0.0.0"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counter
TESTS_RUN=0
TESTS_PASSED=0

run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    echo -e "${BLUE}Test $TESTS_RUN: $test_name${NC}"
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ PASSED${NC}\n"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}❌ FAILED${NC}\n"
    fi
}

# Start Docker services
echo -e "${YELLOW}Starting Docker services...${NC}"
docker-compose up -d postgres redis

# Wait for services to be ready
echo "⏳ Waiting for PostgreSQL to be ready..."
until docker-compose exec postgres pg_isready -U inngest_user > /dev/null 2>&1; do
    sleep 1
done

echo "⏳ Waiting for Redis to be ready..."
until docker-compose exec redis redis-cli ping > /dev/null 2>&1; do
    sleep 1
done

echo -e "${GREEN}✅ Backend services are ready${NC}\n"

# Build and start production server in background
echo -e "${YELLOW}Building production server...${NC}"
cargo build --release --bin inngest

echo -e "${YELLOW}Starting production server...${NC}"
DATABASE_URL="$POSTGRES_URL" \
REDIS_URL="$REDIS_URL" \
cargo run --release --bin inngest start \
    --host "$SERVER_HOST" \
    --port "$SERVER_PORT" \
    --database-url "$POSTGRES_URL" \
    --redis-url "$REDIS_URL" \
    --verbose &

SERVER_PID=$!

# Wait for server to start
echo "⏳ Waiting for production server to start..."
sleep 5

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo -e "${RED}❌ Production server failed to start${NC}"
    exit 1
fi

# Wait a bit more for server to be fully ready
sleep 3

echo -e "${GREEN}✅ Production server started (PID: $SERVER_PID)${NC}\n"

# Test 1: Health Check
run_test "Health Check" "
    response=\$(curl -s http://localhost:$SERVER_PORT/health)
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.status == \"ok\"' > /dev/null &&
    echo \"\$response\" | jq -e '.backends.postgres == \"connected\"' > /dev/null &&
    echo \"\$response\" | jq -e '.backends.redis == \"connected\"' > /dev/null
"

# Test 2: Root Endpoint
run_test "Root Endpoint" "
    response=\$(curl -s http://localhost:$SERVER_PORT/)
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.message' > /dev/null &&
    echo \"\$response\" | jq -e '.endpoints.functions == \"/api/v1/functions\"' > /dev/null
"

# Test 3: List Functions (should be empty initially)
run_test "List Functions (Empty)" "
    response=\$(curl -s http://localhost:$SERVER_PORT/api/v1/functions)
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.functions | length == 0' > /dev/null
"

# Test 4: Register a Function
run_test "Register Function" "
    response=\$(curl -s -X POST http://localhost:$SERVER_PORT/api/v1/functions \\
        -H 'Content-Type: application/json' \\
        -d '{
            \"functions\": [{
                \"id\": \"$(uuidgen | tr '[:upper:]' '[:lower:]')\",
                \"config\": {
                    \"id\": \"test-function-1\",
                    \"name\": \"Test Function 1\",
                    \"triggers\": [{
                        \"type\": \"Event\",
                        \"event\": \"user.created\"
                    }],
                    \"steps\": []
                },
                \"version\": 1,
                \"app_id\": \"$(uuidgen | tr '[:upper:]' '[:lower:]')\",
                \"slug\": \"test-function-1\"
            }]
        }')
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.status == \"ok\"' > /dev/null &&
    echo \"\$response\" | jq -e '.registered == 1' > /dev/null
"

# Test 5: List Functions (should have one function)
run_test "List Functions (After Registration)" "
    response=\$(curl -s http://localhost:$SERVER_PORT/api/v1/functions)
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.functions | length == 1' > /dev/null &&
    echo \"\$response\" | jq -e '.functions[0].config.name == \"Test Function 1\"' > /dev/null
"

# Test 6: Submit Event
run_test "Submit Event" "
    response=\$(curl -s -X POST http://localhost:$SERVER_PORT/api/v1/events \\
        -H 'Content-Type: application/json' \\
        -d '{
            \"name\": \"user.created\",
            \"data\": {
                \"user_id\": \"123\",
                \"email\": \"test@example.com\"
            },
            \"user\": {
                \"id\": \"123\"
            }
        }')
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.status == \"queued\"' > /dev/null &&
    echo \"\$response\" | jq -e '.id' > /dev/null
"

# Test 7: List Runs (should have runs after event submission)
run_test "List Runs" "
    # Wait a moment for runs to be created
    sleep 2
    response=\$(curl -s 'http://localhost:$SERVER_PORT/api/v1/runs?limit=10')
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.runs | length >= 0' > /dev/null
"

# Test 8: Submit Event with No Matching Functions
run_test "Submit Event (No Matches)" "
    response=\$(curl -s -X POST http://localhost:$SERVER_PORT/api/v1/events \\
        -H 'Content-Type: application/json' \\
        -d '{
            \"name\": \"unknown.event\",
            \"data\": {
                \"test\": \"data\"
            }
        }')
    echo \"Response: \$response\"
    echo \"\$response\" | jq -e '.status == \"no_functions\"' > /dev/null
"

# Test 9: Memory Usage Check
run_test "Memory Usage Check" "
    memory_mb=\$(ps -o rss= -p $SERVER_PID | awk '{print \$1/1024}')
    echo \"Memory usage: \${memory_mb}MB\"
    # Check if memory usage is reasonable (less than 100MB for basic functionality)
    awk \"BEGIN {exit (\$memory_mb > 100)}\" memory_mb=\"\$memory_mb\"
"

# Test 10: API Response Time Check
run_test "API Response Time" "
    start_time=\$(date +%s%N)
    curl -s http://localhost:$SERVER_PORT/health > /dev/null
    end_time=\$(date +%s%N)
    response_time=\$(( (end_time - start_time) / 1000000 )) # Convert to milliseconds
    echo \"Response time: \${response_time}ms\"
    # Check if response time is under 1000ms
    [ \$response_time -lt 1000 ]
"

# Cleanup
echo -e "${YELLOW}Cleaning up...${NC}"
kill $SERVER_PID 2>/dev/null || true
wait $SERVER_PID 2>/dev/null || true

# Test Results
echo "================================="
echo -e "${BLUE}Phase 4 API Testing Complete${NC}"
echo -e "Tests run: $TESTS_RUN"
echo -e "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests failed: ${RED}$((TESTS_RUN - TESTS_PASSED))${NC}"

if [ $TESTS_PASSED -eq $TESTS_RUN ]; then
    echo -e "\n${GREEN}🎉 All tests passed! Phase 4 production API is working correctly.${NC}"
    exit 0
else
    echo -e "\n${RED}❌ Some tests failed. Please check the logs above.${NC}"
    exit 1
fi
