# CARNELIAN Implementation Roadmap

**Status:** CI Fixes Complete - Ready for Testing & Production Preparation  
**Timeline:** 4-6 days to production deployment  
**Current Build:** ✅ Passing (commit d1a9e0d)

---

## Phase 1: Test Suite Implementation (2-3 days)

### Day 1: Unit Tests Foundation

#### 1.1 Memory Layer Tests (Priority: High)
**Target Coverage:** 80%

```rust
// tests/unit/memory_tests.rs
#[cfg(test)]
mod memory_tests {
    use carnelian_core::memory::MemoryManager;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_create_memory() {
        let pool = create_test_pool().await;
        let manager = MemoryManager::new(pool);
        
        let memory = manager.create(CreateMemoryRequest {
            content: "test content".to_string(),
            metadata: json!({"type": "test"}),
            tags: vec!["test".to_string()],
        }).await.unwrap();
        
        assert_eq!(memory.content, "test content");
    }
    
    #[tokio::test]
    async fn test_vector_similarity_search() {
        let pool = create_test_pool().await;
        let manager = MemoryManager::new(pool);
        
        // Create test memories
        let mem1 = manager.create(...).await.unwrap();
        let mem2 = manager.create(...).await.unwrap();
        
        // Search by similarity
        let results = manager.search_similar(mem1.embedding, 5).await.unwrap();
        
        assert!(results.len() > 0);
        assert_eq!(results[0].id, mem1.id);
    }
    
    #[tokio::test]
    async fn test_memory_update() { /* ... */ }
    
    #[tokio::test]
    async fn test_memory_delete() { /* ... */ }
    
    #[tokio::test]
    async fn test_memory_tags() { /* ... */ }
}
```

**Files to Create:**
- `tests/unit/memory_tests.rs`
- `tests/unit/skill_execution_tests.rs`
- `tests/unit/xp_system_tests.rs`
- `tests/unit/elixir_tests.rs`
- `tests/helpers/mod.rs` (test utilities)

#### 1.2 Skill Execution Tests
```rust
// tests/unit/skill_execution_tests.rs
#[tokio::test]
async fn test_wasm_skill_execution() {
    let runtime = WasmSkillRuntime::new().await.unwrap();
    
    runtime.load("test-skill", "skills/registry/test-skill/skill.wasm").await.unwrap();
    
    let output = runtime.invoke("test-skill", SkillInput {
        action: "execute".to_string(),
        params: json!({"input": "test"}),
        identity_id: None,
        correlation_id: None,
    }, vec![]).await.unwrap();
    
    assert!(output.success);
}

#[tokio::test]
async fn test_skill_timeout() { /* ... */ }

#[tokio::test]
async fn test_skill_capability_enforcement() { /* ... */ }
```

#### 1.3 XP System Tests
```rust
// tests/unit/xp_system_tests.rs
#[tokio::test]
async fn test_award_xp() { /* ... */ }

#[tokio::test]
async fn test_xp_leaderboard() { /* ... */ }

#[tokio::test]
async fn test_xp_history() { /* ... */ }
```

### Day 2: Integration Tests

#### 2.1 API Integration Tests
**Target:** All major API endpoints

```rust
// tests/integration/api_tests.rs
#[tokio::test]
async fn test_skill_execution_api() {
    let server = start_test_server().await;
    
    let response = server.post("/api/skills/execute")
        .json(&json!({
            "skill_name": "test-skill",
            "input": {"key": "value"}
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
    let body: InvokeResponse = response.json().await.unwrap();
    assert_eq!(body.status, InvokeStatus::Completed);
}

#[tokio::test]
async fn test_memory_crud_api() { /* ... */ }

#[tokio::test]
async fn test_workflow_execution_api() { /* ... */ }

#[tokio::test]
async fn test_xp_award_api() { /* ... */ }
```

