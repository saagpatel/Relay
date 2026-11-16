# Project 2: Relay handshake, formally proved — progress

Branch `fix/relay-protocol-proof` (renamed from the session default to satisfy
`scripts/git/guard-branch.sh`). Started 2026-11-16 PT.

## Tasks

- [x] Check for Codex activity after Nov 9: none (no `codex/*` branch or PR
      newer than Nov 3; Codex sessions since then only list Relay in
      inventories).
- [x] Security properties written down, each tied to code or docs (`README.md`).
- [x] ProVerif 2.05 installed (`brew install opam expat gtk+ pkgconf`;
      `opam init --bare`; `opam switch create default ocaml-base-compiler`;
      `opam install proverif`).
- [x] 2.0.0 model: P1, P2, P7 proved; P3a, P3b, P4, P5, P6 attacked; P8
      attacked (documented).
- [x] Each attack reproduced in Rust (`client/src-tauri/tests/protocol_attacks.rs`);
      all 3 tests failed on `f43d036`.
- [x] Fix: protocol 3.0.0 (HKDF direction keys, role-bound confirmation,
      sealed peer frames). 3.0.0 model: P1 to P7 proved.
- [x] SPAKE2 vectors: crate's python-spake2 vectors pass on 0.4.0; RFC 9382
      and Wycheproof do not cover this construction; HKDF pinned against an
      independent RFC 5869 implementation.
- [x] Nightly sweep: causes found and fixed (below).
- [x] Independent model-vs-code review (fresh subagent): no divergence that breaks a 3.0.0 proof; 7 README corrections applied; 2 real issues outside the model fixed (256 MiB pre-auth frames, code in client logs).
- [ ] PR through `agent-pr-merge --billing-blocked`.

## Nightly sweep, red since at least Nov 1

1. The workflow pinned Go 1.25.13 while `server/go.mod` says `go 1.27` (since
   the Oct 13 modernize commit): `go: go.mod requires go >= 1.27`. Now
   `go-version-file: server/go.mod` in both workflows.
2. Next failure once Go ran: `govulncheck` reports GO-2026-6859 (net/http) in
   go1.27.3, fixed in go1.27.4. `toolchain go1.27.4`; govulncheck v1.7.0 to
   v1.9.0. The server Docker image is still 1.27.3; open Dependabot PR #112
   bumps it.
3. The verify contract and `check-release-evidence.mjs` still required
   `docs/release/*` and `scripts/perf/memory-smoke.mjs`, which were deleted on
   Oct 12. Those requirements are removed.
4. `cargo audit --deny warnings` found five new advisories: quinn-udp
   RUSTSEC-2026-0538, rustls-webpki -0382, webpki-root-certs -0383 and rustix
   -0486 were fixed by `cargo update -p` patch bumps. pin-utils -0441 is
   unmaintained with no fix (via tauri -> reqwest -> hyper) and is ignored
   with a comment.

## Open

- Relay #1 queued: protocol break to 3.0.0 (guess) vs compatibility.
- Not done: pinning the QUIC certificate to the confirmed fingerprint. The
  sealed channel makes it unnecessary for integrity; it would add
  transport-level authentication.
- The code is still in the signaling URL, so the server (and any `ws://`
  observer) knows the password. Hashing the code would not help (about
  655,000 values, brute-forced at once). The fix is magic-wormhole's split:
  send only the leading part (the digit, or a longer nameplate) to the server
  and keep the words as the SPAKE2 password. That changes the room model and
  the code format, so it is a design change for Saagar.
