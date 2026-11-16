#!/usr/bin/env bash
# Run every ProVerif query file in this directory and print each verdict.
# Requires proverif on PATH (opam install proverif). Exits non-zero if any
# query in the current protocol model (relay_v3*) is not proved.
set -euo pipefail
cd "$(dirname "$0")"
status=0
for model in v2 v3; do
  for f in relay_${model}.pv relay_${model}_fs.pv relay_${model}_server.pv; do
    [[ -f "$f" ]] || continue
    echo "== $f"
    out="$(proverif -lib "relay_${model}.pvl" "$f")"
    printf '%s\n' "$out" | grep -E '^(RESULT|Weak secret)' | sed 's/^/  /'
    if [[ "$model" == v3 && "$f" != *_server.pv ]] && printf '%s\n' "$out" | grep -E '^RESULT' | grep -qv 'is true'; then
      status=1
    fi
  done
done
exit "$status"
