#!/bin/sh
set -eu

REPOSITORY="https://github.com/prongbang/drop.git"

if ! command -v cargo >/dev/null 2>&1; then
	cat >&2 <<'EOF'
Rust and Cargo are required to install Drop.
Install Rust from https://rustup.rs, then run this installer again.
EOF
	exit 1
fi

printf 'Installing Drop from %s\n' "$REPOSITORY"
cargo install --git "$REPOSITORY" --force

CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
case ":$PATH:" in
	*":$CARGO_BIN:"*) ;;
	*) printf 'Add %s to your PATH to run drop.\n' "$CARGO_BIN" ;;
esac
