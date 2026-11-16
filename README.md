# Relay

Relay is a desktop file transfer app. It tries a direct QUIC connection first and uses a WebSocket relay when direct connectivity is unavailable. File contents and transfer metadata are encrypted and authenticated end to end; the signaling server sees peer addresses, the code's room number, timing and frame sizes, but never the code's words.

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

1. The sender asks the signaling server for a room number (the nameplate) and picks two words locally, forming a code like `4-guitar-tiger`.
2. The receiver enters the code; only the nameplate `4` is sent to the server.
3. The peers use the whole code as the SPAKE2 password to derive a shared key.
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

- QUIC peer certificates are not verified. Peer authentication rests on the transfer code.
- The signaling server cannot read file contents or names and never learns the code's words, so it cannot impersonate a peer except by guessing the words online (1 in 65,536 per attempt, and a wrong guess fails key confirmation visibly). It observes peer IP addresses, the nameplate, timing and frame sizes.
- Direct QUIC, relay fallback, multi-file transfers, and destination safety need continued coverage as transport and receiver code changes.

### Next steps

- Keep the protocol, receiver safety, and session lifecycle checks passing.
- Exercise direct and relayed transfers on supported desktop platforms.
- Keep self-hosting instructions aligned with the Go server configuration.

## Self-Hosting the Server

See [docs/SELF_HOSTING.md](docs/SELF_HOSTING.md) for local build, configuration, and reverse-proxy guidance.

## License

MIT
