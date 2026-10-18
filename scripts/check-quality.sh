#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
npm run check:format
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --no-deps -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml --all-targets
cargo build --locked --manifest-path src-tauri/Cargo.toml --example engine-check
engine="${CARGO_TARGET_DIR:-src-tauri/target}/debug/examples/engine-check"
python3 scripts/test-workflow.py --engine "$engine" --large

# Network-dependent advisory checks are separate from the functional suite:
# npm audit --package-lock-only
# cargo audit --file src-tauri/Cargo.lock