**Files to Create:**
- `tests/integration/api_tests.rs`
- `tests/integration/workflow_tests.rs`
- `tests/integration/memory_api_tests.rs`

#### 2.2 Database Integration Tests
```rust
// tests/integration/database_tests.rs
#[tokio::test]
async fn test_connection_pool_exhaustion() {
    // Simulate high concurrent load
    let pool = create_pool().await;
    let tasks: Vec<_> = (0..100).map(|_| {
        let pool = pool.clone();
        tokio::spawn(async move {
            pool.acquire().await
        })
    }).collect();
    
    // All should succeed
    for task in tasks {
        assert!(task.await.is_ok());
    }
}

#[tokio::test]
async fn test_transaction_deadlock_retry() { /* ... */ }
```

### Day 3: E2E Tests

#### 3.1 User Workflow Tests (Playwright)
```javascript
// tests/e2e/workflow_execution.spec.js
import { test, expect } from '@playwright/test';

test('complete workflow execution', async ({ page }) => {
    await page.goto('http://localhost:8080');
    
    // Create workflow
    await page.click('[data-testid="create-workflow"]');
    await page.fill('[data-testid="workflow-name"]', 'Test Workflow');
    await page.fill('[data-testid="workflow-description"]', 'E2E test workflow');
    
    // Add skills
    await page.click('[data-testid="add-skill"]');
    await page.selectOption('[data-testid="skill-select"]', 'test-skill');
    
    // Execute
    await page.click('[data-testid="execute-workflow"]');
    
    // Verify completion
    await expect(page.locator('[data-testid="status"]')).toContainText('Completed');
    await expect(page.locator('[data-testid="result"]')).toBeVisible();
});

test('memory creation and search', async ({ page }) => { /* ... */ });
test('skill execution with approval', async ({ page }) => { /* ... */ });
```

**Files to Create:**
- `tests/e2e/workflow_execution.spec.js`
- `tests/e2e/memory_management.spec.js`
- `tests/e2e/skill_execution.spec.js`
- `playwright.config.js`

#### 3.2 Test Infrastructure Setup
```toml
# Cargo.toml additions
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.12"
wiremock = "0.6"
```

```json
// package.json for E2E tests
{
  "devDependencies": {
    "@playwright/test": "^1.40.0"
  },
  "scripts": {
    "test:e2e": "playwright test"
  }
}
```

---

## Phase 2: Security Hardening (1 day)

### 2.1 Docker Secrets Implementation

#### Create Secrets Directory
```bash
mkdir -p secrets
echo "your_postgres_password" > secrets/postgres_password.txt
echo "your_carnelian_api_key" > secrets/carnelian_api_key.txt
chmod 600 secrets/*
```

#### Update docker-compose.yml
```yaml
# docker-compose.yml
version: '3.8'

secrets:
  postgres_password:
    file: ./secrets/postgres_password.txt
  carnelian_api_key:
    file: ./secrets/carnelian_api_key.txt

services:
  postgres:
    secrets:
      - postgres_password
    environment:
      - POSTGRES_PASSWORD_FILE=/run/secrets/postgres_password
  
  carnelian-core:
    secrets:
      - postgres_password
      - carnelian_api_key
    environment:
      - DATABASE_PASSWORD_FILE=/run/secrets/postgres_password
      - CARNELIAN_API_KEY_FILE=/run/secrets/carnelian_api_key
```

#### Update Code to Read Secrets
```rust
// crates/carnelian-core/src/config.rs
pub fn read_secret(secret_name: &str) -> Result<String> {
    let secret_path = format!("/run/secrets/{}", secret_name);
    std::fs::read_to_string(&secret_path)
        .map(|s| s.trim().to_string())
        .map_err(|e| Error::Config(format!("Failed to read secret {}: {}", secret_name, e)))
}

pub fn get_database_password() -> Result<String> {
    // Try secret file first, fallback to env var
    if let Ok(password) = read_secret("postgres_password") {
        Ok(password)
    } else {
        std::env::var("POSTGRES_PASSWORD")
            .map_err(|_| Error::Config("POSTGRES_PASSWORD not set".to_string()))
    }
}
```

