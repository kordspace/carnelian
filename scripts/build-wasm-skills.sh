#!/bin/bash
# 🔥 Carnelian WASM Skills Builder
# Builds all WASM skills in the registry for deployment

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTRY_DIR="${PROJECT_ROOT}/skills/registry"
TARGET="wasm32-wasip1"

echo -e "${BLUE}🔥 Carnelian WASM Skills Builder${NC}"
echo "=========================================="
echo ""

# Ensure the WASM target is installed
echo -e "${YELLOW}Ensuring ${TARGET} target is installed...${NC}"
rustup target add "${TARGET}"
echo ""

# Counters
BUILT=0
FAILED=0
SKIPPED=0

# Iterate over skill directories
for skill_dir in "${REGISTRY_DIR}"/*/; do
    skill_name=$(basename "${skill_dir}")
    
    # Skip if no Cargo.toml (not a Rust project)
    if [[ ! -f "${skill_dir}Cargo.toml" ]]; then
        continue
    fi
    
    # Skip if no skill.json (not a skill)
    if [[ ! -f "${skill_dir}skill.json" ]]; then
        continue
    fi
    
    # Extract runtime from skill.json
    runtime=$(grep -oP '"runtime"\s*:\s*"\K[^"]+' "${skill_dir}skill.json" 2>/dev/null || echo "")
    
    # Skip if not a WASM skill
    if [[ "${runtime}" != "wasm" ]]; then
        ((SKIPPED++))
        continue
    fi
    
    echo -e "${BLUE}Building ${skill_name}...${NC}"
    
    # Build the skill
    if cargo build --manifest-path "${skill_dir}Cargo.toml" --target "${TARGET}" --release; then
        # Binary name matches skill name (Cargo preserves hyphens in WASM output)
        binary_name="${skill_name}.wasm"
        source_path="${skill_dir}target/${TARGET}/release/${binary_name}"
        dest_path="${skill_dir}${skill_name}.wasm"
        
        # Copy the binary to the skill root
        if [[ -f "${source_path}" ]]; then
            cp "${source_path}" "${dest_path}"
            echo -e "${GREEN}✓ Built ${skill_name} → ${skill_name}.wasm${NC}"
            ((BUILT++))
        else
            echo -e "${RED}✗ Binary not found at ${source_path}${NC}"
            ((FAILED++))
        fi
    else
        echo -e "${RED}✗ Failed to build ${skill_name}${NC}"
        ((FAILED++))
    fi
    
    echo ""
done

# Summary
echo "=========================================="
echo -e "${GREEN}Built: ${BUILT}${NC}"
echo -e "${YELLOW}Skipped: ${SKIPPED}${NC}"
echo -e "${RED}Failed: ${FAILED}${NC}"
echo ""

if [[ ${FAILED} -gt 0 ]]; then
    echo -e "${RED}Some skills failed to build!${NC}"
    exit 1
else
    echo -e "${GREEN}All WASM skills built successfully!${NC}"
    exit 0
fi
