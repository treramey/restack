#!/usr/bin/env bash
set -euo pipefail

# Cross-compile restack for all supported platforms.
# Usage: ./scripts/build-release.sh [version]
#
# Requires cross-compilation toolchains or `cross` (https://github.com/cross-rs/cross).

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT"

declare -A TARGETS=(
  ["darwin-arm64"]="aarch64-apple-darwin"
  ["darwin-x64"]="x86_64-apple-darwin"
  ["linux-x64"]="x86_64-unknown-linux-gnu"
  ["win32-x64"]="x86_64-pc-windows-msvc"
)

echo "Building restack v${VERSION} for all platforms..."

for platform in "${!TARGETS[@]}"; do
  target="${TARGETS[$platform]}"
  echo ""
  echo "==> Building ${platform} (${target})"

  # Use cargo if building for host, cross otherwise
  if rustup target list --installed | grep -q "$target"; then
    cargo build --release --target "$target"
  else
    echo "    Target ${target} not installed locally, trying cross..."
    cross build --release --target "$target"
  fi

  # Copy binary to npm platform package
  npm_dir="${ROOT}/npm/${platform}"
  if [[ "$platform" == win32-* ]]; then
    cp "target/${target}/release/restack.exe" "${npm_dir}/restack.exe"
  else
    cp "target/${target}/release/restack" "${npm_dir}/restack"
    chmod +x "${npm_dir}/restack"
  fi

  echo "    Copied to npm/${platform}/"
done

echo ""
echo "Build complete. Platform binaries are in npm/*/"
echo "To publish: cd npm/<platform> && npm publish"
