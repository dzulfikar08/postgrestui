#!/bin/bash
set -euo pipefail

# ============================================================
# PostgresTUI — Local Release Script
# ============================================================
# Builds the CLI binary and web frontend locally, then uploads
# to Cloudflare R2 + records the release in D1.
#
# Usage:
#   ./scripts/release-local.sh                          # Build all + upload
#   ./scripts/release-local.sh --skip-upload            # Build only
#   ./scripts/release-local.sh --target aarch64         # macOS ARM64 only
#   ./scripts/release-local.sh --target x86_64          # macOS Intel only
#   ./scripts/release-local.sh --target linux           # Linux x86_64 only
#   ./scripts/release-local.sh --target windows         # Windows x86_64 only
#   SKIP_BUILD=1 ./scripts/release-local.sh --skip-build
#
# Prerequisites:
#   1. rustup target add aarch64-apple-darwin x86_64-apple-darwin \
#                         x86_64-unknown-linux-gnu x86_64-pc-windows-msvc
#   2. cargo install cargo-xwin                         # For Windows cross-compile
#   3. brew install llvm mingw-w64                      # For cross-compile linkers
#   4. npm install (in web/)
#   5. npm install (in cloudflare-worker/)
#
# Env vars:
#   R2_PUBLIC_BASE_URL=...  Override R2 public URL
#   SKIP_BUILD=1            Skip cargo build
#   SKIP_WEB=1              Skip web frontend build
#   SKIP_UPLOAD=1           Skip R2/D1 upload
#   BUILD_NUMBER=N          Override build number for D1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKER_DIR="$PROJECT_DIR/cloudflare-worker"
WEB_DIR="$PROJECT_DIR/web"
APP_NAME="postgrestui"
R2_BUCKET="postgrestui-releases"
R2_PUBLIC_BASE_URL="${R2_PUBLIC_BASE_URL:-https://dl-postgrestui.voltrus.id}"
D1_DB="postgrestui-releases-db"
SKIP_BUILD="${SKIP_BUILD:-0}"
SKIP_WEB="${SKIP_WEB:-0}"
SKIP_UPLOAD="${SKIP_UPLOAD:-0}"
BUILD_NUMBER="${BUILD_NUMBER:-1}"
VERSION=""
TARGET_FILTER="all"

# Parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-upload)  SKIP_UPLOAD=1; shift ;;
    --skip-build)   SKIP_BUILD=1;  shift ;;
    --skip-web)     SKIP_WEB=1;    shift ;;
    --target)       TARGET_FILTER="${2:-all}"; shift 2 ;;
    --build-number) BUILD_NUMBER="${2:-1}"; shift 2 ;;
    --)             shift; break ;;
    -*)             echo "Unknown option: $1" >&2; exit 1 ;;
    *)
      if [[ -z "$VERSION" ]]; then
        VERSION="$1"
      fi
      shift
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*= *"\([^"]*\)".*/\1/')
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}[*]${NC} $1" >&2; }
ok()    { echo -e "${GREEN}[✓]${NC} $1" >&2; }
warn()  { echo -e "${YELLOW}[!]${NC} $1" >&2; }
err()   { echo -e "${RED}[✗]${NC} $1" >&2; }
step()  { echo -e "\n${BLUE}━━━ $1 ━━━${NC}" >&2; }
banner() { echo -e "\n${BOLD}${BLUE}════════════════════════════════════════════════${NC}\n${BOLD}$1${NC}\n${BOLD}${BLUE}════════════════════════════════════════════════${NC}" >&2; }

BUILT_FILES=()

# ============================================================
# BUILD FUNCTIONS
# ============================================================

build_native() {
  # macOS native build (aarch64 on Apple Silicon, x86_64 on Intel)
  local target="${1}-apple-darwin"
  local platform_key="darwin-${1}"
  local binary_name="$APP_NAME"
  local build_dir="$PROJECT_DIR/target/$target/release"
  local binary_path="$build_dir/$binary_name"

  step "macOS $1 ($target)"

  if [[ "$SKIP_BUILD" == "1" ]]; then
    if [[ -f "$binary_path" ]]; then
      warn "Skipping build, using existing: $binary_path"
      echo "$binary_path|$platform_key"
      return
    fi
    err "Binary not found — cannot skip build"
    return 1
  fi

  info "Building for $target..."
  cargo build --release --target "$target" 2>&1 | tail -5 >&2

  if [[ ! -f "$binary_path" ]]; then
    err "Build failed — binary not found at $binary_path"
    return 1
  fi

  local file_size
  file_size=$(stat -f%z "$binary_path" 2>/dev/null || stat -c%s "$binary_path")
  local file_size_mb
  file_size_mb=$(echo "scale=1; $file_size / 1048576" | bc)
  ok "Built: $binary_path (${file_size_mb} MB)"

  echo "$binary_path|$platform_key"
}