### 2.2 Rate Limiting Middleware

```rust
// crates/carnelian-core/src/middleware/rate_limit.rs
use tower_governor::{GovernorLayer, GovernorConfigBuilder};
use std::time::Duration;

pub fn create_rate_limiter() -> GovernorLayer<'static, PeerIpKeyExtractor> {
    let governor_conf = Box::new(
        GovernorConfigBuilder::default()
            .per_second(10)  // 10 requests per second
            .burst_size(20)  // Allow bursts up to 20
            .finish()
            .unwrap(),
    );
    
    GovernorLayer {
        config: Box::leak(governor_conf),
    }
}
```

```toml
# Cargo.toml
[dependencies]
tower-governor = "0.4"
```

#### Apply to Server
```rust
// crates/carnelian-core/src/server.rs
use crate::middleware::rate_limit::create_rate_limiter;

pub async fn create_server(config: Config) -> Result<Router> {
    let app = Router::new()
        .route("/api/skills/execute", post(execute_skill))
        // ... other routes
        .layer(create_rate_limiter())  // Add rate limiting
        .layer(middleware::from_fn_with_state(state.clone(), carnelian_key_auth));
    
    Ok(app)
}
```

### 2.3 CORS Configuration

```rust
// crates/carnelian-core/src/middleware/cors.rs
use tower_http::cors::{CorsLayer, Any};
use http::Method;

pub fn create_cors_layer(production: bool) -> CorsLayer {
    if production {
        // Strict CORS for production
        CorsLayer::new()
            .allow_origin("https://yourdomain.com".parse::<HeaderValue>().unwrap())
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .max_age(Duration::from_secs(3600))
    } else {
        // Permissive CORS for development
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}
```

```toml
# Cargo.toml
[dependencies]
tower-http = { version = "0.5", features = ["cors"] }
```

### 2.4 Security Audit Checklist

- [ ] All secrets moved to Docker secrets
- [ ] Rate limiting enabled on all API endpoints
- [ ] CORS configured for production domains
- [ ] API key validation enforced
- [ ] SQL injection prevention verified (parameterized queries)
- [ ] XSS prevention in responses
- [ ] HTTPS enforced in production
- [ ] Security headers added (CSP, HSTS, X-Frame-Options)
- [ ] Input validation on all endpoints
- [ ] Error messages don't leak sensitive info

---

## Phase 3: Performance Testing (1 day)

### 3.1 Load Testing with k6

#### Install k6
```bash
# macOS
brew install k6

# Linux
sudo apt-key adv --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6
```

#### Create Load Test Scripts
```javascript
// tests/performance/load_test.js
import http from 'k6/http';
import { check, sleep } from 'k6';

export let options = {
    stages: [
        { duration: '30s', target: 10 },  // Ramp up to 10 users
        { duration: '1m', target: 50 },   // Ramp up to 50 users
        { duration: '2m', target: 50 },   // Stay at 50 users
        { duration: '30s', target: 0 },   // Ramp down
    ],
    thresholds: {
        http_req_duration: ['p(95)<500'], // 95% of requests under 500ms
        http_req_failed: ['rate<0.01'],   // Less than 1% failure rate
    },
};

export default function() {
    // Test skill execution
    let skillResponse = http.post('http://localhost:8080/api/skills/execute', JSON.stringify({
        skill_name: 'test-skill',
        input: { key: 'value' }
    }), {
        headers: { 'Content-Type': 'application/json' },
    });
    
    check(skillResponse, {
        'status is 200': (r) => r.status === 200,
        'response time < 500ms': (r) => r.timings.duration < 500,
    });
    
    // Test memory query
    let memoryResponse = http.get('http://localhost:8080/api/memories?limit=10');
    
    check(memoryResponse, {
        'status is 200': (r) => r.status === 200,
        'response time < 200ms': (r) => r.timings.duration < 200,
    });
    
    sleep(1);
}
```

