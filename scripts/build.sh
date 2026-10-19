#!/bin/bash
set -e

echo "========================================================"
echo "       Zploy - Build Release Binary (Linux)"
echo "========================================================"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

echo "[BUILD] Compiling Zploy (cargo build --release)..."
cargo build --release

echo ""
echo "[SUCCESS] Executable binary created at: target/release/zploy"
ls -lh target/release/zploy
