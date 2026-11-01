#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if command -v cargo >/dev/null 2>&1; then
    cargo build --workspace --release --locked
else
    "$HOME/.cargo/bin/cargo" build --workspace --release --locked
fi
exec ./target/release/djejaj "$@"
