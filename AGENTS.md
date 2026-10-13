# Repository Guide

## Purpose

Relay is a desktop peer-to-peer file transfer app. It uses QUIC for direct transfers and a Go WebSocket service for signaling and encrypted relay. Keep file contents end-to-end encrypted and treat signaling data and file metadata as visible to the service.

## Layout

- `client/src/`: Solid.js user interface
- `client/src-tauri/`: Tauri shell and Rust transfer, protocol, crypto, and networking code
- `server/`: Go signaling and relay service
- `docs/`: architecture, security, self-hosting, troubleshooting, and local smoke guidance
- `.github/workflows/`: CI and release workflows

## Setup

- Use the Node.js, pnpm, Rust, and Go versions pinned in the repository.
- Install client dependencies with `pnpm --dir client install --frozen-lockfile`.
- Verify Go modules with `cd server && go mod verify`.
- Install the platform prerequisites documented by Tauri before building or running the desktop app.

## Checks

- Run `make check` from the repository root for the aggregate local checks.
- For desktop integration, use `cd client && pnpm tauri dev` with a local signaling server (`cd server && go run .`).
- Use `docs/LOCAL_SMOKE.md` for optional macOS desktop validation.
- Report each command's actual result. A local check does not establish hosted CI, deployment, or user acceptance.

## Boundaries

- Preserve end-to-end file encryption, folder structure, and multi-file transfer behavior.
- Do not claim that QUIC peer certificates are verified: clients currently accept unverified self-signed certificates.
- The transfer code is both the rendezvous identifier and SPAKE2 password; do not describe it as an independently authenticated identity.
- The default client signaling URL is `ws://localhost:8080`; do not imply a hosted default exists.
- Keep receiver writes inside the chosen destination and preserve existing files on failed or cancelled transfers.
- Keep changes scoped to the requested behavior and preserve unrelated work.

## Delivery

- Read the relevant code and tests before changing protocol, receiver, or server behavior.
- Add or update regression coverage for behavior changes and run the smallest relevant checks, followed by `make check` when feasible.
- Keep documentation aligned with implemented behavior. Do not claim provider, hosted-CI, deployment, release-signing, or user-validation results without direct evidence.
- Review `git diff --check` and the changed-path list before handing off.
