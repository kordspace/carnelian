#!/bin/bash
# Security Audit Script for CARNELIAN
# Performs comprehensive security checks before deployment

set -e

echo "🔒 CARNELIAN Security Audit"
echo "=========================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ISSUES_FOUND=0

# Function to report issues
report_issue() {
    echo -e "${RED}✗ FAIL:${NC} $1"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
}

report_pass() {
    echo -e "${GREEN}✓ PASS:${NC} $1"
}

report_warning() {
    echo -e "${YELLOW}⚠ WARN:${NC} $1"
}

echo "1. Checking for hardcoded secrets..."
if grep -r "password\s*=\s*['\"]" --include="*.rs" --include="*.toml" --include="*.yml" crates/ docker-compose*.yml 2>/dev/null | grep -v "test" | grep -v "example"; then
    report_issue "Found potential hardcoded passwords"
else
    report_pass "No hardcoded passwords found"
fi

if grep -r "api_key\s*=\s*['\"]" --include="*.rs" --include="*.toml" --include="*.yml" crates/ docker-compose*.yml 2>/dev/null | grep -v "test" | grep -v "example"; then
    report_issue "Found potential hardcoded API keys"
else
    report_pass "No hardcoded API keys found"
fi

echo ""
echo "2. Checking Docker secrets configuration..."
if [ -f "docker-compose.yml" ]; then
    if grep -q "secrets:" docker-compose.yml; then
        report_pass "Docker secrets configured"
    else
        report_warning "Docker secrets not configured in docker-compose.yml"
    fi
else
    report_warning "docker-compose.yml not found"
fi

echo ""
echo "3. Checking for SQL injection vulnerabilities..."
if grep -r "format!\|concat!" --include="*.rs" crates/ | grep -i "select\|insert\|update\|delete" | grep -v "test" | grep -v "//"; then
    report_warning "Potential SQL injection risk - using string formatting with SQL"
else
    report_pass "No obvious SQL injection vulnerabilities"
fi

echo ""
echo "4. Checking for XSS vulnerabilities..."
if grep -r "innerHTML\|dangerouslySetInnerHTML" --include="*.ts" --include="*.tsx" --include="*.js" skills/ 2>/dev/null; then
    report_warning "Potential XSS risk - using innerHTML"
else
    report_pass "No obvious XSS vulnerabilities"
fi

echo ""
echo "5. Checking HTTPS enforcement..."
if grep -q "HSTS\|Strict-Transport-Security" crates/carnelian-core/src/middleware/*.rs 2>/dev/null; then
    report_pass "HSTS headers configured"
else
    report_warning "HSTS headers not found - HTTPS not enforced"
fi

echo ""
echo "6. Checking CORS configuration..."
if [ -f "crates/carnelian-core/src/middleware/cors.rs" ]; then
    if grep -q "production" crates/carnelian-core/src/middleware/cors.rs; then
        report_pass "CORS production mode available"
    else
        report_warning "CORS production mode not configured"
    fi
else
    report_warning "CORS middleware not found"
fi

echo ""
echo "7. Checking rate limiting..."
if [ -f "crates/carnelian-core/src/middleware/rate_limit.rs" ]; then
    report_pass "Rate limiting middleware exists"
else
    report_warning "Rate limiting middleware not found"
fi

echo ""
echo "8. Checking for exposed debug endpoints..."
if grep -r "/debug\|/admin" --include="*.rs" crates/carnelian-core/src/server.rs 2>/dev/null | grep -v "//"; then
    report_warning "Potential debug/admin endpoints found"
else
    report_pass "No debug endpoints found"
fi

echo ""
echo "9. Checking dependency vulnerabilities..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit 2>&1 | grep -q "Vulnerabilities found"; then
        report_issue "Dependency vulnerabilities found (run 'cargo audit' for details)"
    else
        report_pass "No known dependency vulnerabilities"
    fi
else
    report_warning "cargo-audit not installed (run: cargo install cargo-audit)"
fi

echo ""
echo "10. Checking for exposed .env files..."
if [ -f ".env" ]; then
    if grep -q "^\.env$" .gitignore; then
        report_pass ".env file is gitignored"
    else
        report_issue ".env file exists but not in .gitignore"
    fi
fi

echo ""
echo "11. Checking secrets directory..."
if [ -d "secrets" ]; then
    if [ -f "secrets/.gitignore" ]; then
        report_pass "Secrets directory is protected"
    else
        report_warning "Secrets directory exists but no .gitignore"
    fi
fi

echo ""
echo "12. Checking for insecure dependencies..."
if grep -q "openssl.*=.*\"0\." Cargo.toml 2>/dev/null; then
    report_warning "Using potentially outdated OpenSSL version"
fi

echo ""
echo "13. Checking input validation..."
if [ -f "crates/carnelian-core/src/middleware/input_validation.rs" ]; then
    report_pass "Input validation middleware exists"
else
    report_warning "Input validation middleware not found"
fi

echo ""
echo "14. Checking for default credentials..."
if grep -r "admin:admin\|root:root\|postgres:postgres" --include="*.yml" --include="*.toml" . 2>/dev/null | grep -v "example" | grep -v "test"; then
    report_issue "Default credentials found in configuration"
else
    report_pass "No default credentials in configuration"
fi

echo ""
echo "15. Checking security headers..."
if [ -f "crates/carnelian-core/src/middleware/security_headers.rs" ]; then
    if grep -q "Content-Security-Policy\|X-Frame-Options" crates/carnelian-core/src/middleware/security_headers.rs; then
        report_pass "Security headers configured"
    else
        report_warning "Security headers incomplete"
    fi
else
    report_warning "Security headers middleware not found"
fi

echo ""
echo "=========================="
echo "Security Audit Complete"
echo "=========================="
echo ""

if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✓ No critical issues found${NC}"
    exit 0
else
    echo -e "${RED}✗ Found $ISSUES_FOUND critical issue(s)${NC}"
    echo "Please address these issues before deploying to production"
    exit 1
fi
