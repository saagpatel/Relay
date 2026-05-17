# Relay — Portfolio Disposition

**Status:** Release Frozen — macOS release candidate prepared,
governance gates G0–G6 documented, awaiting operator-only Apple
signing + release-promotion ceremony. Do not surface for routine
review until unblock trigger fires.

---

## Why this file exists

Portfolio operating system has been surfacing Relay as a row needing
review. The repo is the opposite of stalled: it has 7 release gates
(G0 Hygiene through G6 Docs Closeout), full release governance
docs, signed/release materials configuration, operational gates
config, CI vulnerability scanning, and a captured local smoke
walkthrough. The most recent commit prepares the macOS release
candidate.

What's missing is the portfolio-OS disposition signal. This file
supplies it.

---

## Current state in one paragraph

Relay is a Go + Rust + Tauri 2 desktop app for peer-to-peer file
transfer with QUIC LAN connections, automatic relay fallback, and
end-to-end encryption (SPAKE2 + AES-256-GCM, SHA-256 checksums). It
runs zero-knowledge — files never touch the cloud, the signaling
server is encrypted-blind. The macOS release candidate is prepared.
G0 (hygiene) through G4 (perf/reliability) are documented and
enforced via `.codex/verify.commands` and CI. G5 (promotion) and G6
(docs closeout) require operator-mediated ceremony: signing,
canary, rollback drill approvals.

For full detail (in priority order):
- `docs/RELEASE_GATES.md`
- `docs/LOCAL_SMOKE.md`
- `docs/ARCHITECTURE.md`
- `docs/release/`
- `docs/SELF_HOSTING.md` (for the relay-server side)

---

## Portfolio operating system instructions

| Aspect | Posture |
|---|---|
| Portfolio status | `Release Frozen` |
| Macos release candidate | **Prepared** (commit `ab85e88`) |
| Review cadence | Suspend overdue counting |
| Resurface conditions | (a) Apple signing credentials wired, (b) operator completes canary + rollback drill (G5), or (c) operator opens a self-hosting deployment packet (separate workstream from the desktop app) |
| Co-batch with | The signing-frozen cluster: DesktopPEt, ContentEngine, AIGCCore. Four repos in one signing session is meaningfully cheaper than four separate sessions. |

---

## Why "Release Frozen" instead of other dispositions

- **Active** — wrong. v0 surface is gated by G5/G6 ceremony, not
  more features. Adding scope now competes with the unblock without
  addressing it.
- **Cold Storage** — wrong. The product is built, tested, and
  release-prepared. Calling it cold misrepresents the gate state.
- **Archived / Wind-down** — wrong. The release-prep commit is from
  2026-03-14; nothing has been abandoned.
- **Release Frozen** — correct, same shape as DesktopPEt /
  ContentEngine / AIGCCore. The signing cluster is now **4 repos**.

---

## Unblock trigger (operator)

When the operator is ready to ship:

1. Wire Apple Developer ID Application certificate + notarization
   credentials.
2. Run the enforced release path against the prepared RC.
3. Execute the canary deployment per `docs/release/` templates.
4. Run the rollback drill, capture approval evidence (`scripts/ci/check-release-evidence.mjs`
   enforces evidence presence in enforced mode).
5. Promote through release rings: `internal` → `beta` → `stable`
   (defined in `client/src-tauri/release-channels.json`).
6. Cut the v0 GitHub release.

Estimated operator time once credentials are in hand: ~4 hours
including signing, canary, rollback drill, and ring promotion.

---

## Reactivation procedure (for the next code session)

When portfolio operating system flips this row to `Active`:

1. **Important: local `main` branch was tracking `legacy-origin/main`
   (the frozen saagar210 GitHub account).** The local branch
   structure was corrected during this disposition: `main` now
   tracks `origin/main` (saagpatel). Verify with
   `git branch -vv | grep '^\* main'`. Do not push to
   `legacy-origin`.
2. Delete the stale `codex/*` branches still tracking
   `legacy-origin/*`. Use `git branch --set-upstream-to=origin/<x>`
   or delete them entirely.
3. Re-run `.codex/verify.commands` (the canonical gate enforcer) to
   confirm G0–G4 still pass on current toolchain.
4. Only then proceed to the signing + canary work that motivated the
   reactivation.

---

## Last known reference

| Field | Value |
|---|---|
| Last meaningful commit on `codex/chore/bootstrap-codex-os` | `ab85e88` chore(repo): prepare Relay macOS release candidate |
| Default branch | `main` (note: docs reference both `master` and `main` due to transitional compatibility) |
| Release gates | G0 (Hygiene) → G6 (Docs Closeout), defined in `docs/RELEASE_GATES.md` |
| Build verification status | green |
| Smoke walkthrough | captured in `docs/LOCAL_SMOKE.md`, unrun against the prepared RC |
| Blocker | Apple signing + canary + rollback drill (operator-only) |
| Migration note | `legacy-origin` points at frozen `saagar210` account; do not push there |
