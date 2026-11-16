//! Protocol 3.0.0 key schedule.
//!
//! The 32-byte SPAKE2 output is never used directly. HKDF-SHA256 splits it
//! into one key per direction plus a separate chunk key, so a message sealed by
//! one peer can never be opened as if the other peer had sent it (no
//! reflection), and the chunk layer and the frame layer never share a key.

use ring::hkdf;

use crate::error::{AppError, AppResult};

const SALT: &[u8] = b"relay/3.0.0 session";
const SENDER_TO_RECEIVER: &[u8] = b"relay/3.0.0 sender->receiver";
const RECEIVER_TO_SENDER: &[u8] = b"relay/3.0.0 receiver->sender";
const CHUNKS: &[u8] = b"relay/3.0.0 file chunks";

/// Which end of the transfer this peer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Sender,
    Receiver,
}

impl Role {
    pub fn parse(role: &str) -> AppResult<Self> {
        match role {
            "sender" => Ok(Self::Sender),
            "receiver" => Ok(Self::Receiver),
            other => Err(AppError::Crypto(format!("unknown role '{other}'"))),
        }
    }

    pub fn label(self) -> &'static [u8] {
        match self {
            Self::Sender => b"sender",
            Self::Receiver => b"receiver",
        }
    }

    pub fn peer(self) -> Self {
        match self {
            Self::Sender => Self::Receiver,
            Self::Receiver => Self::Sender,
        }
    }
}

/// Keys derived from one SPAKE2 run, seen from one role.
pub struct SessionKeys {
    /// Seals what this peer sends.
    pub send: [u8; 32],
    /// Opens what the other peer sends.
    pub recv: [u8; 32],
    /// AES-GCM key for file chunks (sender encrypts, receiver decrypts).
    pub chunk: [u8; 32],
}

impl SessionKeys {
    pub fn derive(shared: &[u8; 32], role: Role) -> AppResult<Self> {
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, SALT).extract(shared);
        let s2r = expand(&prk, SENDER_TO_RECEIVER)?;
        let r2s = expand(&prk, RECEIVER_TO_SENDER)?;
        let chunk = expand(&prk, CHUNKS)?;
        let (send, recv) = match role {
            Role::Sender => (s2r, r2s),
            Role::Receiver => (r2s, s2r),
        };
        Ok(Self { send, recv, chunk })
    }
}

fn expand(prk: &hkdf::Prk, label: &[u8]) -> AppResult<[u8; 32]> {
    let info = [label];
    let okm = prk
        .expand(&info, hkdf::HKDF_SHA256)
        .map_err(|_| AppError::Crypto("HKDF expand failed".into()))?;
    let mut out = [0u8; 32];
    okm.fill(&mut out)
        .map_err(|_| AppError::Crypto("HKDF fill failed".into()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_mirror_and_differ() {
        let shared = [9u8; 32];
        let s = SessionKeys::derive(&shared, Role::Sender).unwrap();
        let r = SessionKeys::derive(&shared, Role::Receiver).unwrap();
        assert_eq!(s.send, r.recv);
        assert_eq!(s.recv, r.send);
        assert_eq!(s.chunk, r.chunk);
        assert_ne!(s.send, s.recv);
        assert_ne!(s.send, s.chunk);
        assert_ne!(s.send, shared);
    }

    fn unhex(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
        }
        out
    }

    /// Known answers computed independently with Python's hmac/hashlib
    /// implementation of RFC 5869 (checked first against RFC 5869 test case 1),
    /// so the wire key schedule cannot drift silently.
    #[test]
    fn key_schedule_matches_rfc5869_reference() {
        let s = SessionKeys::derive(&[9u8; 32], Role::Sender).unwrap();
        assert_eq!(
            s.send,
            unhex("ac4e37b695d9f99ca45c068f368518a83a1c2b69038345841958060a7a2f416f")
        );
        assert_eq!(
            s.recv,
            unhex("16db2ce6f7e3ab809929b72333c79c3d090b53eb8ec3394759ef68f8203d0a0f")
        );
        assert_eq!(
            s.chunk,
            unhex("5009c7b77fd939376f5942a5aaec7f484ca2b0980341aeefb69bd5ab34515ec3")
        );
    }

    #[test]
    fn role_parse() {
        assert_eq!(Role::parse("sender").unwrap(), Role::Sender);
        assert_eq!(Role::parse("receiver").unwrap(), Role::Receiver);
        assert!(Role::parse("relay").is_err());
    }
}
