# Formal model of the Relay pairing handshake

ProVerif models of Relay's SPAKE2 pairing, key confirmation and file transfer
over both transports (direct QUIC and the relay fallback).

- `relay_v2.pvl`: protocol 2.0.0, as implemented at `f43d036` (before this change).
- `relay_v3.pvl`: protocol 3.0.0, the fix in this change.
- `relay_vN.pv`, `relay_vN_fs.pv`, `relay_vN_server.pv`: query files (main
  properties, forward secrecy, malicious server).
- `relay_v3_codeholder.pv`: someone told the whole code (inherent; expected
  to fail).
- `relay_vN_reach.pv`: sanity queries showing that an honest run reaches
  confirmation, storage, delivery and completion, so the correspondence
  proofs are not vacuous.
- `results/summary.txt`: every verdict from `run.sh`;
  `results/v2_attack_traces.txt`: the 2.0.0 counterexample traces.

Run (ProVerif 2.05, installed with `brew install opam expat gtk+ pkgconf`,
`opam init`, `opam install proverif`):

```bash
PATH="$HOME/.opam/default/bin:$PATH" formal/run.sh
```

`run.sh` exits non-zero if any 3.0.0 property (including the malicious-server
case, since the nameplate split) is not proved, or if any sanity query fails to find an
honest run.

An earlier revision had `if sz = zero then ... else ...` without parentheses.
ProVerif binds a bare `else` to the innermost `if`, which made the
nonempty-file branch unreachable, so the 3.0.0 transfer proofs held
vacuously. The Codex review on the PR caught it. Both branches are now
parenthesized, and the `_reach.pv` queries guard against a repeat.

## Why ProVerif

ProVerif proves properties for an unbounded number of sessions automatically,
has a native `weaksecret` query for offline guessing of a low-entropy password
(the secret words of a code have 65,536 values), and handles the
ideal-cipher abstraction of a PAKE that its own EKE example uses. Tamarin has
better Diffie-Hellman support but no built-in offline-guessing query, and its
proofs here would need hand-written lemmas. Every Relay question in scope is a
secrecy or correspondence query, which is ProVerif's home ground.

## Attacker

