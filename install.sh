#!/bin/sh
set -e

REPO="maxischmaxi/maxmux"
INSTALL_DIR="${MAXMUX_INSTALL_DIR:-/usr/local/bin}"

# Detect OS
OS="$(uname -s)"
case "$OS" in
  Linux*)  os="linux" ;;
  Darwin*) os="darwin" ;;
  *)       echo "Error: Unsupported OS: $OS" >&2; exit 1 ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)  arch="x64" ;;
  aarch64|arm64)  arch="arm64" ;;
  *)              echo "Error: Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

BINARY="maxmux-${os}-${arch}"
URL="https://github.com/${REPO}/releases/latest/download/${BINARY}"

echo "Installing maxmux (${os}-${arch})..."

TMPFILE="$(mktemp)"
trap 'rm -f "$TMPFILE"' EXIT

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$URL" -o "$TMPFILE"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TMPFILE" "$URL"
else
  echo "Error: curl or wget required" >&2
  exit 1
fi

chmod +x "$TMPFILE"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPFILE" "$INSTALL_DIR/maxmux"
else
  sudo mv "$TMPFILE" "$INSTALL_DIR/maxmux"
fi

echo "maxmux installed to ${INSTALL_DIR}/maxmux"
maxmux --version 2>/dev/null || true
