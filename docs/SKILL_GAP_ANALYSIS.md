# CARNELIAN Skill Gap Analysis & Enhancement Plan

**Date:** February 26, 2026  
**Current Status:** 651 skills implemented (100% THUMMIM parity)  
**Purpose:** Identify missing native orchestration skills, Python data science capabilities, and testing enhancements

---

## Executive Summary

CARNELIAN has achieved **651 skills** with strong platform integration coverage. However, analysis reveals gaps in:
- **Native orchestration** (internal system management)
- **Python data science** ecosystem (ML/analytics)
- **Google Cloud Platform** integration
- **CI/CD test coverage**

---

## 1. Missing Native Orchestration Skills

### Current State
- **10 native ops** (basic system operations)
- **Strong external integrations** but weak internal orchestration

### Recommended Native Skills (15 new skills)

| Skill Name | Purpose | Implementation | Priority |
|------------|---------|----------------|----------|
| `skill-registry-sync` | Synchronize skill metadata across workers | Native Rust | High |
| `worker-health-monitor` | Monitor worker process health | Native Rust | High |
| `task-queue-rebalance` | Intelligent task distribution | Native Rust | High |
| `event-stream-filter` | Filter/route events by pattern | Native Rust | Medium |
| `ledger-compact` | Compact audit ledger | Native Rust | Medium |
| `config-hot-reload` | Reload config without restart | Native Rust | Medium |
| `circuit-breaker` | Fail-fast for failing skills | Native Rust | High |
| `rate-limiter` | Rate limit skill execution | Native Rust | High |
| `memory-pool-gc` | Garbage collect skill memory | Native Rust | Low |
| `skill-cache-warm` | Pre-warm skill caches | Native Rust | Low |
| `worker-spawn` | Dynamically spawn workers | Native Rust | Medium |
| `worker-terminate` | Gracefully terminate workers | Native Rust | Medium |
| `metrics-aggregate` | Aggregate system metrics | Native Rust | Medium |
| `alert-threshold` | Threshold-based alerting | Native Rust | Low |
| `backup-snapshot` | Create system snapshots | Native Rust | Medium |

**Total:** 15 native orchestration skills

---

## 2. Python Data Science Skills

### Current State
- **0 Python skills** in CARNELIAN
- THUMMIM has ~30 Python skills for ML/data science

### Recommended Python Skills (20 new skills)

#### Data Processing (5 skills)
| Skill Name | Library | Purpose |
|------------|---------|---------|
| `pandas-dataframe` | pandas | DataFrame operations (filter, group, merge) |
| `numpy-array` | numpy | Array computations and linear algebra |
| `polars-query` | polars | Fast DataFrame queries |
| `dask-parallel` | dask | Parallel data processing |
| `arrow-convert` | pyarrow | Apache Arrow conversions |

#### Visualization (4 skills)
| Skill Name | Library | Purpose |
|------------|---------|---------|
| `matplotlib-plot` | matplotlib | Static plots (line, bar, scatter) |
| `seaborn-visualize` | seaborn | Statistical visualizations |
| `plotly-interactive` | plotly | Interactive charts |
| `altair-chart` | altair | Declarative visualizations |

#### Machine Learning (6 skills)
| Skill Name | Library | Purpose |
|------------|---------|---------|
| `sklearn-train` | scikit-learn | Train ML models (classification, regression) |
| `sklearn-predict` | scikit-learn | Model inference |
| `xgboost-train` | xgboost | Gradient boosting |
| `lightgbm-train` | lightgbm | Fast gradient boosting |
| `tensorflow-inference` | tensorflow | Deep learning inference |
| `pytorch-inference` | torch | Neural network inference |

#### Statistical Analysis (3 skills)
| Skill Name | Library | Purpose |
|------------|---------|---------|
| `statsmodels-regression` | statsmodels | Statistical modeling |
| `scipy-stats` | scipy | Statistical tests |
| `scipy-optimize` | scipy | Optimization algorithms |

#### Specialized (2 skills)
| Skill Name | Library | Purpose |
|------------|---------|---------|
| `opencv-image` | opencv-python | Image processing |
| `networkx-graph` | networkx | Graph analysis |

**Total:** 20 Python data science skills

### Implementation Architecture

```
skills/
├── python-registry/           # New directory
│   ├── pandas-dataframe/
│   │   ├── skill.json
│   │   └── main.py
│   ├── sklearn-train/
│   │   ├── skill.json
│   │   └── main.py
│   └── ...
└── ...

workers/
├── python-worker/             # Existing
│   ├── requirements.txt       # Add ML dependencies
│   └── worker.py              # Enhanced for ML skills
```

