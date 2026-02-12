#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"

echo "=== cargo clippy (${TARGET}) ==="
cargo clippy --target "${TARGET}" -- -D warnings

echo ""
echo "All clippy checks passed."
