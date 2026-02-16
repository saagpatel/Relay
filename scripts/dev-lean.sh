#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEAN_TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/relay-lean.XXXXXX")"
SERVER_PID=""

cleanup() {
  local code=$?

  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi

  "$ROOT_DIR/scripts/cleanup-heavy.sh" >/dev/null 2>&1 || true
  rm -rf "$LEAN_TMP_DIR"

  exit "$code"
}

trap cleanup EXIT INT TERM

(
  cd "$ROOT_DIR/server"
  go build -o "$LEAN_TMP_DIR/relay-server" .
)

"$LEAN_TMP_DIR/relay-server" &
SERVER_PID=$!

cd "$ROOT_DIR/client"
RELAY_VITE_CACHE_DIR="$LEAN_TMP_DIR/vite-cache" \
CARGO_TARGET_DIR="$LEAN_TMP_DIR/cargo-target" \
GOCACHE="$LEAN_TMP_DIR/go-build-cache" \
pnpm tauri dev
