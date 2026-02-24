#!/bin/bash
# 🔥 Carnelian OS Packaging Script
# Builds Carnelian for release and bundles it for distribution

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="${PROJECT_ROOT}/target/release"
PACKAGE_NAME="carnelian-os"
VERSION=$(grep -oP 'version = "\K[^"]+' "${PROJECT_ROOT}/Cargo.toml" | head -1)

echo -e "${BLUE}🔥 Carnelian OS Packager v${VERSION}${NC}"
echo "=========================================="

# Detect OS
OS=$(uname -s)
ARCH=$(uname -m)
TARGET="${PACKAGE_NAME}-${VERSION}-${OS}-${ARCH}"

echo -e "${YELLOW}Target: ${TARGET}${NC}"
echo ""

# Clean previous build
echo -e "${YELLOW}→ Cleaning previous build...${NC}"
cargo clean -q 2>/dev/null || true

# Build release binaries
echo -e "${YELLOW}→ Building carnelian-core...${NC}"
cargo build --release -p carnelian-core --bin carnelian --features cli

echo -e "${YELLOW}→ Building carnelian-ui...${NC}"
cargo build --release -p carnelian-ui --bin carnelian-ui --features embedded 2>/dev/null || {
    echo -e "${YELLOW}  (UI build skipped - carnelian-ui not found)${NC}"
}

echo ""

# Create staging directory
echo -e "${YELLOW}→ Creating package structure...${NC}"
STAGING_DIR="${PROJECT_ROOT}/target/${TARGET}"
mkdir -p "${STAGING_DIR}/bin"
mkdir -p "${STAGING_DIR}/lib"
mkdir -p "${STAGING_DIR}/profiles"

# Copy binaries
echo -e "${YELLOW}→ Copying binaries...${NC}"
cp "${RELEASE_DIR}/carnelian" "${STAGING_DIR}/bin/" 2>/dev/null || {
    echo -e "${RED}✗ carnelian binary not found${NC}"
    exit 1
}

if [ -f "${RELEASE_DIR}/carnelian-ui" ]; then
    cp "${RELEASE_DIR}/carnelian-ui" "${STAGING_DIR}/bin/"
    echo -e "${GREEN}  ✓ carnelian-ui${NC}"
fi
echo -e "${GREEN}  ✓ carnelian${NC}"

# Copy profiles
echo -e "${YELLOW}→ Copying machine profiles...${NC}"
cp "${PROJECT_ROOT}/machine-profiles/"*.toml "${STAGING_DIR}/profiles/" 2>/dev/null || {
    echo -e "${YELLOW}  (No profiles found)${NC}"
}

# Copy Docker files
echo -e "${YELLOW}→ Copying Docker files...${NC}"
cp "${PROJECT_ROOT}/docker-compose.yml" "${STAGING_DIR}/" 2>/dev/null || true
cp "${PROJECT_ROOT}/docker-compose."*.yml "${STAGING_DIR}/" 2>/dev/null || true

# Copy README
echo -e "${YELLOW}→ Copying documentation...${NC}"
cp "${PROJECT_ROOT}/README.md" "${STAGING_DIR}/" 2>/dev/null || true
cp "${PROJECT_ROOT}/LICENSE" "${STAGING_DIR}/" 2>/dev/null || true

# Copy docs directory
if [ -d "${PROJECT_ROOT}/docs" ]; then
    echo -e "${YELLOW}→ Copying docs directory...${NC}"
    cp -r "${PROJECT_ROOT}/docs" "${STAGING_DIR}/" 2>/dev/null || true
fi

# Create install script
echo -e "${YELLOW}→ Creating install script...${NC}"
cat > "${STAGING_DIR}/install.sh" << 'EOF'
#!/bin/bash
# Carnelian OS Installer

set -e

INSTALL_DIR="${INSTALL_DIR:-/usr/local}"
BIN_DIR="${INSTALL_DIR}/bin"
PROFILE_DIR="${HOME}/.carnelian/profiles"

echo "🔥 Installing Carnelian OS..."

# Check for sudo if needed
if [[ "$INSTALL_DIR" == "/usr"* ]] && [[ $EUID -ne 0 ]]; then
    echo "This script requires sudo. Run with sudo or set INSTALL_DIR"
    exit 1
fi

# Create directories
mkdir -p "${BIN_DIR}"
mkdir -p "${PROFILE_DIR}"

# Copy binaries
cp "bin/carnelian" "${BIN_DIR}/"
chmod +x "${BIN_DIR}/carnelian"

if [ -f "bin/carnelian-ui" ]; then
    cp "bin/carnelian-ui" "${BIN_DIR}/"
    chmod +x "${BIN_DIR}/carnelian-ui"
fi

# Copy profiles
cp profiles/*.toml "${PROFILE_DIR}/" 2>/dev/null || true

echo "✓ Carnelian OS installed to ${BIN_DIR}"
echo ""
echo "Next steps:"
echo "  1. Run: carnelian init"
echo "  2. Start: carnelian start"
EOF
chmod +x "${STAGING_DIR}/install.sh"

# Create uninstall script
echo -e "${YELLOW}→ Creating uninstall script...${NC}"
cat > "${STAGING_DIR}/uninstall.sh" << 'EOF'
#!/bin/bash
# Carnelian OS Uninstaller

INSTALL_DIR="${INSTALL_DIR:-/usr/local}"
BIN_DIR="${INSTALL_DIR}/bin"

echo "🔥 Uninstalling Carnelian OS..."

rm -f "${BIN_DIR}/carnelian"
rm -f "${BIN_DIR}/carnelian-ui"

echo "✓ Carnelian OS uninstalled"
EOF
chmod +x "${STAGING_DIR}/uninstall.sh"

# Create archive
echo ""
echo -e "${YELLOW}→ Creating distribution archive...${NC}"

ARCHIVE_NAME="${TARGET}.tar.gz"
cd "${PROJECT_ROOT}/target"
tar -czf "${ARCHIVE_NAME}" "${TARGET}"

# Generate SHA256 checksum
echo -e "${YELLOW}→ Generating SHA256 checksum...${NC}"
if command -v sha256sum &> /dev/null; then
    sha256sum "${ARCHIVE_NAME}" > "${ARCHIVE_NAME}.sha256"
    CHECKSUM=$(sha256sum "${ARCHIVE_NAME}" | awk '{print $1}')
else
    # macOS fallback
    shasum -a 256 "${ARCHIVE_NAME}" > "${ARCHIVE_NAME}.sha256"
    CHECKSUM=$(shasum -a 256 "${ARCHIVE_NAME}" | awk '{print $1}')
fi
echo "  Checksum: ${CHECKSUM}"
echo "  Verify with: sha256sum -c ${ARCHIVE_NAME}.sha256"

echo ""
echo -e "${GREEN}==========================================${NC}"
echo -e "${GREEN}✓ Package created: target/${ARCHIVE_NAME}${NC}"
echo -e "${GREEN}==========================================${NC}"
echo ""
echo "Contents:"
ls -lh "${STAGING_DIR}/bin/"
echo ""
echo -e "Install with: tar -xzf ${ARCHIVE_NAME} && cd ${TARGET} && sudo ./install.sh"
