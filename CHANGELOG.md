# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Security

- QUIC certificate pinning: each peer pins the other's self-signed certificate to the fingerprint confirmed under the SPAKE2 key (mutual TLS with real signature checks). The sender no longer hands the transfer to whichever QUIC client connects first, and the receiver refuses impostor servers.
- Nameplate/password split: the server allocates a short room number (the nameplate) and the code's words are generated on the sender and never sent to the server. A malicious signaling server can no longer impersonate peers. Codes now look like `4-guitar-tiger` with a 1-5 digit number. Senders must use the new `/ws` allocation route, so update the server and clients together.
- Protocol 3.0.0: every peer message is sealed under per-direction HKDF keys with a counter nonce, and key confirmation is bound to role and direction. An on-path attacker without the code could previously forge a "verified" zero-length transfer, fake delivery confirmations, and pass key confirmation by reflection. 2.x clients and servers are refused. Formal model in `formal/`.

- Initial release
