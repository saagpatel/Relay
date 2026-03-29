# Relay

[![Rust](https://img.shields.io/badge/rust-%23dea584?style=flat-square&logo=rust)](#) [![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](#)

> Zero-config, zero-cloud file transfer that just works — blazing-fast on your LAN, still works anywhere else.

Relay sends files peer-to-peer using QUIC for direct local-network speed and falls back automatically to an encrypted WebSocket relay when NAT or firewalls block a direct connection. Files are end-to-end encrypted with SPAKE2 key exchange and AES-256-GCM; the signaling server never sees your data.

## Features

- **Direct QUIC transfers** — sub-second LAN speeds with zero port-forwarding
- **Automatic relay fallback** — works across the internet or behind restrictive firewalls
- **Zero-knowledge signaling** — server is cryptographically blind to file contents
- **Multi-file and folder support** — nested directory structure preserved
- **Real-time progress UI** — live speed graph, ETA, and per-file status in a Solid.js interface

## Quick Start

### Prerequisites
- Rust stable toolchain
- Node.js 20+ and pnpm
- Tauri CLI v2

### Installation
```bash
git clone https://github.com/saagpatel/Relay
cd Relay/client
pnpm install
```

### Usage
```bash
# Run in development
pnpm tauri dev

# Build release app
pnpm tauri build
```

## How It Works

1. Sender creates a transfer code and starts listening
2. Receiver enters the code; both connect to the signaling server
3. SPAKE2 key exchange using the transfer code as the shared password
4. Direct QUIC connection attempted first; encrypted WebSocket relay used as fallback
5. File transfer proceeds over the secure channel

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop shell | Tauri 2 |
| Frontend | Solid.js + TypeScript + Tailwind CSS 4 |
| Backend | Rust 2021 |
| Transport | QUIC (direct) + WebSocket relay (fallback) |
| Encryption | SPAKE2 key exchange + AES-256-GCM |
| Signaling server | Go + gorilla/websocket |

## Self-Hosting the Server

The `server/` directory contains a Go signaling server. Deploy it on Fly.io, Docker, or any bare-metal host:

```bash
cd server
go build -o relay-server .
./relay-server
```

## License

MIT
