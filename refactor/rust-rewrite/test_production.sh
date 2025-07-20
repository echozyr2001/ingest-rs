#!/bin/bash

# Production Backend Test Script with Docker
echo "🐳 Phase 3 Production Backend Test with Docker"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to check if a service is healthy
check_service_health() {
    local service_name=$1
    local max_attempts=30
    local attempt=1
    
    echo -e "${YELLOW}⏳ Waiting for $service_name to be healthy...${NC}"
    
    while [ $attempt -le $max_attempts ]; do
        if docker-compose ps | grep -q "$service_name.*healthy"; then
            echo -e "${GREEN}✅ $service_name is healthy!${NC}"
            return 0
        fi
        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    echo -e "${RED}❌ $service_name failed to become healthy${NC}"
    return 1
}

# Function to test database connection
test_database_connection() {
    echo -e "\n${BLUE}🗄️  Testing PostgreSQL Connection${NC}"
    
    if docker exec inngest-postgres psql -U inngest_user -d inngest -c "SELECT version();" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ PostgreSQL connection successful${NC}"
        
        # Show database info
        echo -e "${YELLOW}📊 Database Information:${NC}"
        docker exec inngest-postgres psql -U inngest_user -d inngest -c "
            SELECT 
                current_database() as database_name,
                current_user as user_name,
                version() as postgresql_version;
        " 2>/dev/null | head -10
        
        return 0
    else
        echo -e "${RED}❌ PostgreSQL connection failed${NC}"
        return 1
    fi
}

# Function to test Redis connection
test_redis_connection() {
    echo -e "\n${BLUE}🔴 Testing Redis Connection${NC}"
    
    if docker exec inngest-redis redis-cli ping | grep -q "PONG"; then
        echo -e "${GREEN}✅ Redis connection successful${NC}"
        
        # Show Redis info
        echo -e "${YELLOW}📊 Redis Information:${NC}"
        docker exec inngest-redis redis-cli info server | grep -E "redis_version|os|arch|multiplexing_api" | head -5
        
        return 0
    else
        echo -e "${RED}❌ Redis connection failed${NC}"
        return 1
    fi
}

# Function to test production server
test_production_server() {
    echo -e "\n${BLUE}🚀 Testing Production Server${NC}"
    
    # Load environment variables
    set -a
    source .env 2>/dev/null || echo "Warning: .env file not found"
    set +a
    
    echo -e "${YELLOW}⏳ Building project...${NC}"
    if ! cargo build --quiet; then
        echo -e "${RED}❌ Build failed${NC}"
        return 1
    fi
    
    echo -e "${YELLOW}⏳ Starting production server in background...${NC}"
    
    # Start the server in background
    timeout 30s cargo run -- start --port 8080 --host 0.0.0.0 \
        --database-url "$DATABASE_URL" \
        --redis-url "$REDIS_URL" &
    
    local server_pid=$!
    sleep 5  # Give server time to start
    
    echo -e "${YELLOW}⏳ Testing server endpoints...${NC}"
    
    # Test health endpoint
    local health_response
    health_response=$(curl -s http://localhost:8080/health 2>/dev/null)
    
    if [ $? -eq 0 ] && echo "$health_response" | grep -q '"status":"ok"'; then
        echo -e "${GREEN}✅ Health endpoint responding${NC}"
        echo -e "${YELLOW}📊 Health Response:${NC}"
        echo "$health_response" | jq . 2>/dev/null || echo "$health_response"
        
        # Test root endpoint
        local root_response
        root_response=$(curl -s http://localhost:8080/ 2>/dev/null)
        
        if [ $? -eq 0 ] && echo "$root_response" | grep -q "Inngest"; then
            echo -e "${GREEN}✅ Root endpoint responding${NC}"
            echo -e "${YELLOW}📊 Root Response:${NC}"
            echo "$root_response" | jq . 2>/dev/null || echo "$root_response"
        else
            echo -e "${RED}❌ Root endpoint failed${NC}"
        fi
        
        # Kill the server
        kill $server_pid 2>/dev/null
        wait $server_pid 2>/dev/null
        
        return 0
    else
        echo -e "${RED}❌ Health endpoint failed${NC}"
        echo "Response: $health_response"
        
        # Kill the server
        kill $server_pid 2>/dev/null
        wait $server_pid 2>/dev/null
        
        return 1
    fi
}

# Main execution
echo -e "\n${YELLOW}🐳 Step 1: Starting Docker services...${NC}"

# Stop any existing containers
docker-compose down -v 2>/dev/null

# Start Docker services
if ! docker-compose up -d; then
    echo -e "${RED}❌ Failed to start Docker services${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Docker services started${NC}"

# Wait for services to be healthy
echo -e "\n${YELLOW}⏳ Step 2: Waiting for services to be ready...${NC}"

if ! check_service_health "inngest-postgres"; then
    echo -e "${RED}❌ PostgreSQL failed to start${NC}"
    docker-compose logs postgres
    exit 1
fi

if ! check_service_health "inngest-redis"; then
    echo -e "${RED}❌ Redis failed to start${NC}"
    docker-compose logs redis
    exit 1
fi

# Test database connection
echo -e "\n${YELLOW}🧪 Step 3: Testing Database Connections...${NC}"

if ! test_database_connection; then
    echo -e "${RED}❌ Database connection test failed${NC}"
    exit 1
fi

if ! test_redis_connection; then
    echo -e "${RED}❌ Redis connection test failed${NC}"
    exit 1
fi

# Test production server
echo -e "\n${YELLOW}🧪 Step 4: Testing Production Server...${NC}"

if test_production_server; then
    echo -e "\n${GREEN}🎉 All tests passed!${NC}"
    echo -e "${GREEN}✅ PostgreSQL: Connected and ready${NC}"
    echo -e "${GREEN}✅ Redis: Connected and ready${NC}"
    echo -e "${GREEN}✅ Production Server: Running and responding${NC}"
    
    echo -e "\n${YELLOW}🔧 Service URLs:${NC}"
    echo "• PostgreSQL: postgresql://inngest_user:inngest_password@localhost:5432/inngest"
    echo "• Redis: redis://localhost:6379"
    echo "• Health Check: http://localhost:8080/health"
    echo "• API Server: http://localhost:8080/"
    
    echo -e "\n${YELLOW}💡 Next Steps:${NC}"
    echo "1. Services are running in background"
    echo "2. Start production server: cargo run -- start"
    echo "3. Test with: curl http://localhost:8080/health"
    echo "4. Stop services: docker-compose down"
    
else
    echo -e "\n${RED}❌ Production server test failed${NC}"
    echo -e "${YELLOW}📋 Debugging information:${NC}"
    echo "• Check logs: docker-compose logs"
    echo "• Check server logs: cargo run -- start --verbose"
    echo "• Verify connections manually"
    exit 1
fi

echo -e "\n${BLUE}📊 Final Status:${NC}"
docker-compose ps
