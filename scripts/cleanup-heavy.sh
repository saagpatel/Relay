#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

rm -rf \
  "$ROOT_DIR/client/dist" \
  "$ROOT_DIR/client/src-tauri/target" \
  "$ROOT_DIR/client/src-tauri/gen" \
  "$ROOT_DIR/client/node_modules/.vite" \
  "$ROOT_DIR/client/node_modules/.vite-temp" \
  "$ROOT_DIR/server/relay-server"

echo "Heavy build artifacts removed."
