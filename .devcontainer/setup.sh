#!/bin/bash
set -euo pipefail

echo "=== ferncad dev environment setup ==="

# Verify toolchain
rustc --version
cargo --version
wasm-pack --version
node --version

echo "=== setup complete ==="
