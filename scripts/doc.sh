#!/usr/bin/env bash
# scripts/doc.sh — rustdoc を strict 化 (broken_intra_doc_links 等を error 化)。
# CI の doc job (.github/workflows/ci.yml) と同じ挙動。
set -euo pipefail
echo "==> RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
