# Security Model

## Overview

Relay encrypts file contents between peers with AES-256-GCM using a key derived through SPAKE2. The signaling server forwards signaling data and, during relay transfers, encrypted file frames. It does not receive plaintext file contents. It can observe peer network addresses, transfer timing, and file metadata including names, sizes, relative paths, and checksums.

## Key exchange and peer authentication

The human-readable transfer code is both the signaling rendezvous identifier and the SPAKE2 shared password. Anyone who obtains the code can attempt to join the session and knows the password used for key exchange. Share it only with the intended recipient.

QUIC uses self-signed peer certificates, and the clients do not verify those certificates. The code's SPAKE2 exchange is not bound to the QUIC certificate fingerprints. The current design therefore does not provide certificate-based peer identity or a verified QUIC peer binding. A malicious signaling service or active network attacker may be able to interfere with peer negotiation. Do not treat the transport as providing protection against every active man-in-the-middle attack.

## File encryption and integrity

- File chunks use AES-256-GCM with nonces derived from the file and chunk indices. Those indices are also authenticated as associated data.
- Files are split into 256 KiB plaintext chunks.
- Each file has a SHA-256 checksum that the receiver checks before finalizing it.
- The Rust peer-message decoder rejects serialized messages larger than 256 MiB. Transfer offers are also bounded by file-count and metadata limits.

These mechanisms provide confidentiality and tamper detection for chunk payloads when the peers have established the intended shared key. They do not hide transfer metadata from the signaling server in relay mode.

## Destination handling

The receiver writes incoming files to the selected save directory. When no directory is configured, the current UI uses `/tmp/relay-received`. The receiver validates offered paths and uses temporary partial files before making verified outputs available. Existing destination files must not be truncated or removed by a failed or cancelled transfer.

## Known limitations

- Peer certificates are not verified, and key confirmation or authenticated control messages are not part of the current protocol.
- The transfer code is both the rendezvous ID and password; avoid sharing it through an untrusted channel.
- The signaling server sees participant IP addresses and transfer metadata. Relay mode also exposes timing and message sizes.
- The server's `/health` endpoint is not an authentication or authorization boundary.

## Reporting a Vulnerability

Please use the repository's GitHub security reporting channel if available. Avoid posting exploitable details in a public issue before a fix is available.
