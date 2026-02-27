# Skill Organization Summary

## Current Status (As of latest analysis)

### Directory Structure
```
skills/
├── node-registry/     434 skills → 368 after duplicate removal
├── core-registry/     231 skills (renamed from registry)
├── python-registry/   25 skills
└──
    Total: 690 files
    Unique: 624 skills (66 duplicates)
    Target: 698 skills
    Missing: 74 skills
```

### Completed Work

✅ **Code References Updated**
- `crates/carnelian-core/src/config.rs` - Updated default path
- `crates/carnelian-core/src/worker.rs` - Updated WASM path
- `crates/carnelian-core/src/skills.rs` - Updated documentation
- `crates/carnelian-bin/src/main.rs` - Updated path
- `crates/carnelian-core/src/bin/carnelian.rs` - Updated path
- `crates/carnelian-core/tests/wasm_skill_tests.rs` - Updated test path

✅ **Directory Renamed**
- `skills/registry/` → `skills/core-registry/`
- Updated README with comprehensive documentation

✅ **Duplicate Analysis Complete**
Identified 66 duplicate skills between node-registry and core-registry

### Next Steps (Manual)

#### 1. Remove Duplicate Skills (66 skills)

Run the removal script:
```bash
cd CARNELIAN
./scripts/remove-duplicate-skills.sh
```

Or manually remove these directories from `skills/node-registry/`:
- agents-list, agent-step
- browser-automation, browser-click, browser-navigate, browser-pdf, browser-screenshot, browser-type
- canvas-render, cascade-run, code-format
- cron-add, cron-list, cron-remove, cron-run, cron-schedule
- discord-send, disk-usage, docker-exec, docker-logs, docker-stats
- email-send, env-get
- file-delete, file-move, file-search, file-write
- gateway-config, gateway-query, gateway-restart, gateway-update
- git-branch, git-commit, git-diff, git-log
- hash-file, http-request
- image-analyze, image-generate
- json-transform
- markdown-parse
- memory-read, memory-write, message-send, model-usage
- network-stats, nodes-list
- openai-image-gen
- process-list
- session-history, session-list, session-spawn, session-status
- slack-channel, slack-file, slack-react, slack-send
- telegram-group, telegram-media, telegram-send
- text-search, text-to-speech
- web-fetch, web-search, whatsapp-send
- yaml-parse

#### 2. Find Missing Skills (74 skills needed)

Based on git history, commit e611379 claimed 698 skills:
- Node.js: 433
- WASM/Rust: 230 (now core-registry: 231)
- Python: 25
- Native: 10

Current count: 624 unique
Missing: 74 skills

Search git history for removed skills:
```bash
git log --all --full-history --diff-filter=D --name-only -- 'skills/**/*' | grep -E '^skills/(node|core|python)-registry/' | sort | uniq
```

#### 3. Commit All Changes

After resolving git lock:
```bash
cd CARNELIAN
git add -A
git commit -m "refactor: Organize skills and remove duplicates

- Renamed registry to core-registry
- Updated all code references
- Removed 66 duplicate skills from node-registry
- Restored X missing skills
- Total: 698 skills organized"
git push origin main
```

### Registry Purposes

| Registry | Purpose | Runtime | Count |
|----------|---------|---------|-------|
| **core-registry** | Core system utilities, algorithms | WASM/Rust/Node/Python | 231 |
| **node-registry** | Platform integrations (Stripe, AWS, etc.) | Node.js | 368 (after dedup) |
| **python-registry** | ML/Data Science | Python | 25 |

### Key Points

1. **Duplicates are intentional** - core-registry has optimized WASM versions, node-registry has full-featured Node versions
2. **Favor WASM** - For performance-critical skills, use core-registry WASM implementations
3. **Integration skills** - Platform-specific skills (APIs, services) go in node-registry

### Scripts Created

- `scripts/remove-duplicate-skills.sh` - Removes 66 duplicate skills

### Files Modified

- `crates/carnelian-core/src/config.rs`
- `crates/carnelian-core/src/worker.rs`
- `crates/carnelian-core/src/skills.rs`
- `crates/carnelian-bin/src/main.rs`
- `crates/carnelian-core/src/bin/carnelian.rs`
- `crates/carnelian-core/tests/wasm_skill_tests.rs`
- `skills/core-registry/README.md`

---

**Status**: Code references updated, directory renamed, duplicate analysis complete
**Pending**: Remove duplicates (manual), find missing skills, commit changes
