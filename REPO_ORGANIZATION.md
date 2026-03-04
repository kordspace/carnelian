# Repository Organization Analysis

## Current Structure

The CARNELIAN repository currently has an inconsistent organization for TypeScript/JavaScript packages:

### Current Layout:
```
CARNELIAN/
├── packages/
│   └── mcp-server/          # MCP server for Windsurf IDE integration
├── gateway/                 # LLM gateway service (TypeScript)
├── workers/
│   ├── node-worker/         # Node.js skill execution worker
│   ├── python-worker/       # Python skill execution worker
│   └── shell-worker/        # Shell script worker
├── crates/                  # Rust workspace (well-organized)
└── skills/                  # Skill definitions
```

## Issues Identified

1. **Inconsistent Package Organization**: Only `mcp-server` is in `packages/`, while `gateway` and `node-worker` are at different levels
2. **Mixed Concerns**: `gateway` is a standalone directory but serves similar purpose to workers
3. **Unclear Hierarchy**: Not immediately clear which TypeScript/JavaScript projects are related

## Recommended Organization

### Option 1: Consolidate All TypeScript Projects in `packages/`
```
CARNELIAN/
├── packages/
│   ├── mcp-server/          # MCP server for Windsurf IDE
│   ├── gateway/             # LLM gateway service
│   └── node-worker/         # Node.js skill worker
├── workers/
│   ├── python-worker/       # Python worker (stays here)
│   └── shell-worker/        # Shell worker (stays here)
├── crates/                  # Rust workspace
└── skills/                  # Skill definitions
```

**Pros:**
- All TypeScript/JavaScript projects in one place
- Clear separation by language/runtime
- Follows monorepo conventions (packages/ for npm packages)

**Cons:**
- `node-worker` separated from other workers
- Breaks conceptual grouping of workers

### Option 2: Keep Workers Together, Move Gateway to Packages
```
CARNELIAN/
├── packages/
│   ├── mcp-server/          # MCP server for Windsurf IDE
│   └── gateway/             # LLM gateway service
├── workers/
│   ├── node-worker/         # Node.js worker
│   ├── python-worker/       # Python worker
│   └── shell-worker/        # Shell worker
├── crates/                  # Rust workspace
└── skills/                  # Skill definitions
```

**Pros:**
- Workers stay conceptually grouped
- Gateway clearly separated as infrastructure service
- MCP server and gateway both in packages/ (both are services, not workers)

**Cons:**
- `packages/` becomes less clear (contains different types of services)

### Option 3: Current Structure with Documentation (Minimal Change)
Keep current structure but add clear documentation explaining:
- `packages/` = External integrations and standalone services
- `gateway/` = Core infrastructure service (could move to packages/)
- `workers/` = Skill execution runtimes

**Pros:**
- No breaking changes
- Minimal refactoring needed

**Cons:**
- Inconsistency remains
- Less intuitive for new contributors

## Recommendation

**Option 2** is recommended because:

1. **Clear Separation of Concerns**:
   - `packages/` = Standalone services (MCP server, Gateway)
   - `workers/` = Skill execution runtimes (all together)
   - `crates/` = Rust core and libraries

2. **Minimal Disruption**: Only requires moving `gateway/` to `packages/gateway/`

3. **Logical Grouping**: Workers are conceptually related and should stay together

4. **Future-Proof**: Easy to add new services to `packages/` or new workers to `workers/`

## Migration Steps (If Adopting Option 2)

1. Move `gateway/` to `packages/gateway/`
2. Update all references in:
   - `docker-compose.yml`
   - CI/CD workflows (`.github/workflows/`)
   - Documentation files
   - Build scripts
3. Update package.json name to `@carnelian/gateway` (already done)
4. Add workspace configuration if using npm/pnpm workspaces

## Current Package Scoping

All packages already use `@carnelian/` scope:
- ✅ `@carnelian/mcp-server`
- ✅ `@carnelian/gateway`
- ✅ `@carnelian/node-worker`

This is good practice and should be maintained.

## Additional Recommendations

1. **Add Root package.json**: Consider adding a root `package.json` with workspace configuration:
   ```json
   {
     "name": "carnelian-monorepo",
     "private": true,
     "workspaces": [
       "packages/*",
       "workers/node-worker"
     ]
   }
   ```

2. **Consistent Build Scripts**: Ensure all TypeScript projects have consistent:
   - `build` script
   - `dev` script
   - `test` script
   - `start` script

3. **Shared TypeScript Config**: Consider a shared `tsconfig.base.json` at root

4. **Documentation**: Update `CONTRIBUTING.md` to explain the repository structure clearly
