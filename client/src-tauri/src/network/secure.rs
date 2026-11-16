// Authenticated peer channel (protocol 3.0.0).
//
// Every PeerMessage is MessagePack-encoded and sealed with AES-256-GCM under
// the key for its direction. The nonce is a per-direction counter, so a frame
// that is dropped, replayed, reordered, reflected back to its sender, or
// spliced in from another session fails to open and the transfer aborts.
// Direct QUIC and relay frames carry the same sealed bytes, so the guarantees
// do not depend on which transport the race in send.rs / receive.rs picked.

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};

use crate::crypto::session_keys::{Role, SessionKeys};
use crate::error::{AppError, AppResult};
use crate::network::transport::Transport;
use crate::protocol::messages::PeerMessage;

const FRAME_AAD: &[u8] = b"relay/3.0.0 frame";

pub struct SecureChannel<'a> {
    transport: &'a mut Transport,
    send_key: LessSafeKey,
    recv_key: LessSafeKey,
    send_seq: u64,
    recv_seq: u64,
}

impl<'a> SecureChannel<'a> {
    pub fn new(transport: &'a mut Transport, keys: &SessionKeys) -> AppResult<Self> {
        Ok(Self {
            transport,
            send_key: aead_key(&keys.send)?,
            recv_key: aead_key(&keys.recv)?,
            send_seq: 0,
            recv_seq: 0,
        })
    }

    /// Derive the directional keys for `role` from the SPAKE2 output and wrap
    /// the transport.
    pub fn from_shared(
        transport: &'a mut Transport,
        shared: &[u8; 32],
        role: Role,
    ) -> AppResult<(Self, SessionKeys)> {
        let keys = SessionKeys::derive(shared, role)?;
        Ok((Self::new(transport, &keys)?, keys))
    }

    pub async fn send(&mut self, msg: &PeerMessage) -> AppResult<()> {
        let frame = seal_frame(&self.send_key, self.send_seq, msg)?;
        self.send_seq = self
            .send_seq
            .checked_add(1)
            .ok_or_else(|| AppError::Crypto("frame counter exhausted".into()))?;
        self.transport.send_frame(frame).await
    }

    pub async fn recv(&mut self) -> AppResult<PeerMessage> {
        let frame = self.transport.recv_frame().await?;
        let msg = open_frame(&self.recv_key, self.recv_seq, frame)?;
        self.recv_seq = self
            .recv_seq
            .checked_add(1)
            .ok_or_else(|| AppError::Crypto("frame counter exhausted".into()))?;
        Ok(msg)
    }

    pub async fn finish_send(&mut self) -> AppResult<()> {
        self.transport.finish_send().await
    }
}

fn aead_key(bytes: &[u8; 32]) -> AppResult<LessSafeKey> {
    let unbound = UnboundKey::new(&AES_256_GCM, bytes)
        .map_err(|_| AppError::Crypto("failed to create frame key".into()))?;
    Ok(LessSafeKey::new(unbound))
}

fn frame_nonce(seq: u64) -> Nonce {
    let mut nonce = [0u8; 12];
    nonce[4..].copy_from_slice(&seq.to_be_bytes());
    Nonce::assume_unique_for_key(nonce)
}

fn seal_frame(key: &LessSafeKey, seq: u64, msg: &PeerMessage) -> AppResult<Vec<u8>> {
    let mut buf =
        rmp_serde::to_vec(msg).map_err(|e| AppError::Serialization(format!("encode: {e}")))?;
    key.seal_in_place_append_tag(frame_nonce(seq), Aad::from(FRAME_AAD), &mut buf)
        .map_err(|_| AppError::Crypto("frame seal failed".into()))?;
    Ok(buf)
}

fn open_frame(key: &LessSafeKey, seq: u64, mut frame: Vec<u8>) -> AppResult<PeerMessage> {
    let plain = key
        .open_in_place(frame_nonce(seq), Aad::from(FRAME_AAD), &mut frame)
        .map_err(|_| {
            AppError::Crypto(
                "peer frame failed authentication (tampered, replayed, reordered or wrong key)"
                    .into(),
            )
        })?;
    rmp_serde::from_slice(plain)
        .map_err(|e| AppError::Serialization(format!("failed to decode message: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(role: Role) -> SessionKeys {
        SessionKeys::derive(&[3u8; 32], role).unwrap()
    }

    #[test]
    fn frames_open_only_in_order_and_direction() {
        let s = keys(Role::Sender);
        let r = keys(Role::Receiver);
        let s_send = aead_key(&s.send).unwrap();
        let s_recv = aead_key(&s.recv).unwrap();
        let r_recv = aead_key(&r.recv).unwrap();

        let f0 = seal_frame(&s_send, 0, &PeerMessage::FileAccept).unwrap();
        let f1 = seal_frame(&s_send, 1, &PeerMessage::TransferComplete).unwrap();

        // in order, right direction
        assert!(matches!(
            open_frame(&r_recv, 0, f0.clone()).unwrap(),
            PeerMessage::FileAccept
        ));
        // replayed or reordered
        assert!(open_frame(&r_recv, 1, f0.clone()).is_err());
        assert!(open_frame(&r_recv, 0, f1.clone()).is_err());
        // reflected back to its sender
        assert!(open_frame(&s_recv, 0, f0.clone()).is_err());
        // tampered
        let mut bad = f1;
        bad[0] ^= 1;
        assert!(open_frame(&r_recv, 1, bad).is_err());
        // another session's key
        let other = SessionKeys::derive(&[4u8; 32], Role::Receiver).unwrap();
        assert!(open_frame(&aead_key(&other.recv).unwrap(), 0, f0).is_err());
    }
}