build_linux() {
  local target="x86_64-unknown-linux-gnu"
  local platform_key="linux-x86_64"
  local binary_name="$APP_NAME"
  local build_dir="$PROJECT_DIR/target/$target/release"
  local binary_path="$build_dir/$binary_name"

  step "Linux x86_64 ($target)"

  if [[ "$SKIP_BUILD" == "1" ]]; then
    if [[ -f "$binary_path" ]]; then
      warn "Skipping build, using existing: $binary_path"
      echo "$binary_path|$platform_key"
      return
    fi
    err "Binary not found — cannot skip build"
    return 1
  fi

  info "Building for $target..."

  # Ensure linker is available
  local linker
  for candidate in x86_64-linux-gnu-gcc x86_64-unknown-linux-gnu-gcc x86_64-linux-musl-gcc; do
    if command -v "$candidate" &>/dev/null; then
      linker="$candidate"
      break
    fi
  done

  if [[ -n "${linker:-}" ]]; then
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$linker" \
      cargo build --release --target "$target" 2>&1 | tail -5 >&2
  else
    warn "No cross-linker found — attempting native build (may fail on macOS)"
    cargo build --release --target "$target" 2>&1 | tail -10 >&2
  fi

  if [[ ! -f "$binary_path" ]]; then
    err "Build failed — binary not found at $binary_path"
    return 1
  fi

  local file_size
  file_size=$(stat -f%z "$binary_path" 2>/dev/null || stat -c%s "$binary_path")
  local file_size_mb
  file_size_mb=$(echo "scale=1; $file_size / 1048576" | bc)
  ok "Built: $binary_path (${file_size_mb} MB)"

  echo "$binary_path|$platform_key"
}

build_windows() {
  local target="x86_64-pc-windows-msvc"
  local platform_key="windows-x86_64"
  local binary_name="${APP_NAME}.exe"
  local build_dir="$PROJECT_DIR/target/$target/release"
  local binary_path="$build_dir/$binary_name"

  step "Windows x86_64 ($target)"

  if ! command -v cargo-xwin &>/dev/null; then
    err "cargo-xwin not found. Install with: cargo install cargo-xwin"
    return 1
  fi

  if [[ "$SKIP_BUILD" == "1" ]]; then
    if [[ -f "$binary_path" ]]; then
      warn "Skipping build, using existing: $binary_path"
      echo "$binary_path|$platform_key"
      return
    fi
    err "Binary not found — cannot skip build"
    return 1
  fi

  info "Building for $target with cargo-xwin..."

  eval "$(cargo xwin env --target "$target" 2>/dev/null)"
  local llvm_bin
  llvm_bin="$(brew --prefix llvm 2>/dev/null)/bin"
  if [[ -n "$llvm_bin" ]]; then
    export PATH="$llvm_bin:$PATH"
  fi

  cargo xwin build --release --target "$target" 2>&1 | tail -10 >&2

  if [[ ! -f "$binary_path" ]]; then
    err "Build failed — binary not found at $binary_path"
    return 1
  fi

  local file_size
  file_size=$(stat -f%z "$binary_path" 2>/dev/null || stat -c%s "$binary_path")
  local file_size_mb
  file_size_mb=$(echo "scale=1; $file_size / 1048576" | bc)
  ok "Built: $binary_path (${file_size_mb} MB)"

  echo "$binary_path|$platform_key"
}

# ============================================================
# RESOLVE PLATFORMS
# ============================================================

PLATFORMS=()

case "$TARGET_FILTER" in
  all)       PLATFORMS=("aarch64" "x86_64" "linux" "windows") ;;
  mac)       PLATFORMS=("aarch64" "x86_64") ;;
  mac-arm)   PLATFORMS=("aarch64") ;;
  mac-intel) PLATFORMS=("x86_64") ;;
  aarch64)   PLATFORMS=("aarch64") ;;
  x86_64)    PLATFORMS=("x86_64") ;;
  linux)     PLATFORMS=("linux") ;;
  windows)   PLATFORMS=("windows") ;;
  win)       PLATFORMS=("windows") ;;
  *)         err "Unknown target: $TARGET_FILTER (use: all, mac, mac-arm, mac-intel, linux, windows)"; exit 1 ;;
esac

# ============================================================
# MAIN
# ============================================================

banner "PostgresTUI $VERSION — Local Release"

info "Platforms: ${PLATFORMS[*]}"

# Ensure Rust targets are installed
for platform in "${PLATFORMS[@]}"; do
  case "$platform" in
    aarch64)   target="aarch64-apple-darwin" ;;
    x86_64)    target="x86_64-apple-darwin" ;;
    linux)     target="x86_64-unknown-linux-gnu" ;;
    windows)   target="x86_64-pc-windows-msvc" ;;
  esac
  if ! rustup target list --installed | grep -q "$target"; then
    warn "Installing missing target: $target"
    rustup target add "$target"
  fi
