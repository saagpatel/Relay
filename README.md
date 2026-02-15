# Relay

Fast, secure, peer-to-peer file transfer. Direct LAN, automatic relay fallback, end-to-end encrypted.

## Features

### Core
- **Direct LAN transfers** — QUIC connections for blazing-fast local network speeds
- **Automatic relay fallback** — Works anywhere (home, office, across internet)
- **Zero-knowledge architecture** — Files never touch cloud; signaling server is encrypted-blind
- **End-to-end encrypted** — SPAKE2 + AES-256-GCM, verified SHA-256 checksums
- **Multi-file transfers** — Send entire folders with nested structure preserved

### UX
- **Real-time progress** — Live speed graph, ETA, per-file status
- **Drag & drop** — Drop files into app or use traditional file picker
- **Keyboard shortcuts** — Power-user workflow (Cmd+O, Cmd+V, Escape, Cmd+,)
- **Dark/Light theme** — System preference detection + manual toggle
- **Zero configuration** — No port forwarding or network setup required

### Platform
- **Cross-platform** — macOS, Linux (Windows coming soon)
- **Self-hostable** — Run your own signaling server (Docker, Fly.io, bare metal)

## How It Works

1. **Sender** creates a transfer code and starts listening
2. **Receiver** enters the code and connects to the signaling server
3. **Key exchange** via SPAKE2 protocol (password = transfer code)
4. **Connection attempt**: tries direct QUIC connection first
5. **Automatic fallback**: if QUIC fails (NAT/firewall), switches to encrypted WebSocket relay
6. **File transfer** happens over the secure connection (direct or relayed)

## Architecture

- **Client**: Tauri app (Rust backend + React/TypeScript frontend)
- **Server**: Go signaling server + WebSocket relay
- **Transport**: QUIC for direct transfers, WebSocket for relay fallback
- **Encryption**: SPAKE2 key exchange + AES-256-GCM chunk encryption

## Development

### Prerequisites
- Rust/Cargo
- Go 1.22+
- Node.js/pnpm
- macOS/Linux (Windows support TBD)

### Running the signaling server
```bash
cd server
go build -o relay-server .
./relay-server
```

Server flags:
- `--addr` — listen address (default: `:8080`)
- `--max-sessions` — max concurrent sessions (default: 1000)
- `--session-ttl` — session expiration (default: 1h)
- `--relay-rate-limit` — relay bandwidth limit in bytes/sec (default: 10 MB/s)

### Running the client
```bash
cd client
pnpm install
pnpm tauri dev
```

### Testing
```bash
# Go server tests (with race detector)
cd server && go test -race ./...

# Rust unit tests
cd client/src-tauri && cargo test

# Rust integration tests (requires server binary at server/relay-server)
cd client/src-tauri && cargo test --test signaling_e2e

# Frontend build check
cd client && pnpm build
```

## Status

✅ **Production-ready**

**Completed Phases:**
- Phase 1: Direct QUIC transfers ✓
- Phase 2: Signaling + key exchange ✓
- Phase 3: Relay fallback + folders ✓
- Phase 4: Polish + distribution ✓

**All 43 tests passing** (26 Rust unit + 5 integration + 12 Go)

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — System design, data flow, module responsibilities
- [Security Model](docs/SECURITY.md) — Cryptography, threat model, vulnerability disclosure
- [Self-Hosting Guide](docs/SELF_HOSTING.md) — Deploy your own signaling server
- [Troubleshooting](docs/TROUBLESHOOTING.md) — Common issues and debugging

## Keyboard Shortcuts

- **Cmd+O / Ctrl+O** — Open file picker
- **Cmd+V / Ctrl+V** — Paste transfer code from clipboard
- **Escape** — Cancel transfer or go back
- **Cmd+, / Ctrl+,** — Toggle settings

## Deployment

### Self-Hosting (Docker)

```bash
docker run -d -p 8080:8080 \
  -e RELAY_MAX_SESSIONS=5000 \
  ghcr.io/your-org/relay-server:latest
```

See [Self-Hosting Guide](docs/SELF_HOSTING.md) for full details.

### CI/CD

GitHub Actions automatically:
- Runs tests on every push
- Builds platform-specific releases (macOS, Linux)
- Publishes Docker images on release tags

## Contributing

Contributions welcome! Please:
1. Search [existing issues](https://github.com/your-org/relay/issues)
2. Open a PR with tests
3. Follow existing code style

## License

MIT
