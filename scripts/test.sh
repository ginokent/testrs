#!/usr/bin/env bash
# scripts/test.sh — workspace test と doctest を順次実行。
# CI の test job (.github/workflows/ci.yml) と同じ挙動。toolchain は
# rust-toolchain.toml の pin (channel = "1.95") に従う。
# MSRV (1.82) 検証は CI 専用 job なのでローカル task からは外している。
set -euo pipefail
echo "==> cargo test --workspace --all-targets"
cargo test --workspace --all-targets
echo "==> cargo test --workspace --doc"
cargo test --workspace --doc