#### Run Load Tests
```bash
# Standard profile test
k6 run tests/performance/load_test.js

# Performance profile test (higher load)
k6 run --vus 100 --duration 5m tests/performance/load_test.js
```

### 3.2 Memory Query Benchmarks

```rust
// benches/memory_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use carnelian_core::memory::MemoryManager;

fn benchmark_vector_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(create_test_pool());
    let manager = MemoryManager::new(pool);
    
    c.bench_function("vector_search_10", |b| {
        b.to_async(&rt).iter(|| async {
            manager.search_similar(black_box(test_embedding()), 10).await
        });
    });
    
    c.bench_function("vector_search_100", |b| {
        b.to_async(&rt).iter(|| async {
            manager.search_similar(black_box(test_embedding()), 100).await
        });
    });
}

fn benchmark_memory_create(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(create_test_pool());
    let manager = MemoryManager::new(pool);
    
    c.bench_function("memory_create", |b| {
        b.to_async(&rt).iter(|| async {
            manager.create(black_box(test_memory_request())).await
        });
    });
}

criterion_group!(benches, benchmark_vector_search, benchmark_memory_create);
criterion_main!(benches);
```

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }

[[bench]]
name = "memory_benchmarks"
harness = false
```

#### Run Benchmarks
```bash
cargo bench --bench memory_benchmarks
```

### 3.3 Skill Execution Benchmarks

```rust
// benches/skill_benchmarks.rs
fn benchmark_wasm_skill_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let runtime = rt.block_on(WasmSkillRuntime::new()).unwrap();
    
    rt.block_on(runtime.load("test-skill", "skills/registry/test-skill/skill.wasm")).unwrap();
    
    c.bench_function("wasm_skill_execute", |b| {
        b.to_async(&rt).iter(|| async {
            runtime.invoke("test-skill", black_box(test_input()), vec![]).await
        });
    });
}
```

### 3.4 Performance Targets

| Metric | Standard Profile | Performance Profile | Status |
|--------|------------------|---------------------|--------|
| Skill Execution | <100ms | <50ms | ⏳ To Test |
| Model Inference | <5s | <2s | ⏳ To Test |
| Memory Query | <50ms | <20ms | ⏳ To Test |
| API Response | <200ms | <100ms | ⏳ To Test |
| Concurrent Tasks | 10-20 | 50-100 | ⏳ To Test |
| Memory Ops/sec | 100-200 | 500-1000 | ⏳ To Test |

---

## Phase 4: Production Deployment (1 day)

### 4.1 Choose Machine Profile

**Decision Matrix:**

| Use Case | Profile | Rationale |
|----------|---------|-----------|
| Development/Testing | Standard | Cost-effective, sufficient for dev |
| Small Team (<10 users) | Standard | Adequate performance |
| Production (<50 users) | Performance | Better UX, faster responses |
| Production (50+ users) | Performance | Required for scale |
| Enterprise | Performance | High-quality models needed |

### 4.2 Configure Monitoring

#### Prometheus + Grafana Setup
```yaml
# docker-compose.monitoring.yml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    ports:
      - "9090:9090"
  
  grafana:
    image: grafana/grafana:latest
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana/dashboards:/etc/grafana/provisioning/dashboards
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin

volumes:
  prometheus_data:
  grafana_data:
```

#### Prometheus Configuration
```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'carnelian'
    static_configs:
      - targets: ['carnelian-core:8080']
  
  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres:5432']
```

#### Add Metrics to Carnelian
```rust
// crates/carnelian-core/src/metrics.rs
use prometheus::{IntCounter, Histogram, Registry};

