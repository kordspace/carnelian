# 🔥 Carnelian OS — MAGIC (Mixed Authenticated Quantum Intelligence Core)

MAGIC is an optional quantum-entropy subsystem that replaces the OS CSPRNG with quantum-derived randomness for key generation, ledger salting, and mantra scheduling. OS entropy is always the safe fallback — MAGIC never blocks core operations.

---

## Provider Priority

| Priority | Provider | Source | Requirement |
|----------|----------|--------|-------------|
| 1 | `quantum-origin` | Quantinuum Quantum Origin REST API | `CARNELIAN_QUANTUM_ORIGIN_API_KEY` |
| 2 | `quantinuum-h2` | Quantinuum H2 hardware / emulator (via pytket) | `carnelian magic auth` |
| 3 | `qiskit-rng` | IBM Quantum (via Qiskit) | `IBM_QUANTUM_TOKEN` |
| 4 | `os` | OS CSPRNG (`OsRng`) — always available | None |

---

## Quantum Origin Setup

Quantum Origin is the simplest setup: register at `quantinuum.com`, obtain an API key, and set it either in `machine.toml` (`quantum_origin_api_key = "..."`) or via `CARNELIAN_QUANTUM_ORIGIN_API_KEY`.

```bash
# Set via environment variable
export CARNELIAN_QUANTUM_ORIGIN_API_KEY="your-api-key-here"

# Or add to machine.toml
[magic]
enabled = true
quantum_origin_api_key = "your-api-key-here"
quantum_origin_url = "https://origin.quantinuum.com"
```

The provider uses a 5-second timeout and performs a single automatic retry on network errors.

---

## Quantinuum H2 Setup

Step-by-step configuration for Quantinuum H2 hardware or emulator access:

### 1. Install Dependencies

```bash
pip install pytket pytket-quantinuum
```

### 2. Authenticate Interactively

```bash
carnelian magic auth
```

This prompts for your Quantinuum email and password, calls `qapi.quantinuum.com/v1/login`, and stores tokens encrypted in `config_store`.

### 3. Configure Device

Set `quantinuum_enabled = true` and choose your device in `machine.toml`:

```toml
[magic]
enabled = true
quantinuum_enabled = true
quantinuum_device = "H1-1E"  # Free emulator
# quantinuum_device = "H2-1"  # Real hardware (requires credits)
```

### 4. Token Refresh

Tokens expire after 1 hour. Refresh manually:

```bash
carnelian magic auth --refresh
```

Or automate via `POST /v1/magic/auth/quantinuum/refresh`.

---

## Qiskit / IBM Quantum Setup

### 1. Install Dependencies

```bash
pip install qiskit qiskit-ibm-runtime
```

### 2. Obtain Token

Register at `quantum.ibm.com` and obtain your API token. Set it as an environment variable:

```bash
export IBM_QUANTUM_TOKEN="your-ibm-token-here"
```

### 3. Configure Backend

Set `qiskit_enabled = true` and choose your backend in `machine.toml`:

```toml
[magic]
enabled = true
qiskit_enabled = true
qiskit_backend = "ibm_brisbane"  # Or other available backend
```

---

## Offline / Fallback Behaviour

The `MixedEntropyProvider` implements a waterfall strategy: it always calls `OsEntropyProvider.get_bytes()` first, then attempts quantum providers in priority order. If all quantum sources fail (network outage, missing credentials, timeout), the provider transparently returns the OS bytes. The `health` field in `EntropyHealth` reflects per-provider status. Core operations are never blocked.

> **Tip:** Run `carnelian magic status` to see live provider availability and last-checked latency.

---

## Entropy Audit Log

When `log_entropy_events = true`, every entropy request is recorded in the `magic_entropy_log` table for compliance and debugging.

### Schema

| Column | Type | Description |
|--------|------|-------------|
| `log_id` | `BIGSERIAL` | Auto-incrementing log entry ID |
| `ts` | `TIMESTAMPTZ` | Request timestamp |
| `source` | `TEXT` | Provider used (`os`, `quantum-origin`, `quantinuum-h2`, `qiskit-rng`, `mixed`) |
| `bytes_requested` | `INTEGER` | Number of bytes requested |
| `quantum_available` | `BOOLEAN` | Whether quantum entropy was successfully retrieved |
| `latency_ms` | `INTEGER` | Request latency in milliseconds |
| `error` | `TEXT` | Error message if request failed |
| `correlation_id` | `UUID` | Request correlation ID for tracing |

### Query Examples

```sql
-- Recent entropy requests
SELECT ts, source, bytes_requested, quantum_available, latency_ms
FROM magic_entropy_log
ORDER BY ts DESC
LIMIT 20;

-- Quantum availability rate over last 24 hours
SELECT source,
       COUNT(*) AS total,
       COUNT(*) FILTER (WHERE quantum_available) AS quantum_ok
FROM magic_entropy_log
WHERE ts > NOW() - INTERVAL '24 hours'
GROUP BY source;
```

