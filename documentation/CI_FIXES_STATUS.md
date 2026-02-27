# CARNELIAN CI Fixes Status

## Completed Fixes ✅

1. **Duplicate Function** - Removed duplicate `carnelian_key_auth` function (lines 6830-6870)
2. **Axum 0.8 API** - Fixed `carnelian_key_auth` signature (removed generic B parameter)
3. **Unknown Lints** - Fixed `clippy::unused_imports` → `unused_imports`
4. **Unused Imports** - Removed unused imports from `elixir.rs` and `server.rs`
5. **Wasmtime API** - Updated wasmtime-wasi imports (removed p1/p2 modules)
6. **WasmState** - Fixed to use `WasiCtx` instead of `WasiP1Ctx`
7. **async_trait** - Added import to `worker.rs`
8. **SkillInput** - Fixed struct usage (correct fields: action, params, identity_id, correlation_id)

## Remaining Critical Errors ⚠️

### 1. Error::Permission Variant Missing
**Location:** `worker.rs` (multiple instances)
**Error:** `no variant or associated item named 'Permission' found for enum 'carnelian_common::Error'`
**Fix Required:** Add `Permission` variant to `Error` enum in `carnelian-common/src/error.rs`

### 2. sqlx::Row Import Missing
**Location:** `server.rs:7017-7026`
**Error:** `no method named 'get' found for reference '&sqlx::sqlx_postgres::PgRow'`
**Fix Required:** Add `use sqlx::Row;` to imports

### 3. Result Type Mismatch
**Location:** `wasm_runtime.rs:231`
**Error:** `type alias takes 1 generic argument but 2 generic arguments were supplied`
**Fix Required:** Change `Result<(), Error>` to `Result<()>`

### 4. SkillOutput Type Mismatch
**Location:** `wasm_runtime.rs:277, 279, 287, 289`
**Error:** `expected HashMap<String, String>, found Option<Value>`
**Fix Required:** Fix return type or conversion logic

### 5. OsStr Conversion
**Location:** `worker.rs:1719`
**Error:** `expected &[u8], found &OsStr`
**Fix Required:** Use `disk.file_system().to_string_lossy()` instead

### 6. async_trait Lifetime Issues
**Location:** `worker.rs:810, 924, 931, 938, 947, 1030, 1868, 1875, 1881, 1890`
**Error:** `lifetime parameters or bounds on method do not match the trait declaration`
**Fix Required:** Review trait definition and ensure implementations match

## Next Steps

1. Add `Permission` variant to Error enum
2. Add sqlx::Row import
3. Fix Result type aliases
4. Fix SkillOutput type conversions
5. Fix OsStr to string conversion
6. Review and fix async_trait lifetime issues
7. Rename skills/registry → skills/wasm-registry
8. Move dev docs to ../DOCUMENTATION/
9. Perform deep CARNELIAN vs OPENCLAW comparison

## Status

**Partial Fix Committed:** 8e3289d
**Build Status:** Still failing (major issues resolved, remaining errors need attention)
**Priority:** High - CI must pass before production deployment
