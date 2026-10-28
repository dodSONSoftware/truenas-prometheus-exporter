#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
rustfmt --edition 2024 --check "$root/deployment/tests/runtime_probe.rs"
cargo fmt --manifest-path "$root/application/Cargo.toml" --check
cargo clippy --locked --manifest-path "$root/application/Cargo.toml" --all-targets -- -D warnings
cargo test --locked --manifest-path "$root/application/Cargo.toml"
