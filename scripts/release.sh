#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$SCRIPT_DIR/.env"

# ── Colors ──────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# ── Load .env ───────────────────────────────────────────────────────
if [[ -f "$ENV_FILE" ]]; then
  info "Loading environment from $ENV_FILE"
  set -a
  source "$ENV_FILE"
  set +a
else
  warn "No .env file found at $ENV_FILE"
  cat <<'TEMPLATE'

Create scripts/.env with the following variables:

  APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
  APPLE_ID="your@apple.id"
  APPLE_PASSWORD="app-specific-password"
  APPLE_TEAM_ID="YOURTEAMID"
  TAURI_SIGNING_PRIVATE_KEY="updater private key"
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="updater key password"

Pull these values from the GitHub repo secrets settings.
TEMPLATE
  error "Please create scripts/.env before running this script."
fi

# ── Validate env vars ───────────────────────────────────────────────
REQUIRED_VARS=(APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID)
for var in "${REQUIRED_VARS[@]}"; do
  [[ -z "${!var:-}" ]] && error "Missing required env var: $var (set it in scripts/.env)"
done

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  warn "TAURI_SIGNING_PRIVATE_KEY not set — updater .sig file will not be generated"
fi

# ── Validate tools ──────────────────────────────────────────────────
for cmd in rustc cargo node npm gh curl tar codesign; do
  command -v "$cmd" &>/dev/null || error "'$cmd' is not installed or not in PATH"
done

# ── Version argument ────────────────────────────────────────────────
VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  read -rp "Enter release version (e.g. v1.0.1): " VERSION
fi
[[ "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || error "Version must match vX.Y.Z (got: $VERSION)"

info "Building Chirp $VERSION"

# ── Install frontend deps ──────────────────────────────────────────
cd "$PROJECT_DIR"
info "Installing frontend dependencies..."
npm ci

# ── Download sherpa-onnx dylibs ─────────────────────────────────────
SHERPA_VERSION="1.12.29"
DYLIB_DIR="src-tauri/sherpa-onnx-lib/macos"

if ls "$DYLIB_DIR"/*.dylib &>/dev/null; then
  info "sherpa-onnx dylibs already present, skipping download"
else
  info "Downloading sherpa-onnx v${SHERPA_VERSION} for macOS..."
  ARCHIVE="sherpa-onnx-v${SHERPA_VERSION}-osx-universal2-shared.tar.bz2"
  URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${SHERPA_VERSION}/${ARCHIVE}"

  curl -fSL "$URL" -o "$ARCHIVE"
  tar xjf "$ARCHIVE"

  mkdir -p "$DYLIB_DIR"
  cp "sherpa-onnx-v${SHERPA_VERSION}-osx-universal2-shared/lib/"*.dylib "$DYLIB_DIR/"

  rm -rf "$ARCHIVE" "sherpa-onnx-v${SHERPA_VERSION}-osx-universal2-shared"
  info "dylibs downloaded to $DYLIB_DIR"
fi

ls -la "$DYLIB_DIR/"*.dylib

# ── Unlock keychain for codesigning ─────────────────────────────────
info "Unlocking keychain for codesigning..."
read -rsp "Enter your Mac login password (for keychain access): " KEYCHAIN_PASSWORD
echo
security unlock-keychain -p "$KEYCHAIN_PASSWORD" ~/Library/Keychains/login.keychain-db
security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASSWORD" ~/Library/Keychains/login.keychain-db
unset KEYCHAIN_PASSWORD

# ── Codesign dylibs ────────────────────────────────────────────────
info "Codesigning dylibs..."
for dylib in "$DYLIB_DIR"/*.dylib; do
  codesign --force --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$dylib"
  echo "  Signed: $(basename "$dylib")"
done

# ── Build Tauri app ─────────────────────────────────────────────────
info "Building Tauri app (target: aarch64-apple-darwin)..."
export APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-}"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

npm run tauri build -- --target aarch64-apple-darwin

# ── Locate build artifacts ──────────────────────────────────────────
DMG_DIR="src-tauri/target/aarch64-apple-darwin/release/bundle/dmg"
DMG_FILE=$(ls "$DMG_DIR"/*.dmg 2>/dev/null | head -1)
[[ -z "$DMG_FILE" ]] && error "No .dmg found in $DMG_DIR"

info "Built: $DMG_FILE"

# Collect updater sig if present
UPLOAD_FILES=("$DMG_FILE")
UPDATE_DIR="src-tauri/target/aarch64-apple-darwin/release/bundle/macos"
if ls "$UPDATE_DIR"/*.tar.gz &>/dev/null; then
  UPLOAD_FILES+=("$UPDATE_DIR"/*.tar.gz)
fi
if ls "$UPDATE_DIR"/*.tar.gz.sig &>/dev/null; then
  UPLOAD_FILES+=("$UPDATE_DIR"/*.tar.gz.sig)
fi

# ── Create GitHub release ───────────────────────────────────────────
info "Creating GitHub release $VERSION..."
gh release create "$VERSION" \
  --title "Chirp $VERSION" \
  --notes "See the assets below to download and install Chirp." \
  --draft \
  "${UPLOAD_FILES[@]}"

info "Release $VERSION created (draft). Review and publish at:"
gh release view "$VERSION" --json url -q '.url'
