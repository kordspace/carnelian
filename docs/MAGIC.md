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
