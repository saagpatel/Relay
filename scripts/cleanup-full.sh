#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$ROOT_DIR/scripts/cleanup-heavy.sh"

rm -rf "$ROOT_DIR/client/node_modules"

(
  cd "$ROOT_DIR/server"
  go clean -cache -testcache
)

echo "Full local reproducible cleanup complete."
