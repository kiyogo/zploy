#!/bin/bash
set -e

echo "========================================================"
echo "       Zploy - Static Web Hosting Server (Linux)"
echo "========================================================"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

if [ ! -f "config/config.toml" ]; then
    echo "[INFO] Copying sample configuration config/config.example.toml to config/config.toml..."
    cp config/config.example.toml config/config.toml
fi

mkdir -p data storage/sites storage/staging storage/temp

echo "[START] Starting Zploy..."
if [ -f "target/release/zploy" ]; then
    ./target/release/zploy
else
    cargo run --release
fi
