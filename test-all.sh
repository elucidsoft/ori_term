#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo test ==="
cargo test

echo ""
echo "All tests passed."
