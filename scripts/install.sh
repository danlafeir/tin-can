#!/bin/sh
set -e

REPO=danlafeir/tin-can
BINARY=tin-can
INSTALL_DIR=~/.local/bin

# Detect OS
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$OS" in
  linux)  OS=linux  ;;
  darwin) OS=darwin ;;
  *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

# Detect ARCH
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)   ARCH=amd64 ;;
  arm64|aarch64)  ARCH=arm64 ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

# Find the latest binary for this OS/ARCH via the GitHub API.
# Deployed binaries follow the pattern: tin-can-<os>-<arch>-<git-hash>
API_URL="https://api.github.com/repos/$REPO/contents/bin"
FILENAME=$(curl -sSL "$API_URL" \
  | grep -o '"name": *"'"$BINARY"'-'"$OS"'-'"$ARCH"'-[a-zA-Z0-9]*"' \
  | sed 's/.*: *"//;s/"//' \
  | sort \
  | tail -n1)

if [ -z "$FILENAME" ]; then
  echo "No release binary found for $OS/$ARCH." >&2
  echo "Build from source: https://github.com/$REPO" >&2
  exit 1
fi

URL="https://raw.githubusercontent.com/$REPO/main/bin/$FILENAME"
TMP=$(mktemp)

echo "Downloading $FILENAME ..."
curl -sSLf "$URL" -o "$TMP"
chmod +x "$TMP"

mkdir -p "$INSTALL_DIR"
echo "Installing to $INSTALL_DIR/$BINARY ..."
mv "$TMP" "$INSTALL_DIR/$BINARY"

echo ""
echo "$BINARY installed successfully."
echo "Make sure $INSTALL_DIR is in your PATH:"
echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