---

## `machine.toml` Reference

| Field | Type | Default | Env Override | Description |
|-------|------|---------|--------------|-------------|
| `enabled` | `bool` | `false` | `CARNELIAN_MAGIC_ENABLED` | Master switch for MAGIC subsystem |
| `quantum_origin_url` | `string` | `"https://origin.quantinuum.com"` | `CARNELIAN_QUANTUM_ORIGIN_URL` | Quantum Origin API base URL |
| `quantum_origin_api_key` | `string` | `""` | `CARNELIAN_QUANTUM_ORIGIN_API_KEY` | Quantinuum Quantum Origin API key |
| `quantinuum_enabled` | `bool` | `false` | `CARNELIAN_QUANTINUUM_ENABLED` | Enable Quantinuum H2 provider |
| `quantinuum_device` | `string` | `"H1-1E"` | `CARNELIAN_QUANTINUUM_DEVICE` | Quantinuum device name (`H1-1E`, `H2-1`, etc.) |
| `quantinuum_n_bits` | `u32` | `256` | `CARNELIAN_QUANTINUUM_N_BITS` | Number of bits to request from Quantinuum |
| `qiskit_enabled` | `bool` | `false` | `CARNELIAN_QISKIT_ENABLED` | Enable IBM Qiskit provider |
| `qiskit_backend` | `string` | `"ibm_brisbane"` | `CARNELIAN_QISKIT_BACKEND` | IBM Quantum backend name |
| `entropy_timeout_ms` | `u64` | `5000` | `CARNELIAN_ENTROPY_TIMEOUT_MS` | Timeout for entropy requests (milliseconds) |
| `entropy_mix_ratio` | `f64` | `0.5` | `CARNELIAN_ENTROPY_MIX_RATIO` | Fraction of bytes sourced from quantum provider (0.0-1.0) |
| `log_entropy_events` | `bool` | `true` | `CARNELIAN_LOG_ENTROPY_EVENTS` | Log all entropy requests to `magic_entropy_log` |
| `mantra_cooldown_beats` | `u32` | `3` | `CARNELIAN_MANTRA_COOLDOWN_BEATS` | Mantra cooldown in heartbeat cycles |

---

## Quantum Circuit Skills

Three Python skills in `skills/python-registry/` leverage quantum circuits for entropy generation and optimization. All follow the `execute(context) -> Dict` contract and raise `RuntimeError` on failure.

### quantinuum-h2-rng

Generates entropy using Hadamard circuits on Quantinuum H-series quantum computers via pytket.

**Installation:**
```bash
pip install pytket pytket-quantinuum
```

**Authentication:**
Requires prior authentication via `carnelian magic auth` to store Quantinuum tokens.

**`execute(context)` Contract:**

**Input keys:**
- `n_bits` (int, default 256) — Number of random bits to generate
- `device` (str, default `"H1-1E"`) — Quantinuum device identifier

**Output keys:**
- `bytes` (str) — Hex-encoded random bytes
- `bits` (str) — Raw bitstring
- `device` (str) — Device used for generation
- `n_bits` (int) — Number of bits generated

**Errors:**
Raises `RuntimeError` on circuit compilation or backend failure.

### qiskit-rng

Generates entropy using Hadamard circuits on IBM Quantum backends via Qiskit.

**Installation:**
```bash
pip install qiskit qiskit-ibm-runtime
```

**Authentication:**
Requires `IBM_QUANTUM_TOKEN` environment variable or `machine.toml` with `qiskit_enabled = true`.

**`execute(context)` Contract:**

**Input keys:**
- `shots` (int, default 2048) — Number of circuit shots (treated as n_bits)
- `backend_name` (str, default `"ibm_brisbane"`) — IBM Quantum backend identifier

**Output keys:**
- `bytes` (str) — Hex-encoded random bytes
- `bits` (str) — Raw bitstring
- `device` (str) — Backend used for generation

**Errors:**
Raises `RuntimeError` on circuit compilation or backend failure.

### quantum-optimize

Quantum-seeded simulated annealing optimizer for query plans and data-loading problems.

**Installation:**
```bash
pip install numpy
```

**`execute(context)` Contract:**

**Input keys:**
- `entropy_seed` (str or int, optional) — Quantum entropy seed (hex string or integer)
- `problem` (dict) — Problem specification containing:
  - `operations` (list, optional) — Operation identifiers to optimize
  - `steps` (int, default 500) — Number of annealing iterations
  - `temperature` (float, default 1.0) — Initial temperature
  - `cooling_rate` (float, default 0.995) — Temperature decay factor

