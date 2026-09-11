#!/bin/sh
set -e

# Detect OS and Architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64|amd64)
    ARCH="x86_64"
    ;;
  aarch64|arm64)
    ARCH="aarch64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

case "$OS" in
  linux)
    TARGET="${ARCH}-unknown-linux-musl"
    if [ "$ARCH" = "x86_64" ]; then
      BIN_URL="https://openhub.run/dist/oh-linux-x86_64"
    else
      BIN_URL="https://openhub.run/dist/oh-linux-${ARCH}"
    fi
    ;;
  darwin)
    if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
      BIN_URL="https://openhub.run/dist/oh-darwin-arm64"
    else
      BIN_URL="https://openhub.run/dist/oh-darwin-${ARCH}"
    fi
    ;;
  *)
    echo "Unsupported operating system: $OS"
    exit 1
    ;;
esac

INSTALL_DIR="${OPENHUB_INSTALL_DIR:-/usr/local/bin}"
TMP_DIR="$(mktemp -d)"
TMP_BIN="${TMP_DIR}/oh"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Downloading oh CLI from ${BIN_URL}..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$BIN_URL" -o "$TMP_BIN"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TMP_BIN" "$BIN_URL"
else
  echo "Error: curl or wget is required to install oh"
  exit 1
fi

if head -c 64 "$TMP_BIN" | grep -q '<!DOCTYPE html>\|<!doctype html>'; then
  echo "Error: ${BIN_URL} returned HTML, not a binary."
  echo "Build from source: cargo install --path . --bin oh"
  exit 1
fi

chmod +x "$TMP_BIN"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP_BIN" "${INSTALL_DIR}/oh"
else
  echo "Installing to ${INSTALL_DIR} (requires sudo)..."
  sudo mv "$TMP_BIN" "${INSTALL_DIR}/oh"
fi

echo "Successfully installed oh to ${INSTALL_DIR}/oh"
oh --help 2>&1 | head -n 8 || true
