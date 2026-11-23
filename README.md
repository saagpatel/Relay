# Relay

Relay is a desktop file transfer app. It tries a direct QUIC connection first and uses a WebSocket relay when direct connectivity is unavailable. File contents, names, sizes and relative paths are encrypted and authenticated end to end. The signaling server sees peer addresses, the code's room number, and the timing and sizes of messages it handles.

## Quick Start

### Prerequisites

- Rust toolchain pinned in `rust-toolchain.toml`, with rustfmt and Clippy
- Node.js version in `.node-version` and pnpm version in `client/package.json`
- Go version pinned by `server/go.mod`
- Tauri desktop prerequisites for your platform
- Make and a POSIX shell for `make check`

### Install and check

```bash
pnpm --dir client install --frozen-lockfile
go -C server mod verify
make check
```

### Run the client

Start the local signaling server with `cd server && go run .` in a separate terminal at the repository root. The client defaults to `ws://localhost:8080`. For transfers between computers, configure both clients in Settings to use a signaling server they can both reach.

Start the client from the repository root.

```bash
cd client
pnpm tauri dev
```

## How It Works

1. The sender asks the signaling server for a room number (the nameplate) and picks two words locally, forming a code like `4-guitar-tiger`.
2. The receiver enters the code. The client joins room `4` without sending the two words to the server.
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

- Peer authentication rests on the transfer code. Each side pins the other's QUIC certificate to the fingerprint confirmed under the SPAKE2 key; there is no certificate authority.
- Anyone who knows the code can impersonate a peer. The signaling server sees the nameplate. Guessing the two randomly chosen words online has a 1 in 65,536 chance per attempt, and a wrong guess fails key confirmation.
- Direct QUIC, relay fallback, multi-file transfers, and destination safety need continued coverage as transport and receiver code changes.

### Next steps

- Keep the protocol, receiver safety, and session lifecycle checks passing.
- Exercise direct and relayed transfers on supported desktop platforms.
- Keep self-hosting instructions aligned with the Go server configuration.

## Self-Hosting the Server

See [docs/SELF_HOSTING.md](docs/SELF_HOSTING.md) for local build, configuration, and reverse-proxy guidance.

## License

MIT
