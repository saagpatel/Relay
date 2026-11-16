# Security Model

## Overview

Relay encrypts file contents between peers with AES-256-GCM using keys derived through SPAKE2. Since protocol 3.0.0 every peer message (offer, accept, chunks, checksums, acknowledgements) is sealed and authenticated, on the direct QUIC path and through the relay alike. A signaling server that only forwards traffic sees peer network addresses, transfer timing and frame sizes, but not file names, paths, checksums or contents. Because the server learns the transfer code (see below), a malicious server can still impersonate each peer to the other; nothing in the protocol protects against that.

## Key exchange and peer authentication

The human-readable transfer code is both the signaling rendezvous identifier and the SPAKE2 shared password. Anyone who obtains the code can attempt to join the session and knows the password used for key exchange. Share it only with the intended recipient.

After SPAKE2, HKDF-SHA256 splits the shared secret into one key per direction and a chunk key (`crypto/session_keys.rs`). Each peer then sends its QUIC certificate fingerprint sealed with its own direction key and its role as associated data. Opening the peer's message is explicit key confirmation: it proves the peer derived the same key in the opposite role, and a reflected or forged message fails.

QUIC uses self-signed peer certificates, and the clients do not verify those certificates; the exchanged fingerprints are not used to pin them. Integrity therefore comes from the sealed peer channel (`network/secure.rs`), not from QUIC: every frame is opened with the direction key and a per-direction counter nonce, so an on-path attacker who does not know the code cannot forge, replay, reorder, reflect or drop frames without the transfer failing. A party that knows the code (including the signaling server and anyone who can read a `ws://` signaling URL) can run its own SPAKE2 exchanges and act as a man in the middle.

The ProVerif models in `formal/` check these properties; see `formal/README.md` for what is proved and under which abstractions.

## File encryption and integrity

- File chunks use AES-256-GCM with nonces derived from the file and chunk indices. Those indices are also authenticated as associated data.
- Files are split into 256 KiB plaintext chunks.
- Each file has a SHA-256 checksum that the receiver checks before finalizing it.
- Peers refuse frames larger than 4 MiB before buffering them, and the server refuses peer messages larger than one frame. Transfer offers are also bounded by file-count and metadata limits.

These mechanisms provide confidentiality and tamper detection for chunk payloads and transfer metadata when the peers have established the intended shared key. Relay mode still exposes timing and frame sizes.

## Destination handling

The receiver writes incoming files to the selected save directory. When no directory is configured, the current UI uses `/tmp/relay-received`. The receiver validates offered paths and uses temporary partial files before making verified outputs available. Existing destination files must not be truncated or removed by a failed or cancelled transfer.

## Known limitations

- Peer certificates are not verified or pinned; peer authentication rests entirely on the transfer code.
- The transfer code is both the rendezvous ID and password; avoid sharing it through an untrusted channel.
- The signaling server sees participant IP addresses and the transfer code. Relay mode also exposes timing and message sizes.
- Protocol 3.0.0 peers and servers refuse 2.x; clients and self-hosted servers must be upgraded together.
- The server's `/health` endpoint is not an authentication or authorization boundary.

## Reporting a Vulnerability

Please use the repository's GitHub security reporting channel if available. Avoid posting exploitable details in a public issue before a fix is available.
