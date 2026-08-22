# AGENTS.md

<!-- comm-contract:start -->

## Communication Contract

- Inherit global Codex communication and reporting rules from `~/.codex/AGENTS.override.md` and `~/.codex/policies/communication/BigPictureReportingV1.md`.
- Repo-specific instructions below add project constraints only; do not restate global voice or status-reporting rules here.
<!-- comm-contract:end -->

## Inherited Operating Rules

- Inherit global git, review/fix, testing, docs, skill-use, and reporting gates from `~/.codex/AGENTS.md` and active session instructions.
- Use `.codex/verify.commands` and `.codex/scripts/run_verify_commands.sh` as this repo's local verification authority when present.
- Keep the project-specific portfolio constraints below as the source of truth for runtime, privacy, and release risks.

<!-- portfolio-context:start -->
# Portfolio Context

## What This Project Is

Relay is a zero-cloud file-transfer app that uses direct QUIC for LAN speed and falls back to an encrypted WebSocket relay when NAT or firewalls block direct connections. Transfers are end-to-end encrypted with SPAKE2 key exchange and AES-256-GCM so the signaling server cannot read file contents.

## Current State

The repo is active desktop/networking product work. Existing local changes are PR-template metadata, so context recovery should stay documentation-only.

## Stack

| Layer | Technology |
|-------|------------|
| Desktop shell | Tauri 2 |
| Frontend | Solid.js + TypeScript + Tailwind CSS 4 |
| Backend | Rust 2021 |
| Transport | QUIC (direct) + WebSocket relay (fallback) |
| Encryption | SPAKE2 key exchange + AES-256-GCM |
| Signaling server | Go + gorilla/websocket |

## How To Run

```bash
# Run in development
pnpm tauri dev

# Build release app
pnpm tauri build
```

## Known Risks

- File contents must remain end-to-end encrypted; the signaling server should stay zero-knowledge.
- QUIC direct transfer and WebSocket relay fallback both need coverage when transport code changes.
- Preserve folder structure and multi-file behavior during transfer changes.
- Keep PR-template drift separate from protocol or app behavior.

## Next Recommended Move

Resolve PR-template drift separately, then verify direct QUIC, relay fallback, encryption handshake, progress UI, and folder transfer behavior before shipping changes.

<!-- portfolio-context:end -->
