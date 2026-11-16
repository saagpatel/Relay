#!/usr/bin/env bash
# Run every ProVerif query file in this directory and print each verdict.
# Requires proverif on PATH (opam install proverif). Exits non-zero unless:
#   - every 3.0.0 property query (relay_v3.pv, relay_v3_fs.pv, relay_v3_server.pv)
#     is proved, and
#   - every sanity query (relay_v*_reach.pv) finds a trace, so no proof is vacuous.
set -euo pipefail
cd "$(dirname "$0")"
status=0
for model in v2 v3; do
  for f in relay_${model}.pv relay_${model}_fs.pv relay_${model}_server.pv relay_${model}_codeholder.pv relay_${model}_reach.pv; do
    [[ -f "$f" ]] || continue
    echo "== $f"
    out="$(proverif -lib "relay_${model}.pvl" "$f")"
    results="$(printf '%s\n' "$out" | grep -E '^RESULT' || true)"
    printf '%s\n' "$results" | sed 's/^/  /'
    if [[ -z "$results" ]]; then
      echo "  no results (parse error?)"; status=1
    fi
    case "$f" in
      *_reach.pv)
        if printf '%s\n' "$results" | grep -qv 'is false'; then
          echo "  FAIL: an honest run cannot complete; proofs would be vacuous"; status=1
        fi ;;
      relay_v3.pv|relay_v3_fs.pv|relay_v3_server.pv)
        if printf '%s\n' "$results" | grep -v '^RESULT (even' | grep -qv 'is true'; then
          echo "  FAIL: a 3.0.0 property is not proved"; status=1
        fi ;;
    esac
  done
done
exit "$status"
