# Relay

Relay is a desktop file transfer app. It tries a direct QUIC connection first and uses a WebSocket relay when direct connectivity is unavailable. File contents are encrypted end to end; the signaling server can see transfer metadata such as file names, sizes, relative paths, and checksums during relayed transfers.

## Quick Start

### Prerequisites

- Rust toolchain with rustfmt and Clippy
- Node.js and pnpm versions pinned by the repository
- Go version pinned by `server/go.mod`
- Tauri desktop prerequisites for your platform

### Install and check

```bash
pnpm --dir client install --frozen-lockfile
go -C server mod verify
make check
```

### Run the client

```bash
cd client
pnpm tauri dev
```

The client uses `ws://localhost:8080` as its default signaling URL. Run a local server with `cd server && go run .`, or set another signaling URL in Settings.

## How It Works

1. The sender creates a transfer code and registers with the signaling server.
2. The receiver enters the code and registers with the same server.
3. The peers use the code as the SPAKE2 password to derive a shared key.
4. They try direct QUIC first and use the signaling server's WebSocket relay if needed.
5. The receiver verifies each completed file using SHA-256.

## Status

### Stack

- Desktop: Tauri 2
- Frontend: Solid.js, TypeScript, Tailwind CSS 4
- Backend: Rust 2021
- Transports: QUIC and WebSocket relay
- Signaling server: Go and gorilla/websocket
- Encryption: SPAKE2 and AES-256-GCM

### Known risks

- QUIC peer certificates are not verified. The transfer code is both the rendezvous identifier and the shared password.
- The signaling server cannot decrypt file contents, but it can observe peer IP addresses, timing, file names, sizes, paths, and checksums in relay mode.
- Direct QUIC, relay fallback, multi-file transfers, and destination safety need continued coverage as transport and receiver code changes.

### Next steps

- Keep the protocol, receiver safety, and session lifecycle checks passing.
- Exercise direct and relayed transfers on supported desktop platforms.
- Keep self-hosting instructions aligned with the Go server configuration.

## Self-Hosting the Server

See [docs/SELF_HOSTING.md](docs/SELF_HOSTING.md) for local build, configuration, and reverse-proxy guidance.

## License

MIT