done
ok "Rust targets ready"

# Build web frontend
if [[ "$SKIP_WEB" != "1" && -d "$WEB_DIR" ]]; then
  step "Building web frontend"
  if [[ ! -d "$WEB_DIR/node_modules" ]]; then
    info "Installing npm dependencies..."
    (cd "$WEB_DIR" && npm install)
  fi
  (cd "$WEB_DIR" && npm run build) 2>&1 | tail -5 >&2
  ok "Web frontend built"
fi

# Verify wrangler
if [[ "$SKIP_UPLOAD" != "1" ]]; then
  if [[ ! -d "$WORKER_DIR/node_modules" ]]; then
    info "Installing wrangler dependencies..."
    (cd "$WORKER_DIR" && npm install)
  fi
  ok "Wrangler ready"
fi

# ============================================================
# BUILD EACH PLATFORM
# ============================================================

for platform in "${PLATFORMS[@]}"; do
  case "$platform" in
    aarch64|x86_64)
      build_result=$(build_native "$platform") || {
        err "Failed to build macOS $platform, skipping..."
        continue
      }
      ;;
    linux)
      build_result=$(build_linux) || {
        err "Failed to build Linux, skipping..."
        continue
      }
      ;;
    windows)
      build_result=$(build_windows) || {
        err "Failed to build Windows, skipping..."
        continue
      }
      ;;
  esac

  IFS='|' read -r binary_path platform_key <<< "$build_result"

  # Create zip
  zip_name="${APP_NAME}-v${VERSION}-${platform_key}.zip"
  zip_path="$PROJECT_DIR/$zip_name"
  rm -f "$zip_path"

  info "Creating $zip_name..."
  (cd "$(dirname "$binary_path")" && zip "$zip_path" "$(basename "$binary_path")") >&2

  _file_size=$(stat -f%z "$zip_path" 2>/dev/null || stat -c%s "$zip_path")
  _file_size_mb=$(echo "scale=1; $_file_size / 1048576" | bc)
  ok "Zip: $zip_name (${_file_size_mb} MB)"

  BUILT_FILES+=("$platform_key|$zip_name|$zip_path")
done

# ============================================================
# UPLOAD TO R2 & D1
# ============================================================

if [[ "$SKIP_UPLOAD" == "1" ]]; then
  banner "Build Complete (upload skipped)"
  for entry in "${BUILT_FILES[@]}"; do
    IFS='|' read -r platform_key filename filepath <<< "$entry"
    echo -e "  ${GREEN}${platform_key}${NC}: ${BLUE}$filepath${NC}"
  done
  echo ""
  echo -e "  ${YELLOW}To upload:${NC}  SKIP_BUILD=1 SKIP_WEB=1 ./scripts/release-local.sh --skip-build --skip-web"
  echo ""
  exit 0
fi

step "Uploading to R2 & D1"

cd "$WORKER_DIR"

for entry in "${BUILT_FILES[@]:-}"; do
  [[ -z "$entry" ]] && continue
  IFS='|' read -r platform_key filename filepath <<< "$entry"

  info "Uploading $filename to R2..."
  npx wrangler r2 object put "$R2_BUCKET/$filename" --file="$filepath" --remote
  ok "Uploaded: $filename"

  file_size=$(stat -f%z "$filepath" 2>/dev/null || stat -c%s "$filepath")

  info "Inserting $platform_key into D1 $D1_DB..."
  npx wrangler d1 execute "$D1_DB" --remote --command \
    "INSERT OR REPLACE INTO release_files (version, platform, filename, r2_key, size) VALUES ($BUILD_NUMBER, '$platform_key', '$filename', '$filename', $file_size);"
  ok "D1 record: $platform_key"
done

cd "$PROJECT_DIR"

# ============================================================
# DONE
# ============================================================

banner "Release Complete — v$VERSION"

for entry in "${BUILT_FILES[@]:-}"; do
  [[ -z "$entry" ]] && continue
  IFS='|' read -r platform_key filename filepath <<< "$entry"
  _file_size=$(stat -f%z "$filepath" 2>/dev/null || stat -c%s "$filepath")
  _file_size_mb=$(echo "scale=1; $_file_size / 1048576" | bc)
  echo -e "  ${GREEN}${platform_key}${NC}: ${BLUE}$filepath${NC} (${_file_size_mb} MB)"
done

echo ""
echo -e "  Download:  ${BLUE}${R2_PUBLIC_BASE_URL}/latest${NC}"
echo -e "  Downloads: ${BLUE}${R2_PUBLIC_BASE_URL}/downloads${NC}"
echo ""
echo -e "  ${YELLOW}Deploy worker if updated:${NC}"
echo -e "    cd $WORKER_DIR && npx wrangler deploy"
echo ""
