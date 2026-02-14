#!/usr/bin/env bash
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"

echo "=== cargo build --workspace (${TARGET}) ==="
cargo build --workspace --target "${TARGET}"

echo ""
echo "Build succeeded."