Dolev-Yao on one channel that carries signaling, the direct QUIC stream and the
relay WebSocket. That is faithful to the code: the server forwards `spake2` and
`cert_fingerprint` frames to whoever is in the room (`server/handler.go:260`),
in 2.0.0 QUIC certificates were never verified (and the model keeps QUIC on
the attacker's channel even now that they are pinned), and
the relay forwards binary frames verbatim (`server/relay.go`). The transfer
code is not given to the attacker. The server cases differ by version:

- 2.0.0 (`relay_v2_server.pv`): the whole code was the WebSocket path
  `/ws/{code}`, so the signaling server, anyone on a `ws://` path and anyone
  reading server logs knew the SPAKE2 password. The model gives the attacker
  `pw`.
- 3.0.0 with the nameplate split (`relay_v3_server.pv`): the sender is
  allocated a nameplate on `/ws`, the receiver joins `/ws/{nameplate}`
  (`network/signaling.rs connect`, `allocate`; `server/handler.go`), and the
  words never leave the peers. The password is the whole code, modeled as
  `mkcode(nameplate, words)` with the nameplate given to the attacker.
- `relay_v3_codeholder.pv`: an attacker told the whole code. That breaks any
  code-based PAKE and is expected to fail.

The single channel makes the attacker stronger than reality on signaling: the
real server admits one sender and one receiver per room and forwards
`spake2`/`cert_fingerprint` only to the other peer, so injecting into a
victim's signaling stream needs the code, a broken `wss`, or the server
itself. On the direct QUIC path, before certificate pinning,
certificates were unverified and the sender
accepted the first QUIC connection that arrived. Both sides now pin the
other's certificate to the confirmed fingerprint; the model still treats
QUIC as attacker-controlled, which is conservative.

## Properties and results

| # | Property | Source of the intent | 2.0.0 | 3.0.0 |
|---|---|---|---|---|
| P1 | File contents stay secret from a network attacker without the code (contents modeled as unguessable; see note) | `docs/SECURITY.md` "Overview" | proved, but see note | proved |
| P2 | The code cannot be guessed offline from any number of observed or active sessions | SPAKE2 choice, `crypto/spake.rs:21` | proved | proved |
| P3a | Receiver finishing key confirmation implies a sender derived the same key | `network/signaling.rs` step 5 comment; `docs/ARCHITECTURE.md` "Prevents MITM attacks during key derivation" | **attack** | proved |
| P3b | Same, sender side | same | **attack** | proved |
| P4 | Whatever the receiver stores as verified (name, contents), over either transport, was offered by a sender with the same key | `docs/ARCHITECTURE.md` "Integrity Verification", "Authenticated encryption detects ciphertext tampering" | **attack** | proved |
| P5 | A sender that sees `FileVerified` talked to a receiver that stored exactly that file | `transfer/sender.rs` FileVerified handling | **attack** | proved |
| P6 | Replay: each completed receive matches a distinct sender session (injective) | chunk nonce/AAD design, `crypto/aes_gcm.rs` | **attack** | proved |
| P7 | Forward secrecy: leaking the code after the sessions keeps earlier contents secret | ephemeral SPAKE2 scalars | proved | proved |
| P8 | Contents secret, transfers authentic and words not guessable offline, against the signaling server (it knows the nameplate) | nameplate split, `docs/SECURITY.md` | attack (server knew the whole code) | proved |
| P8b | Same against someone told the whole code | inherent to a PAKE | attack | attack (inherent) |
| P9 | No downgrade through the relay fallback | `network/transport.rs` | holds | holds |

Note on P1 for 2.0.0: the model treats file contents as an unguessable value.
In 2.0.0 the SHA-256 of every file was sent in the clear in `FileComplete`, so
anyone on the path could confirm a guessed file offline. In 3.0.0 the
checksum, names and paths travel inside sealed frames. No query checks the
secrecy of names or checksums separately; it follows from the sealing, and
frame lengths still reveal sizes.

P9 is structural rather than a separate query: in both models the transport
(`mode`) is chosen by the attacker, and the same keys and framing are used on
both paths, so P1 to P7 hold whichever path the attacker forces. There is a
single cipher suite and protocol versions outside the same major are refused
(`protocol/version.rs`), so there is nothing weaker to downgrade to. An
attacker can still force relay mode (block QUIC or inject `relay_request`),
which exposes timing and sizes to the relay; that is documented.

## The counterexamples and why they are real

Traces are in `results/v2_attack_traces.txt`.

1. **Reflected key confirmation (P3a, P3b).** Needs the ability to inject into
   the victim's signaling stream (see Attacker above), so in practice a code
   holder, the server, or a broken `wss`. Both peers sealed their
   certificate fingerprint with the same SPAKE2 key, a random nonce and empty
   AAD (`aes_gcm.rs encrypt_one`, `signaling.rs exchange_cert_fingerprint` at
   `f43d036`). The trace sends the victim any SPAKE2 message, then returns the
   victim's own fingerprint ciphertext. It decrypts, so the victim "confirms" a
   key no peer holds. The other attacks below need no code and no server
   access: an on-path attacker on the unverified QUIC connection is enough. Reproduced by
   `tests/protocol_attacks.rs::key_confirmation_rejects_reflected_message`.
2. **Forged empty transfer (P4, P6).** Peer messages other than chunk payloads
   were plain MessagePack (`protocol/messages.rs`). The trace offers a
   zero-length file, sends `FileComplete(sha256(""))` and `TransferComplete`.
   No chunk is needed for a zero-length file, so the receiver stores an
   attacker-named file and reports success without the attacker knowing any
   key. On a real transfer the same attacker can rewrite the offer to zero
   bytes, drop the chunks and forward the rest, so both ends report success
   while the receiver gets an empty file. Reproduced by
   `receiver_rejects_forged_empty_file_transfer`.
3. **Forged delivery confirmation (P5).** `FileAccept` and `FileVerified` were
   unauthenticated, so the attacker can make a sender report a delivered,
   verified transfer. Reproduced by
   `sender_rejects_forged_delivery_confirmation`.

All three regression tests failed on `f43d036` and pass after the fix.

## The fix (protocol 3.0.0)

- `crypto/session_keys.rs`: HKDF-SHA256 splits the SPAKE2 output into a
  sender-to-receiver key, a receiver-to-sender key and a chunk key.
- `network/signaling.rs`: key confirmation seals with the key for one's own
  direction and binds the role in the AAD; the peer opens with the opposite
  direction and role. A reflected message no longer opens.
- `network/secure.rs`: every `PeerMessage` is sealed with AES-256-GCM under its
  direction key, nonce = per-direction counter. Forged, replayed, reordered,
  reflected or spliced frames fail to open and abort the transfer. A
  withheld suffix (for example the final `TransferComplete`) is not
  detected as an error; the receiver simply never completes. The model's
  properties are safety properties and say nothing about liveness. The relay
  and the QUIC path now carry only ciphertext, so file names and checksums are
  no longer visible to the relay.
- `protocol/version.rs`: version 3.0.0; clients refuse 2.x peers and 2.x
  servers (the refusal is client-side; `server/handler.go` only stamps 3.0.0
  and does not check client versions). The version is not bound into the key
  schedule; 3.0.0 does not depend on it, since a 2.x peer cannot produce
  frames that open.
- Found by the independent review, outside the model: frames are buffered
  before authentication, and the limit was 256 MiB. Peers now refuse frames
  over 4 MiB (`protocol/messages.rs MAX_FRAME_BYTES`, `network/relay.rs`, the
  WebSocket config in `network/signaling.rs`), and the server refuses peer
  messages over one frame (`server/handler.go`). The client no longer logs
  the transfer code.

## The nameplate/password split

- `server/handler.go`, `server/server.go AllocatePeer`: a sender connecting to
  `/ws` gets the smallest free nameplate (`allocated` message); a receiver may
  not allocate.
- `transfer/code.rs`: `TransferCode { nameplate, word1, word2 }`; `room()` is
  the nameplate alone. Words come from `generate(nameplate)` on the sender.
- `network/signaling.rs`: `connect(url, code)` parses the code and sends only
  `/ws/{nameplate}`; `allocate(url)` plus `wait_for_allocation()` for senders.
- Regression tests: `tests/protocol_attacks.rs::
  signaling_server_never_sees_the_password_words` (failed before: the server
  saw `/ws/7-guitar-tiger`), and `server/nameplate_test.go`.
- Online guessing remains: anyone can join a nameplate as the receiver and
  try one pair of words (1 in 65,536). Key confirmation then fails and both
  peers report it. ProVerif's `weaksecret` covers offline guessing only.

## Abstractions (where the model is coarser than the code)

- SPAKE2 blinding `X = xB + pw·S` is a keyed bijection (`blind`/`unblind`), the
  standard ideal-cipher abstraction. The symmetric transcript sorts the two
  messages (`spake2 0.4 ed25519.rs hash_symmetric`), modeled by a commutative
  `msgs`. Group-element malleability beyond this is not modeled.
- AES-GCM is an ideal AEAD; nonce reuse is not modeled. In the code, chunk
  nonces are unique per (file, chunk) under the chunk key, and frame nonces
  are a counter under each direction key. The confirmation uses a random
  96-bit nonce under the same direction key as the frames, so a collision with
  a frame nonce is possible only with negligible probability (about
  frames / 2^96), not impossible.
- One file of one chunk per session, with fixed `seq0..seq3` constants. The
  receiver's zero-length path is reachable only by the attacker in 3.0.0,
  since the honest sender in the model always sends a chunk. ProVerif does
  not check honest empty-file, multi-file or multi-chunk runs. The argument
  for those: every frame opens only under its direction key at exactly the
  next counter value (`network/secure.rs`), so each side acts only on a prefix
  of the other's honest stream, and the per-file checks in `receiver.rs` then
  apply to authentic messages.
- `FileDecline`, `Cancel`, mismatched indices, size and length bounds and
  other error paths are not modeled; they only make an honest party abort.
- `FileOffer.protocol_version` and the signaling `protocol_version` field are
  not modeled (see above).
- Certificate fingerprints are fresh secrets in the model but public in
  reality (any QUIC client sees the certificate). Known plaintext does not
  matter under an ideal AEAD, and both sides discard the peer's fingerprint.
- MessagePack decoding is modeled as free constructors (no parsing confusion).
- QUIC certificates are now pinned in both directions to the confirmed
  fingerprints (`network/quic.rs`), but the model still puts QUIC on the
  attacker's channel. The proved properties therefore do not depend on
  pinning; pinning adds transport-level authentication and stops a stranger
  from occupying the sender's QUIC listener.

## SPAKE2 test vectors

RFC 9382 vectors do not apply: Relay uses the `spake2` 0.4 crate's Ed25519
construction, which follows python-spake2 / magic-wormhole (its own `S`
constant, a sorted 160-byte transcript, no RFC 9382 key schedule or MAC), and
Wycheproof has no SPAKE2 vectors. The crate's own python-spake2 compatibility
vectors (`test_hash_symmetric`, `test_password_to_scalar`, `test_asymmetric`
and the rest, 17 tests) were run against the exact locked version, 0.4.0, and
pass. The new HKDF key schedule is pinned by
`session_keys::key_schedule_matches_rfc5869_reference`, whose expected values
were computed with an independent RFC 5869 implementation that was first
checked against RFC 5869 test case 1.