lazy_static! {
    pub static ref SKILL_EXECUTIONS: IntCounter = IntCounter::new(
        "carnelian_skill_executions_total",
        "Total number of skill executions"
    ).unwrap();
    
    pub static ref SKILL_DURATION: Histogram = Histogram::new(
        "carnelian_skill_duration_seconds",
        "Skill execution duration"
    ).unwrap();
}

pub fn register_metrics(registry: &Registry) {
    registry.register(Box::new(SKILL_EXECUTIONS.clone())).unwrap();
    registry.register(Box::new(SKILL_DURATION.clone())).unwrap();
}
```

### 4.3 Deployment Checklist

#### Pre-Deployment
- [ ] All tests passing (unit, integration, E2E)
- [ ] Performance benchmarks meet targets
- [ ] Security audit complete
- [ ] Docker secrets configured
- [ ] Machine profile selected
- [ ] Monitoring configured
- [ ] Backup strategy implemented
- [ ] Rollback plan documented

#### Deployment Steps
```bash
# 1. Pull latest code
git pull origin main

# 2. Build images
docker-compose build

# 3. Run database migrations
docker-compose run carnelian-core carnelian migrate

# 4. Start services (choose profile)
docker-compose -f docker-compose.yml -f docker-compose.performance.yml up -d

# 5. Verify health
curl http://localhost:8080/health

# 6. Check logs
docker-compose logs -f carnelian-core

# 7. Start monitoring
docker-compose -f docker-compose.monitoring.yml up -d
```

#### Post-Deployment
- [ ] Health check passing
- [ ] All services running
- [ ] Metrics being collected
- [ ] Logs aggregating properly
- [ ] Test skill execution
- [ ] Test memory operations
- [ ] Monitor resource usage
- [ ] Verify backup running

### 4.4 Production Validation

```bash
# Test skill execution
curl -X POST http://localhost:8080/api/skills/execute \
  -H "Content-Type: application/json" \
  -d '{"skill_name": "test-skill", "input": {}}'

# Test memory creation
curl -X POST http://localhost:8080/api/memories \
  -H "Content-Type: application/json" \
  -d '{"content": "test", "metadata": {}}'

# Check metrics
curl http://localhost:8080/metrics

# Monitor logs
docker-compose logs -f --tail=100 carnelian-core
```

---

## Timeline Summary

| Phase | Duration | Status |
|-------|----------|--------|
| **Phase 1: Test Suite** | 2-3 days | ⏳ Pending |
| - Unit Tests | 1 day | ⏳ Pending |
| - Integration Tests | 1 day | ⏳ Pending |
| - E2E Tests | 1 day | ⏳ Pending |
| **Phase 2: Security** | 1 day | ⏳ Pending |
| **Phase 3: Performance** | 1 day | ⏳ Pending |
| **Phase 4: Deployment** | 1 day | ⏳ Pending |
| **Total** | **5-7 days** | ⏳ Pending |

---

## Success Criteria

### Test Suite
- ✅ Unit test coverage ≥ 80%
- ✅ All integration tests passing
- ✅ E2E tests cover critical user flows
- ✅ CI/CD pipeline includes all tests

### Security
- ✅ No secrets in environment variables
- ✅ Rate limiting prevents abuse
- ✅ CORS properly configured
- ✅ Security audit checklist complete

### Performance
- ✅ All benchmarks meet targets
- ✅ Load tests pass at expected concurrency
- ✅ No memory leaks detected
- ✅ Response times within SLA

### Deployment
- ✅ Zero-downtime deployment possible
- ✅ Monitoring and alerting active
- ✅ Backup/restore tested
- ✅ Rollback procedure documented

---

## Next Immediate Actions

1. **Wait for CI to pass** on commit `d1a9e0d`
2. **Begin Phase 1** - Create test infrastructure
3. **Implement unit tests** for memory layer
4. **Set up integration test framework**
5. **Configure E2E testing with Playwright**

**Status:** Ready to proceed once CI confirms build passes ✅
