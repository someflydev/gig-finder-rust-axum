#!/bin/sh
set -eu

VERSION="${TAILWIND_VERSION:-v3.4.17}"
ROOT_DIR="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
BIN_DIR="$ROOT_DIR/bin"
OUT="$BIN_DIR/tailwindcss"
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) os_tag="macos" ;;
  Linux) os_tag="linux" ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) arch_tag="arm64" ;;
  x86_64|amd64) arch_tag="x64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

mkdir -p "$BIN_DIR"

URL="https://github.com/tailwindlabs/tailwindcss/releases/download/${VERSION}/tailwindcss-${os_tag}-${arch_tag}"
echo "Downloading Tailwind CSS standalone binary ${VERSION} -> $OUT"
curl -fsSL "$URL" -o "$OUT"
chmod +x "$OUT"
"$OUT" -v || true
echo "Installed $OUT"
