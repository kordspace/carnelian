#!/bin/bash
# Deployment Validation Script for CARNELIAN
# Validates that all services are running correctly

set -e

echo "🚀 CARNELIAN Deployment Validation"
echo "==================================="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

BASE_URL="${BASE_URL:-http://localhost:8080}"
API_KEY="${API_KEY:-test_api_key}"
FAILURES=0

check_service() {
    local name=$1
    local url=$2
    local expected_status=${3:-200}
    
    echo -n "Checking $name... "
    
    status=$(curl -s -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000")
    
    if [ "$status" = "$expected_status" ]; then
        echo -e "${GREEN}✓ OK${NC} (HTTP $status)"
    else
        echo -e "${RED}✗ FAIL${NC} (HTTP $status, expected $expected_status)"
        FAILURES=$((FAILURES + 1))
    fi
}

check_authenticated_service() {
    local name=$1
    local url=$2
    local expected_status=${3:-200}
    
    echo -n "Checking $name... "
    
    status=$(curl -s -o /dev/null -w "%{http_code}" -H "X-Carnelian-Key: $API_KEY" "$url" 2>/dev/null || echo "000")
    
    if [ "$status" = "$expected_status" ]; then
        echo -e "${GREEN}✓ OK${NC} (HTTP $status)"
    else
        echo -e "${RED}✗ FAIL${NC} (HTTP $status, expected $expected_status)"
        FAILURES=$((FAILURES + 1))
    fi
}

echo "1. Core Services"
echo "----------------"
check_service "Health endpoint" "$BASE_URL/health"
check_service "Metrics endpoint" "$BASE_URL/metrics"

echo ""
echo "2. API Endpoints (Authenticated)"
echo "---------------------------------"
check_authenticated_service "List skills" "$BASE_URL/api/skills"
check_authenticated_service "List memories" "$BASE_URL/api/memories?limit=10"
check_authenticated_service "List workflows" "$BASE_URL/api/workflows?limit=10"
check_authenticated_service "XP leaderboard" "$BASE_URL/api/xp/leaderboard?limit=10"

echo ""
echo "3. Authentication"
echo "-----------------"
echo -n "Checking unauthenticated request... "
status=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/api/skills" 2>/dev/null || echo "000")
if [ "$status" = "401" ]; then
    echo -e "${GREEN}✓ OK${NC} (Properly rejected)"
else
    echo -e "${RED}✗ FAIL${NC} (Expected 401, got $status)"
    FAILURES=$((FAILURES + 1))
fi

echo ""
echo "4. Database Connection"
echo "----------------------"
echo -n "Checking database... "
if docker-compose exec -T postgres pg_isready -U postgres > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${RED}✗ FAIL${NC}"
    FAILURES=$((FAILURES + 1))
fi

echo ""
echo "5. Docker Services"
echo "------------------"
services=("carnelian-core" "postgres" "ollama")
for service in "${services[@]}"; do
    echo -n "Checking $service... "
    if docker-compose ps | grep -q "$service.*Up"; then
        echo -e "${GREEN}✓ Running${NC}"
    else
        echo -e "${RED}✗ Not running${NC}"
        FAILURES=$((FAILURES + 1))
    fi
done

echo ""
echo "6. Resource Usage"
echo "-----------------"
if command -v docker &> /dev/null; then
    echo "Container resource usage:"
    docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}" | head -n 5
fi

echo ""
echo "7. Security Headers"
echo "-------------------"
echo -n "Checking security headers... "
headers=$(curl -s -I "$BASE_URL/health" 2>/dev/null)

has_hsts=false
has_csp=false
has_xframe=false

if echo "$headers" | grep -qi "strict-transport-security"; then
    has_hsts=true
fi

if echo "$headers" | grep -qi "content-security-policy"; then
    has_csp=true
fi

if echo "$headers" | grep -qi "x-frame-options"; then
    has_xframe=true
fi

if $has_hsts && $has_csp && $has_xframe; then
    echo -e "${GREEN}✓ All present${NC}"
elif $has_hsts || $has_csp || $has_xframe; then
    echo -e "${YELLOW}⚠ Partial${NC}"
else
    echo -e "${RED}✗ Missing${NC}"
    FAILURES=$((FAILURES + 1))
fi

echo ""
echo "8. Performance Check"
echo "--------------------"
echo -n "Measuring response time... "
response_time=$(curl -s -o /dev/null -w "%{time_total}" "$BASE_URL/health" 2>/dev/null || echo "0")
response_ms=$(echo "$response_time * 1000" | bc)

if (( $(echo "$response_time < 0.5" | bc -l) )); then
    echo -e "${GREEN}✓ Fast${NC} (${response_ms}ms)"
elif (( $(echo "$response_time < 1.0" | bc -l) )); then
    echo -e "${YELLOW}⚠ Acceptable${NC} (${response_ms}ms)"
else
    echo -e "${RED}✗ Slow${NC} (${response_ms}ms)"
fi

echo ""
echo "9. Logs Check"
echo "-------------"
echo "Recent errors in logs:"
if docker-compose logs --tail=50 carnelian-core 2>/dev/null | grep -i "error\|panic\|fatal" | tail -n 5; then
    echo -e "${YELLOW}⚠ Errors found in logs${NC}"
else
    echo -e "${GREEN}✓ No recent errors${NC}"
fi

echo ""
echo "==================================="
echo "Validation Complete"
echo "==================================="
echo ""

if [ $FAILURES -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
    echo "CARNELIAN is ready for production"
    exit 0
else
    echo -e "${RED}✗ $FAILURES check(s) failed${NC}"
    echo "Please address the issues before deploying"
    exit 1
fi
