#!/bin/sh
set -eu

REPOSITORY="https://raw.githubusercontent.com/prongbang/drop/main/bin"
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS/$ARCH" in
	Darwin/arm64|Darwin/aarch64) PLATFORM="darwin-arm64" ;;
	Darwin/x86_64) PLATFORM="darwin-x86_64" ;;
	*)
		printf 'Prebuilt Drop binary is not available for %s/%s.\n' "$OS" "$ARCH" >&2
		exit 1
		;;
esac

INSTALL_DIR=${DROP_INSTALL_DIR:-"$HOME/.local/bin"}
TMP_FILE=$(mktemp "${TMPDIR:-/tmp}/drop.XXXXXX")
trap 'rm -f "$TMP_FILE"' 0
trap 'exit 1' HUP INT TERM

printf 'Downloading Drop for %s...\n' "$PLATFORM"
curl -fsSL "$REPOSITORY/drop-$PLATFORM" -o "$TMP_FILE"
mkdir -p "$INSTALL_DIR"
install -m 755 "$TMP_FILE" "$INSTALL_DIR/drop"

printf 'Installed Drop to %s/drop\n' "$INSTALL_DIR"
case ":$PATH:" in
	*":$INSTALL_DIR:"*) ;;
	*) printf 'Add %s to your PATH to run drop.\n' "$INSTALL_DIR" ;;
esac
