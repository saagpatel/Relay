# Relay Architecture

## System Overview

Relay is a peer-to-peer file transfer application with automatic connection fallback.

```
┌──────────────────┐                    ┌──────────────────┐
│   Sender Client  │                    │ Receiver Client  │
│  (Tauri + Rust)  │                    │  (Tauri + Rust)  │
└────────┬─────────┘                    └─────────┬────────┘
         │                                        │
         │  1. Register (code)                    │
         ├───────────────────┐   ┌───────────────┤
         │                   ▼   ▼                │
         │          ┌─────────────────┐           │
         │          │ Signaling Server│           │
         │          │   (Go + WebSocket)          │
         │          └─────────────────┘           │
         │                   │                    │
         │  2. Exchange keys (SPAKE2)            │
         │◄──────────────────┼──────────────────►│
         │                                        │
         │  3. Try direct QUIC connection        │
         │◄─────────────────────────────────────►│
         │                                        │
         │  4. Fallback to relay if QUIC fails   │
         │◄────── (encrypted via server) ───────►│
         │                                        │
         │  5. Transfer encrypted file chunks    │
         │◄─────────────────────────────────────►│
         │                                        │
         └────────────────────────────────────────┘
```

## Layer Architecture

### 1. Frontend Layer (Solid.js + TypeScript)
**Location**: `client/src/`

- **Components**: Dumb UI components (`SendView.tsx`, `ReceiveView.tsx`, `TransferProgress.tsx`)
- **Stores**: Solid.js reactive stores (`transfer.ts`, `settings.ts`)
- **State Machine**: App.tsx orchestrates phases (idle → selecting → waiting → transferring → completed)
- **Events**: Listens to Tauri backend events via `transfer:progress` channel

**Key Files**:
- `App.tsx` — Main state machine
- `stores/transfer.ts` — Transfer state (connection type, progress, speed history)
- `stores/settings.ts` — User settings (signal server URL, theme)
- `components/SpeedGraph.tsx` — Canvas-based real-time speed visualization

### 2. Tauri Backend Layer (Rust)
**Location**: `client/src-tauri/src/`

#### Crypto Module
- `crypto/spake.rs` — SPAKE2 password-authenticated key exchange
- `crypto/aes_gcm.rs` — AES-256-GCM symmetric encryption with per-file, per-chunk nonces
- `crypto/checksum.rs` — SHA-256 file integrity verification

#### Protocol Module
- `protocol/messages.rs` — MessagePack wire protocol (10 message types)
- `protocol/chunker.rs` — File chunking (256 KB chunks) with streaming encryption
- `protocol/reassembler.rs` — Chunk decryption + reassembly with path sanitization

#### Network Module
- `network/quic.rs` — QUIC endpoint (self-signed TLS, custom cert verifier)
- `network/signaling.rs` — WebSocket client for peer discovery
- `network/relay.rs` — WebSocket relay for encrypted forwarding
- `network/transport.rs` — Abstraction over Direct (QUIC) vs Relayed (WebSocket)

#### Transfer Module
- `transfer/sender.rs` — Sender pipeline: offer → chunks → verify
- `transfer/receiver.rs` — Receiver pipeline: accept → chunks → verify
- `transfer/code.rs` — Human-friendly code generation (digit-word-word)
- `transfer/session.rs` — Session state machine (WaitingForPeer → Exchanging → Connecting → Transferring → Completed)
- `transfer/progress.rs` — Real-time speed/ETA tracking (3-second sliding window)

#### Commands Module (Tauri IPC)
- `commands/send.rs` — `start_send` handler
- `commands/receive.rs` — `start_receive`, `accept_transfer` handlers
- `commands/transfer.rs` — `cancel_transfer` handler

### 3. Signaling Server (Go)
**Location**: `server/`