**Output keys:**
- `optimized_plan` (list) — Optimized sequence of operations
- `index_order` (list) — Index ordering for debugging
- `cost_estimate` (float) — Final cost estimate
- `iterations` (int) — Number of iterations performed
- `quantum_seeded` (bool) — Whether quantum entropy was used

**Errors:**
Raises `RuntimeError` on optimization failure.

**Fallback Behavior:**
Falls back to non-deterministic numpy entropy when `entropy_seed` is absent.

---

## MAGIC UI Panel

The MAGIC panel provides a comprehensive interface for managing quantum entropy providers, mantras, and integration settings. Access it by launching the Carnelian desktop UI (`carnelian ui` or from the system tray), then clicking the **✨ MAGIC** tab in the top navigation bar.

**Requirements:**
- `magic.enabled = true` in `machine.toml`, or
- `CARNELIAN_MAGIC_ENABLED=true` environment variable

### Sub-tabs

| Sub-tab | Purpose |
|---------|---------|
| **Entropy Dashboard** | Live provider health cards, sample entropy, view audit log |
| **Mantra Library** | Browse categories, add/edit/disable mantra entries, view history |
| **Quantum Jobs** | Trigger circuit skill jobs, view job status |
| **Elixir & Skill Integration** | Toggle MAGIC entropy on per-elixir and per-skill basis, rehash elixir embeddings |
| **Auth Settings** | Set Quantum Origin API key, authenticate Quantinuum, set IBM Quantum token |

### Entropy Dashboard

Displays real-time health status for all configured entropy providers:
- **Quantum Origin** — ✅ Configured / ⚪ Not Configured
- **Quantinuum H2** — ✅ Authenticated (with expiry) / ⚪ Not Authenticated
- **Qiskit RNG** — ✅ Available / ⚪ Not Available
- **OS Random** — ✅ Always Available

Actions:
- **Request Entropy Sample (Quantum-First)** — Triggers entropy generation with quantum provider priority
- **View Entropy Log** — Displays recent entropy requests from `magic_entropy_log` table

### Mantra Library

Browse and manage mantra categories and entries:
- View all categories with entry counts and total weights
- Add new mantra entries with optional elixir linkage
- Edit existing entries (text, weight, enabled status)
- View mantra selection history (last 10 selections)
- Simulate mantra selection with current context

### Quantum Jobs

Trigger quantum circuit skill executions:
- Run `quantinuum-h2-rng` with configurable device and bit count
- Run `qiskit-rng` with configurable backend and shots
- Run `quantum-optimize` with problem specification
- View job results and execution logs

### Elixir & Skill Integration

Configure how MAGIC entropy integrates with elixirs and skills:
- Toggle quantum entropy for elixir embedding generation
- Rehash existing elixir embeddings with fresh quantum entropy
- View which skills receive automatic entropy seed injection
- Enable/disable per-skill entropy seeding

### Auth Settings

Manage authentication credentials for quantum providers:
- **Quantum Origin** — Set API key, test connection
- **Quantinuum** — Email/password authentication, view token expiry
- **IBM Quantum** — Enable Qiskit provider, test connection

---

## Mantra Library Management

The Mantra Library provides weighted, category-grouped prompt fragments that are injected into the agent's heartbeat context. Mantras are selected via `MantraTree::select_with_pool` using quantum entropy seeding to ensure non-deterministic selection patterns.

### Concept

Each mantra belongs to a **category** (e.g., `focus`, `creativity`, `caution`) and has:
- **Text** — The prompt fragment to inject
- **Weight** — Selection probability (higher = more likely)
- **Enabled** — Whether the mantra is active
- **Elixir ID** (optional) — Link to a specific elixir for context-aware selection

The `MantraTree` maintains a cooldown map to prevent the same category from firing repeatedly. The `mantra_cooldown_beats` configuration parameter (default 3) controls how many heartbeat cycles must pass before a category can be selected again.

### REST API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/v1/magic/mantras` | List all categories with entry counts and weights |
| `GET` | `/v1/magic/mantras/{category_id}` | List entries for a specific category |
| `POST` | `/v1/magic/mantras` | Add a new entry (`text`, optional `elixir_id`) |
| `PATCH` | `/v1/magic/mantras/{entry_id}` | Edit text, weight, or disable an entry |
| `GET` | `/v1/magic/mantras/history` | Last 10 rows from `mantra_history` |
| `POST` | `/v1/magic/mantras/simulate` | Dry-run `MantraTree::select_with_pool` with current context |

### Configuration

The `mantra_cooldown_beats` parameter in `machine.toml` controls category cooldown:

```toml
[magic]
mantra_cooldown_beats = 3  # Default: 3 heartbeat cycles
```

