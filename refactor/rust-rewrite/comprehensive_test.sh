#!/bin/bash

# Comprehensive test script for Inngest Rust Dev Server Phase 2
echo "🧪 Comprehensive Testing - Inngest Rust Dev Server Phase 2"
echo "=============================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counter
TOTAL_TESTS=0
PASSED_TESTS=0

# Function to run test
run_test() {
    local test_name="$1"
    local command="$2"
    local expected_status="$3"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -e "\n${BLUE}Test ${TOTAL_TESTS}: ${test_name}${NC}"
    echo "Command: $command"
    
    if eval "$command"; then
        if [ "$expected_status" = "success" ]; then
            echo -e "${GREEN}✅ PASSED${NC}"
            PASSED_TESTS=$((PASSED_TESTS + 1))
        else
            echo -e "${RED}❌ FAILED (expected failure but got success)${NC}"
        fi
    else
        if [ "$expected_status" = "failure" ]; then
            echo -e "${GREEN}✅ PASSED (expected failure)${NC}"
            PASSED_TESTS=$((PASSED_TESTS + 1))
        else
            echo -e "${RED}❌ FAILED${NC}"
        fi
    fi
}

# Start the dev server in background
echo -e "\n${YELLOW}🚀 Starting dev server...${NC}"
cargo run -- dev --port 8289 --verbose > test_server.log 2>&1 &
DEV_SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 5

# Test 1: Health Check
run_test "Health endpoint" \
    "curl -s -f http://127.0.0.1:8289/health | jq -e '.status == \"ok\"'" \
    "success"

# Test 2: Dashboard
run_test "Dashboard endpoint" \
    "curl -s -f http://127.0.0.1:8289/ | grep -q 'Inngest Dev Server'" \
    "success"

# Test 3: Function Registration - Basic
run_test "Register basic function" \
    "curl -s -f -X POST http://127.0.0.1:8289/api/v1/functions \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"basic-test\", \"triggers\": [{\"event\": \"test.basic\"}], \"slug\": \"basic-test\"}' \
     | jq -e '.success == true'" \
    "success"

# Test 4: Function Registration - Wildcard
run_test "Register wildcard function" \
    "curl -s -f -X POST http://127.0.0.1:8289/api/v1/functions \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"wildcard-test\", \"triggers\": [{\"event\": \"user.*\"}], \"slug\": \"wildcard-test\"}' \
     | jq -e '.success == true'" \
    "success"

# Test 5: Function Listing
run_test "List registered functions" \
    "curl -s -f http://127.0.0.1:8289/api/v1/functions | jq -e '.functions | length >= 2'" \
    "success"

# Test 6: Event Routing - Exact Match
run_test "Send exact match event" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/test-basic \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"test.basic\", \"data\": {\"message\": \"hello\"}}' \
     | jq -e '.matched_functions == 1'" \
    "success"

# Test 7: Event Routing - Wildcard Match
run_test "Send wildcard match event" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/user-event \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"user.login\", \"data\": {\"user_id\": \"123\"}}' \
     | jq -e '.matched_functions == 1'" \
    "success"

# Test 8: Event Routing - Multiple Matches
run_test "Register catch-all function for multiple matches" \
    "curl -s -f -X POST http://127.0.0.1:8289/api/v1/functions \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"catch-all\", \"triggers\": [{\"event\": \"user.login\"}], \"slug\": \"catch-all\"}' \
     | jq -e '.success == true'" \
    "success"

run_test "Send event that matches multiple functions" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/multi-match \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"user.login\", \"data\": {\"user_id\": \"456\"}}' \
     | jq -e '.matched_functions == 2'" \
    "success"

# Test 9: Event Routing - No Match
run_test "Send event with no matches" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/no-match \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"order.created\", \"data\": {\"order_id\": \"789\"}}' \
     | jq -e '.matched_functions == 0'" \
    "success"

# Test 10: Routing Metadata
run_test "Check routing metadata in response" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/metadata-test \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"user.signup\", \"data\": {}}' \
     | jq -e '.routing_metadata.pattern_matchers_count >= 0 and .routing_metadata.matched_count >= 0'" \
    "success"

# Test 11: Function Execution Attempts
run_test "Check execution attempts in response" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/execution-test \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"user.profile\", \"data\": {\"user_id\": \"999\"}}' \
     | jq -e '.executions | type == \"array\"'" \
    "success"

# Test 12: Invalid JSON handling
run_test "Handle invalid JSON in function registration" \
    "curl -s -X POST http://127.0.0.1:8289/api/v1/functions \
     -H 'Content-Type: application/json' \
     -d '{invalid json}' \
     | jq -e '.error and .success == false'" \
    "success"

# Test 13: API Events endpoint
run_test "Use API events endpoint" \
    "curl -s -f -X POST http://127.0.0.1:8289/api/v1/events \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"api.test\", \"data\": {\"api\": true}}' \
     | jq -e '.events | type == \"array\"'" \
    "success"

# Test 14: Complex event data
run_test "Send event with complex nested data" \
    "curl -s -f -X POST http://127.0.0.1:8289/e/complex-data \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"user.updated\", \"data\": {\"user\": {\"id\": 123, \"profile\": {\"name\": \"John\", \"settings\": [1,2,3]}}}, \"user\": {\"id\": \"123\"}}' \
     | jq -e '.success == true'" \
    "success"

# Test 15: Cron trigger registration
run_test "Register function with cron trigger" \
    "curl -s -f -X POST http://127.0.0.1:8289/api/v1/functions \
     -H 'Content-Type: application/json' \
     -d '{\"name\": \"cron-test\", \"triggers\": [{\"cron\": \"0 */5 * * *\"}], \"slug\": \"cron-test\"}' \
     | jq -e '.success == true'" \
    "success"

# Stop the dev server
echo -e "\n${YELLOW}⏹️  Stopping dev server...${NC}"
kill $DEV_SERVER_PID
wait $DEV_SERVER_PID 2>/dev/null

# Print summary
echo -e "\n${BLUE}=============================================================="
echo "Test Summary"
echo -e "==============================================================${NC}"
echo -e "Total Tests: ${TOTAL_TESTS}"
echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
echo -e "Failed: ${RED}$((TOTAL_TESTS - PASSED_TESTS))${NC}"

if [ $PASSED_TESTS -eq $TOTAL_TESTS ]; then
    echo -e "\n${GREEN}🎉 ALL TESTS PASSED! Phase 2 implementation is complete!${NC}"
    exit 0
else
    echo -e "\n${RED}❌ Some tests failed. Please check the implementation.${NC}"
    echo "Check test_server.log for detailed server logs."
    exit 1
fi
