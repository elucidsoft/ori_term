#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo fmt ==="
cargo fmt --all -- --check

echo ""
echo "All formatting checks passed."