Higher values reduce mantra frequency but increase diversity. Lower values allow more frequent selections but risk repetitive context.

### Selection Algorithm

1. **Filter eligible categories** — Exclude categories in cooldown
2. **Quantum entropy seeding** — Generate seed from MAGIC entropy provider
3. **Weighted random selection** — Select category based on total weight
4. **Entry selection** — Choose random entry from selected category
5. **Update cooldown** — Mark category as used for N beats
6. **Log to history** — Record selection in `mantra_history` table

### Example Usage

**List all categories:**
```bash
curl http://localhost:8080/v1/magic/mantras
```

**Add new mantra:**
```bash
curl -X POST http://localhost:8080/v1/magic/mantras \
  -H "Content-Type: application/json" \
  -d '{
    "category": "focus",
    "text": "Prioritize clarity and precision in your reasoning",
    "weight": 10
  }'
```

**Simulate selection:**
```bash
curl -X POST http://localhost:8080/v1/magic/mantras/simulate \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "current_task": "code_review",
      "complexity": "high"
    }
  }'
```

---

## Elixir & Skill Integration

MAGIC entropy can be integrated with the Elixir and Skill subsystems to provide quantum-seeded randomness for embedding generation and skill execution.

### Per-Elixir Entropy

When MAGIC is enabled, elixirs can source their embedding salt from quantum entropy instead of OS random. This provides:
- **Non-deterministic embeddings** — Each elixir gets unique quantum-seeded embeddings
- **Enhanced security** — Quantum entropy is cryptographically stronger than pseudo-random generators
- **Auditability** — All entropy requests are logged to `magic_entropy_log`

**Rehash Existing Elixirs:**

Trigger a rehash of all active elixir embeddings with fresh quantum entropy:

```bash
curl -X POST http://localhost:8080/v1/magic/elixirs/rehash
```

Response:
```json
{
  "rehashed": 42,
  "message": "Rehashed 42 elixirs with fresh entropy"
}
```

The rehash operation:
1. Fetches all active elixirs from the database
2. Generates 32 bytes of quantum entropy per elixir
3. Updates the `quantum_hash` column with hex-encoded entropy
4. Updates the `updated_at` timestamp

### Per-Skill Entropy Seed

Skills that support entropy seeding (e.g., `quantum-optimize`) automatically receive the current MAGIC entropy sample when `magic.enabled = true`. The orchestrator injects the entropy seed into the skill's execution context.

**Skill Context Injection:**

When a skill is executed, the orchestrator checks:
1. Is MAGIC enabled? (`magic.enabled = true`)
2. Does the skill accept `entropy_seed` in its context?
3. Is a quantum provider available?

If all conditions are met, the orchestrator:
1. Requests entropy from the MAGIC provider
2. Converts the entropy to a hex string
3. Injects `entropy_seed` into the skill's `context` dict
4. Logs the entropy request to `magic_entropy_log`

**Example Skill Context:**
```json
{
  "entropy_seed": "a3f5b2c8d1e9f4a7b6c3d2e1f8a5b4c7",
  "problem": {
    "operations": ["load_data", "transform", "aggregate", "export"],
    "steps": 1000,
    "temperature": 1.5,
    "cooling_rate": 0.99
  }
}
```

### Integration Toggles

Control MAGIC integration via the configuration API:

**Get current configuration:**
```bash
curl http://localhost:8080/v1/magic/config
```

**Update configuration:**
```bash
curl -X PATCH http://localhost:8080/v1/magic/config \
  -H "Content-Type: application/json" \
  -d '{
    "quantum_origin_api_key": "your-key-here",
    "quantinuum_enabled": true,
    "qiskit_enabled": false
  }'
```

**UI Access:**

The **Elixir & Skill Integration** sub-tab in the MAGIC UI panel provides:
- Toggle switches for per-elixir entropy
- Rehash button for existing elixirs
- List of skills that support entropy seeding
- Per-skill entropy toggle (future feature)
- Real-time integration status

### Configuration Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `magic.enabled` | `bool` | `false` | Master toggle for MAGIC subsystem |
| `log_entropy_events` | `bool` | `true` | Log all entropy requests to database |
| `entropy_timeout_ms` | `u64` | `5000` | Timeout for entropy provider requests |
| `entropy_mix_ratio` | `f64` | `0.5` | Fraction of bytes from quantum provider (0.0-1.0) |

### Security Considerations

- **API Key Storage** — Quantum Origin API keys are stored in `machine.toml` or environment variables, never in the database
- **Token Expiry** — Quantinuum tokens expire after 24 hours and must be refreshed via `carnelian magic auth --refresh`
- **Entropy Logging** — All entropy requests are logged with timestamps, sources, and byte counts for audit purposes
- **Fallback Safety** — If all quantum providers fail, the system falls back to OS random to prevent service disruption
