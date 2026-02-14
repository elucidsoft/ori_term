#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo test --workspace ==="
cargo test --workspace

echo ""
echo "All tests passed."
