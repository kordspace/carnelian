# 🔥 Carnelian OS Demo Script — 5-Minute Walkthrough

This script provides a timestamped walkthrough of Carnelian OS features for demos and onboarding.

**Prerequisites:**
- Carnelian OS installed (`cargo install --path crates/carnelian-core`)
- Docker running (for PostgreSQL)
- Node.js 18+ (for UI development)

---

## 0:00–0:30 — Initialization Wizard

**Scene:** Terminal

```bash
# Start the initialization wizard
carnelian init
```

**Actions:**
1. Select machine profile: `urim` (optimized for desktop development)
2. Database URL: accept default `postgresql://carnelian:carnelian@localhost:5432/carnelian`
3. Ollama URL: accept default `http://localhost:11434`
4. HTTP port: accept default `18789`
5. Workspace paths: accept default `.`
6. Generate new owner keypair: `Y`

**Expected Output:**
```
✓ Generated new owner keypair
  Public key (hex): a1b2c3d4...
  Private key file: /home/user/.carnelian/owner.key
✓ Wrote machine.toml
```

---

## 0:30–1:00 — Start Server & Health Check

**Scene:** Terminal

```bash
# Start the server
carnelian start
```

**Actions:**
1. Wait for "🔥 Carnelian server listening on 0.0.0.0:18789"
2. Open second terminal for health check

```bash
# Check health
curl http://localhost:18789/v1/health
```

**Expected Output:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "database": "connected"
}
```

---

## 1:00–1:45 — UI Dashboard & Task Creation

**Scene:** Browser at `http://localhost:3000` (Dioxus UI)

**Actions:**
1. Navigate to "Tasks" tab
2. Click "Create Task" button
3. Fill in:
   - Title: "Demo Task - File Analysis"
   - Description: "Analyze the project structure and report on file types"
   - Priority: 3 (Medium)
   - Requires Approval: No
4. Click "Create"

**Expected Output:**
- Task appears in list with state "pending"
- Real-time event appears in "Events" stream

---

## 1:45–2:30 — XP Progression & Leaderboard

**Scene:** UI Dashboard

**Actions:**
1. Navigate to "XP" tab
2. Show:
   - Agent XP progress bars
   - Skill metrics with heatmap
   - Leaderboard showing top agents

**Key Points:**
- XP accrues automatically from task completion
- Skills level up at thresholds (1000 XP = Level 1)
- Multipliers based on task complexity and risk

---

## 2:30–3:15 — Voice Gateway Configuration

**Scene:** UI Settings

**Actions:**
1. Navigate to "Settings" → "Voice"
2. Configure ElevenLabs:
   - API Key: (paste test key)
   - Agent ID: (optional, for voice clone)
3. Click "Test TTS" — hear sample audio
4. Click "Test STT" — speak into microphone, verify transcription

**Expected Output:**
```
✓ Voice configuration saved
✓ TTS test successful
✓ STT test successful (confidence: 0.94)
```

---

## 3:15–4:00 — Capability Grants & Approval Queue

**Scene:** UI Admin Panel

**Actions:**
1. Navigate to "Capabilities" tab
2. Show existing grants
3. Create new grant:
   - Subject Type: "identity"
   - Subject ID: (select agent)
   - Capability: "fs.write"
   - Scope: `{"paths": ["/tmp/demo"]}`
4. If safe mode is enabled, show approval queue
5. Approve pending request

**Key Points:**
- Granular permissions with JSON scope
- Approval queue for sensitive operations
- Safe mode blocks side effects

---

## 4:00–4:30 — Migration from Thummim

**Scene:** Terminal

```bash
# Import from Thummim database
carnelian migrate-from-thummim \
  --thummim-db "postgresql://thummim@localhost/thummim" \
  --dry-run
```

**Actions:**
1. Review dry-run report:
   - Skills to migrate: 12
   - Tasks to migrate: 45
   - Identity mapping: Thummim:1 → Carnelian:new_uuid
2. Execute migration (remove `--dry-run`)

**Expected Output:**
```
✓ Migration complete
  Imported: 12 skills, 45 tasks, 3 identities
  Duration: 2.3s
```

---

## 4:30–5:00 — Key Rotation & Ledger Verification

**Scene:** Terminal

```bash
# Rotate owner keypair
carnelian key rotate
```

**Actions:**
1. Confirm rotation
2. Show old and new public keys
3. Demonstrate ledger verification

```bash
# Verify ledger integrity
curl http://localhost:18789/v1/ledger/verify
```

**Expected Output:**
```
🔥 Key rotation completed
   Old public key: a1b2c3d4...
   New public key: e5f6g7h8...
   Rotation signature: 0x...
   New key file: /home/user/.carnelian/owner.key.new

{"verified": true, "events": 127, "last_event_id": 127}
```

---

## Summary

**Covered Features:**
1. ✅ Interactive setup wizard
2. ✅ Health monitoring
3. ✅ Task creation and execution
4. ✅ XP system and leaderboards
5. ✅ Voice gateway (TTS/STT)
6. ✅ Capability-based security
7. ✅ Approval workflows
8. ✅ Migration from Thummim
9. ✅ Cryptographic key rotation
10. ✅ Ledger integrity verification

**Next Steps:**
- Explore API documentation at `http://localhost:18789/docs`
- Review architecture at `docs/ARCHITECTURE.md`
- Run validation tests: `cargo test --test checkpoint3_validation_test -- --ignored`

---

*Built with 🔥 by the Carnelian OS team*
