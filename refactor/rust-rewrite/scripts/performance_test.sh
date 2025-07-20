#!/bin/bash

# Performance comparison script between Go and Rust implementations
set -e

echo "🔥 Inngest Go vs Rust Performance Comparison"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GO_PORT=8288
RUST_PORT=8289
TEST_DURATION=30
CONCURRENT_USERS=10
TEST_EVENTS=1000

# Helper functions
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check if Go version exists
    if [ ! -f "../cmd/main.go" ]; then
        log_error "Go implementation not found at ../cmd/main.go"
        exit 1
    fi
    
    # Check if Rust version exists
    if [ ! -f "Cargo.toml" ]; then
        log_error "Rust implementation not found (no Cargo.toml)"
        exit 1
    fi
    
    # Check for required tools
    command -v curl >/dev/null 2>&1 || { log_error "curl is required but not installed"; exit 1; }
    command -v ab >/dev/null 2>&1 || { log_error "apache-bench (ab) is required but not installed"; exit 1; }
    command -v ps >/dev/null 2>&1 || { log_error "ps is required but not installed"; exit 1; }
    
    log_success "Prerequisites check passed"
}

# Build both versions
build_versions() {
    log_info "Building Go version..."
    cd ..
    if ! go build -o inngest-go ./cmd/main.go; then
        log_error "Failed to build Go version"
        exit 1
    fi
    cd rust-rewrite
    
    log_info "Building Rust version..."
    if ! cargo build --release; then
        log_error "Failed to build Rust version"
        exit 1
    fi
    
    log_success "Both versions built successfully"
}

# Start servers
start_go_server() {
    log_info "Starting Go server on port $GO_PORT..."
    cd ..
    ./inngest-go dev --port $GO_PORT --no-discovery --no-poll > go_server.log 2>&1 &
    GO_PID=$!
    cd rust-rewrite
    
    # Wait for server to start
    for i in {1..30}; do
        if curl -s "http://localhost:$GO_PORT/health" > /dev/null 2>&1; then
            log_success "Go server started (PID: $GO_PID)"
            return 0
        fi
        sleep 1
    done
    
    log_error "Go server failed to start"
    exit 1
}

start_rust_server() {
    log_info "Starting Rust server on port $RUST_PORT..."
    ./target/release/inngest dev --port $RUST_PORT --no-discovery --no-poll > rust_server.log 2>&1 &
    RUST_PID=$!
    
    # Wait for server to start
    for i in {1..30}; do
        if curl -s "http://localhost:$RUST_PORT/health" > /dev/null 2>&1; then
            log_success "Rust server started (PID: $RUST_PID)"
            return 0
        fi
        sleep 1
    done
    
    log_error "Rust server failed to start"
    exit 1
}

# Performance tests
run_health_check_test() {
    local name=$1
    local port=$2
    local url="http://localhost:$port/health"
    
    log_info "Running health check performance test for $name..."
    
    # Run Apache Bench test
    local result=$(ab -n $TEST_EVENTS -c $CONCURRENT_USERS -q "$url" 2>/dev/null)
    
    # Extract metrics
    local requests_per_sec=$(echo "$result" | grep "Requests per second" | awk '{print $4}')
    local time_per_request=$(echo "$result" | grep "Time per request" | head -1 | awk '{print $4}')
    local transfer_rate=$(echo "$result" | grep "Transfer rate" | awk '{print $3}')
    
    echo "$name Results:"
    echo "  Requests/sec: $requests_per_sec"
    echo "  Time/request: ${time_per_request}ms"
    echo "  Transfer rate: ${transfer_rate} KB/sec"
    echo ""
    
    # Store results for comparison
    if [ "$name" = "Go" ]; then
        GO_RPS=$requests_per_sec
        GO_TPR=$time_per_request
        GO_TRANSFER=$transfer_rate
    else
        RUST_RPS=$requests_per_sec
        RUST_TPR=$time_per_request
        RUST_TRANSFER=$transfer_rate
    fi
}

run_event_ingestion_test() {
    local name=$1
    local port=$2
    local url="http://localhost:$port/e/test-key"
    
    log_info "Running event ingestion test for $name..."
    
    # Create test payload
    local payload='{"name":"test.event","data":{"test":true}}'
    
    # Run Apache Bench test with POST
    local result=$(ab -n 100 -c 5 -p <(echo "$payload") -T "application/json" -q "$url" 2>/dev/null)
    
    local requests_per_sec=$(echo "$result" | grep "Requests per second" | awk '{print $4}')
    local time_per_request=$(echo "$result" | grep "Time per request" | head -1 | awk '{print $4}')
    
    echo "$name Event Ingestion:"
    echo "  Requests/sec: $requests_per_sec"
    echo "  Time/request: ${time_per_request}ms"
    echo ""
}