- **Session Management**: In-memory map with TTL cleanup (10 min default)
- **Peer Registration**: Code-based pairing (sender + receiver)
- **Message Forwarding**: SPAKE2 messages, cert fingerprints
- **Relay Negotiation**: Both peers must agree to relay mode
- **Binary Relay**: Rate-limited (10 MB/s default) bidirectional forwarding

**Key Files**:
- `main.go` — CLI + server initialization
- `server.go` — Session map, capacity limits, TTL
- `handler.go` — WebSocket upgrade, peer registration, message routing
- `relay.go` — Token bucket rate limiter + relay loop
- `session.go` — Thread-safe session/peer data structures

## Data Flow

### File Transfer Sequence

1. **Code Generation** (Sender)
   - User selects files → `start_send` command
   - Generate transfer code (e.g., `42-elephant-zebra`)
   - Register with signaling server as "sender"
   - Listen on QUIC port

2. **Peer Discovery** (Receiver)
   - User enters code → `start_receive` command
   - Register with signaling server as "receiver"
   - Server notifies sender: "peer connected"

3. **Key Exchange** (Both)
   - Exchange SPAKE2 messages via signaling server
   - Derive a shared key from the transfer code, which is both the rendezvous ID and password
   - Exchange QUIC certificate fingerprints as signaling data; peers do not verify the certificates

4. **Connection Negotiation** (Both)
   - **Try Direct**: Receiver connects to sender's QUIC endpoint
   - **On Success**: Direct P2P transfer (LAN speed)
   - **On Failure**: Both request relay from server
   - Server accepts relay → establishes relay loop

5. **File Transfer** (Both)
   - Sender offers files (metadata: name, size, path)
   - Receiver accepts/declines
   - For each file:
     - Sender reads 256 KB chunk → encrypts → sends
     - Receiver receives → decrypts → writes to disk
   - After all chunks: verify SHA-256 checksum
   - Repeat for all files

6. **Completion** (Both)
   - All files verified → `TransferComplete` event
   - Frontend shows summary (file count, bytes, duration, speed)

## Security Model

### Threat Model
- **Users trust each other**: Share the transfer code only with the intended peer. It is both the rendezvous identifier and the SPAKE2 password.
- **Signaling server**: Cannot decrypt file contents, but sees peer IPs and file metadata; relay mode exposes timing and frame sizes.
- **Peer identity**: QUIC peer certificates are self-signed and not verified. SPAKE2 is not bound to the QUIC certificate fingerprints, so active peer impersonation is not ruled out.

### Encryption Layers

1. **Key Exchange**: SPAKE2 (symmetric, password = transfer code)
   - Prevents MITM attacks during key derivation
   - Both parties must know the code

2. **Transport Encryption**: AES-256-GCM
   - Chunk-level encryption with nonces derived from file and chunk indices
   - File and chunk indices are authenticated as associated data
   - Authenticated encryption detects ciphertext tampering

3. **Integrity Verification**: SHA-256
   - Per-file checksum computed during transfer
   - Verified after all chunks received
   - Mismatch triggers error + cleanup

### Input Validation

- **Path Traversal**: Validates and confines output paths to the selected save directory
- **Code Validation**: Enforces digit-word-word format
- **Offer Limits**: File count and metadata size are bounded
- **Message Length**: Rust peer-message decoder rejects messages above 256 MiB
- **Signaling route**: WebSocket connections use `/ws/{code}`; the default client URL is `ws://localhost:8080`
- **Destination default**: When no save directory is configured, the UI uses `/tmp/relay-received`

## Performance Characteristics

- **Chunk Size**: 256 KB (optimal for Tokio + QUIC)
- **Memory**: Streaming I/O (no full file loads)
- **Speed Tracking**: 3-second sliding window for smoothing
- **Graph Update**: Every 500ms (30-second history)
- **Concurrency**: Async throughout (Tokio multi-threaded)

## Deployment

- **Client**: Tauri desktop app (macOS, Linux)
- **Server**: Self-hostable Go signaling and relay service
- **Health**: GET /health endpoint (active session count)
- **Monitoring**: Tracing logs (info level)