**Dependencies to add:**
```txt
pandas>=2.0.0
numpy>=1.24.0
scikit-learn>=1.3.0
matplotlib>=3.7.0
seaborn>=0.12.0
plotly>=5.14.0
scipy>=1.11.0
statsmodels>=0.14.0
xgboost>=2.0.0
lightgbm>=4.0.0
tensorflow>=2.13.0  # Optional
torch>=2.0.0        # Optional
opencv-python>=4.8.0
networkx>=3.1.0
polars>=0.18.0
pyarrow>=12.0.0
dask>=2023.5.0
altair>=5.0.0
```

---

## 3. Google Cloud Platform Skills

### Current State
- AWS: 2 skills (S3, Lambda)
- Azure: 1 skill (Blob Storage)
- GCP: **1 skill** (Storage Upload) - needs expansion

### Recommended GCP Skills (12 new skills)

| Skill Name | GCP Service | Purpose |
|------------|-------------|---------|
| `gcp-storage-download` | Cloud Storage | Download objects |
| `gcp-storage-list` | Cloud Storage | List buckets/objects |
| `gcp-bigquery-query` | BigQuery | Run SQL queries |
| `gcp-bigquery-load` | BigQuery | Load data into tables |
| `gcp-pubsub-publish` | Pub/Sub | Publish messages |
| `gcp-pubsub-subscribe` | Pub/Sub | Subscribe to topics |
| `gcp-functions-deploy` | Cloud Functions | Deploy serverless functions |
| `gcp-run-deploy` | Cloud Run | Deploy containers |
| `gcp-compute-vm` | Compute Engine | Manage VMs |
| `gcp-secret-access` | Secret Manager | Access secrets |
| `gcp-firestore-doc` | Firestore | Document operations |
| `gcp-cloudsql-query` | Cloud SQL | Managed database queries |

**Total:** 12 GCP skills

**Implementation:** Add to `skills/node-registry/` using `@google-cloud/*` SDKs

---

## 4. CARNELIAN vs THUMMIM Final Comparison

### Skill Count Comparison

| Category | CARNELIAN | THUMMIM | Advantage |
|----------|-----------|---------|-----------|
| **Total Skills** | **651** | ~480 | CARNELIAN +171 |
| Platform APIs (Node.js) | 411 | ~300 | CARNELIAN +111 |
| Self-contained (WASM) | 230 | ~150 | CARNELIAN +80 |
| Native Ops | 10 | ~50 | THUMMIM +40 |
| Python Skills | 0 | ~30 | THUMMIM +30 |
| MCP Integration | ✅ Windsurf | ✅ Windsurf | Equal |
| UI Theme | Liquid Glass | Aurora Dusk | Different |
| CI/CD Stability | ⚠️ Needs work | ✅ Stable | THUMMIM better |

### Coverage Analysis

**CARNELIAN Strengths:**
- ✅ **More comprehensive** platform integrations (Stripe, PayPal, CRM, project management)
- ✅ **Stronger WASM** ecosystem (230 self-contained Rust skills)
- ✅ **Better encoding/crypto** coverage (base58, morse, soundex, metaphone, XOR, etc.)
- ✅ **Modern stack** (Rust, Axum, SQLx, WASM)

**THUMMIM Strengths:**
- ✅ **Better native orchestration** (50 internal skills vs 10)
- ✅ **Python ML/data science** ecosystem (~30 skills)
- ✅ **Stable CI/CD** pipeline
- ✅ **Production-tested** with longer runtime

### Recommended Enhancements

To achieve **100% feature parity + enhancements:**

1. **Add 15 native orchestration skills** → 666 total skills
2. **Add 20 Python data science skills** → 686 total skills
3. **Add 12 GCP skills** → 698 total skills
4. **Fix CI/CD pipeline** → Production-ready
5. **Add integration tests** → Quality assurance

**Final Target:** **698 skills** (145% of THUMMIM baseline)

---

## 5. CI/CD Test Coverage Enhancement

### Current Test Status

```
✅ Secret Scanning (passing)
❌ Rust Lint (failing - clippy errors)
⏭️ Rust Build & Test (skipped)
⏭️ Node.js Worker (skipped)
⏭️ Integration Tests (skipped)
```

### Issues Identified

1. **Clippy errors** in `carnelian-common/src/types.rs` (FIXED ✅)
2. **No integration tests** running
3. **Node.js worker tests** not executed
4. **Rust unit tests** skipped due to lint failure