# Memory usage monitoring
get_memory_usage() {
    local pid=$1
    local name=$2
    
    # Get RSS memory in KB
    local memory_kb=$(ps -o rss= -p $pid 2>/dev/null | tr -d ' ')
    if [ -n "$memory_kb" ]; then
        local memory_mb=$((memory_kb / 1024))
        echo "$name Memory Usage: ${memory_mb}MB"
        
        if [ "$name" = "Go" ]; then
            GO_MEMORY=$memory_mb
        else
            RUST_MEMORY=$memory_mb
        fi
    else
        echo "$name Memory Usage: N/A (process not found)"
    fi
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    
    if [ -n "$GO_PID" ]; then
        kill $GO_PID 2>/dev/null || true
    fi
    
    if [ -n "$RUST_PID" ]; then
        kill $RUST_PID 2>/dev/null || true
    fi
    
    # Wait a moment for processes to terminate
    sleep 2
    
    # Force kill if still running
    kill -9 $GO_PID 2>/dev/null || true
    kill -9 $RUST_PID 2>/dev/null || true
    
    log_success "Cleanup completed"
}

# Comparison and reporting
generate_report() {
    echo ""
    echo "📊 PERFORMANCE COMPARISON REPORT"
    echo "================================="
    echo ""
    
    # Health check comparison
    echo "🏥 Health Check Performance:"
    echo "Go:   $GO_RPS req/sec, ${GO_TPR}ms/req, ${GO_TRANSFER} KB/sec"
    echo "Rust: $RUST_RPS req/sec, ${RUST_TPR}ms/req, ${RUST_TRANSFER} KB/sec"
    echo ""
    
    # Calculate improvements
    if [ -n "$GO_RPS" ] && [ -n "$RUST_RPS" ]; then
        local rps_improvement=$(echo "scale=2; ($RUST_RPS / $GO_RPS - 1) * 100" | bc 2>/dev/null || echo "N/A")
        echo "Throughput improvement: ${rps_improvement}%"
    fi
    
    echo ""
    echo "💾 Memory Usage:"
    echo "Go:   ${GO_MEMORY}MB"
    echo "Rust: ${RUST_MEMORY}MB"
    
    if [ -n "$GO_MEMORY" ] && [ -n "$RUST_MEMORY" ] && [ "$GO_MEMORY" -gt 0 ]; then
        local memory_reduction=$(echo "scale=2; (1 - $RUST_MEMORY / $GO_MEMORY) * 100" | bc 2>/dev/null || echo "N/A")
        echo "Memory reduction: ${memory_reduction}%"
    fi
    
    echo ""
    echo "📁 Binary Sizes:"
    if [ -f "../inngest-go" ]; then
        local go_size=$(du -h "../inngest-go" | cut -f1)
        echo "Go binary:   $go_size"
    fi
    
    if [ -f "target/release/inngest" ]; then
        local rust_size=$(du -h "target/release/inngest" | cut -f1)
        echo "Rust binary: $rust_size"
    fi
    
    echo ""
    echo "🎯 Goals vs Actual:"
    echo "Target: 2x throughput improvement, 50% memory reduction"
    echo "Note: Results may vary based on system load and configuration"
}

# Main execution
main() {
    # Trap cleanup on exit
    trap cleanup EXIT
    
    echo "Starting performance comparison..."
    echo "Configuration:"
    echo "  Test duration: ${TEST_DURATION}s"
    echo "  Concurrent users: $CONCURRENT_USERS"
    echo "  Test events: $TEST_EVENTS"
    echo ""
    
    check_prerequisites
    build_versions
    
    # Test Go version
    start_go_server
    sleep 2
    get_memory_usage $GO_PID "Go"
    run_health_check_test "Go" $GO_PORT
    run_event_ingestion_test "Go" $GO_PORT
    
    # Test Rust version
    start_rust_server
    sleep 2
    get_memory_usage $RUST_PID "Rust"
    run_health_check_test "Rust" $RUST_PORT
    run_event_ingestion_test "Rust" $RUST_PORT
    
    generate_report
}

# Check if bc is available for calculations
if ! command -v bc >/dev/null 2>&1; then
    log_warning "bc (calculator) not available, percentage calculations will show N/A"
fi

# Run main function
main "$@"