### Recommended Test Enhancements

#### 1. Add Integration Tests (New)

```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests
on: [push, pull_request]

jobs:
  integration:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
    steps:
      - uses: actions/checkout@v4
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
      - name: Run integration tests
        run: cargo test --test '*' -- --test-threads=1
```

#### 2. Add E2E Tests

```rust
// tests/e2e/skill_execution.rs
#[tokio::test]
async fn test_skill_execution_flow() {
    // 1. Start server
    // 2. Execute skill via API
    // 3. Verify result
    // 4. Check ledger entry
}

#[tokio::test]
async fn test_worker_communication() {
    // Test Node.js worker skill execution
}

#[tokio::test]
async fn test_wasm_skill_loading() {
    // Test WASM skill compilation and execution
}
```

#### 3. Add Performance Tests

```rust
// tests/performance/throughput.rs
#[tokio::test]
async fn test_skill_throughput() {
    // Execute 1000 skills/sec
    // Measure latency p50, p95, p99
}

#[tokio::test]
async fn test_concurrent_skills() {
    // 100 concurrent skill executions
}
```

#### 4. Add Python Worker Tests

```yaml
# .github/workflows/python-worker.yml
name: Python Worker Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'
      - name: Install dependencies
        run: |
          cd workers/python-worker
          pip install -r requirements.txt
          pip install pytest pytest-asyncio
      - name: Run tests
        run: |
          cd workers/python-worker
          pytest tests/ -v
```

### Test Coverage Goals

| Test Type | Current | Target |
|-----------|---------|--------|
| Unit Tests | ~40% | 80% |
| Integration Tests | 0% | 60% |
| E2E Tests | 0% | 40% |
| Performance Tests | 0% | 20% |

---

## 6. Implementation Roadmap

### Phase 1: CI/CD Stabilization (Week 1)
- [x] Fix clippy errors
- [ ] Enable Rust build & test
- [ ] Add basic integration tests
- [ ] Add Node.js worker tests
- [ ] Add Python worker tests

### Phase 2: Native Orchestration (Week 2)
- [ ] Implement 15 native orchestration skills
- [ ] Add circuit breaker pattern
- [ ] Add rate limiting
- [ ] Add worker health monitoring
- [ ] Add ledger compaction

### Phase 3: Python Data Science (Week 3)
- [ ] Setup Python skill registry
- [ ] Implement 5 data processing skills
- [ ] Implement 4 visualization skills
- [ ] Implement 6 ML skills
- [ ] Implement 3 statistical skills
- [ ] Implement 2 specialized skills

### Phase 4: GCP Integration (Week 4)
- [ ] Implement 12 GCP skills
- [ ] Add GCP authentication
- [ ] Add GCP error handling
- [ ] Add GCP integration tests

### Phase 5: Testing & Quality (Week 5)
- [ ] Achieve 80% unit test coverage
- [ ] Add E2E test suite
- [ ] Add performance benchmarks
- [ ] Add load testing
- [ ] Documentation updates

---

## 7. Success Metrics

### Skill Coverage
- ✅ **651 skills** (100% THUMMIM parity)
- 🎯 **698 skills** (145% THUMMIM + enhancements)

### Quality Metrics
- 🎯 **80% test coverage**
- 🎯 **All CI checks passing**
- 🎯 **<100ms p95 latency** for skill execution
- 🎯 **1000+ skills/sec throughput**

### Feature Parity
- ✅ Platform integrations (CARNELIAN leads)
- ✅ WASM skills (CARNELIAN leads)
- 🎯 Native orchestration (achieve parity)
- 🎯 Python ML/data science (achieve parity)
- 🎯 CI/CD stability (achieve parity)

---

## Conclusion

CARNELIAN has achieved **651 skills** with strong platform integration coverage, surpassing THUMMIM's baseline. To achieve complete feature parity and production readiness:

1. **Add 15 native orchestration skills** for internal system management
2. **Add 20 Python data science skills** for ML/analytics workloads
3. **Add 12 GCP skills** for cloud platform completeness
4. **Stabilize CI/CD** with comprehensive test coverage
5. **Achieve 80% test coverage** for production confidence

**Final Target:** 698 skills, 80% test coverage, all CI checks passing

---

**Next Steps:**
1. Commit clippy fixes
2. Implement native orchestration skills
3. Setup Python skill registry
4. Add GCP skills
5. Enhance CI/CD pipeline
