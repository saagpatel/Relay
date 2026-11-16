use quinn::{RecvStream, SendStream};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// All messages exchanged between peers over QUIC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PeerMessage {
    /// Sender → Receiver: here's what I want to send.
    FileOffer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        protocol_version: Option<String>,
        files: Vec<FileInfo>,
    },

    /// Receiver → Sender: I accept the transfer.
    FileAccept,

    /// Receiver → Sender: I decline the transfer.
    FileDecline,

    /// Sender → Receiver: one encrypted chunk of file data.
    FileChunk {
        file_index: u16,
        chunk_index: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
        nonce: [u8; 12],
    },

    /// Sender → Receiver: file transfer complete, verify checksum.
    FileComplete { file_index: u16, sha256: [u8; 32] },

    /// Receiver → Sender: checksum verified.
    FileVerified { file_index: u16 },

    /// Either → Either: all files transferred successfully.
    TransferComplete,

    /// Either → Either: cancel the transfer.
    Cancel { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    /// Relative path within a folder transfer.
    pub relative_path: Option<String>,
}

/// Largest frame either peer accepts. Frames are buffered before they are
/// authenticated, so this bounds what an unauthenticated party can make a peer
/// allocate. The largest honest frames are a sealed 256 KiB chunk and an offer
/// whose names and paths are capped at 1 MiB (sender.rs / receiver.rs), both
/// well under 4 MiB.
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// Read one length-prefixed frame (a sealed PeerMessage) from a QUIC stream.
pub async fn read_frame(stream: &mut RecvStream) -> AppResult<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| AppError::Network(format!("failed to read message length: {e}")))?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(AppError::Transfer(format!(
            "message too large: {len} bytes"
        )));
    }

    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(|e| AppError::Network(format!("failed to read message payload: {e}")))?;
    Ok(payload)
}

/// Write one length-prefixed frame to a QUIC stream.
pub async fn write_frame(stream: &mut SendStream, frame: &[u8]) -> AppResult<()> {
    let len =
        u32::try_from(frame.len()).map_err(|_| AppError::Transfer("frame too large".into()))?;
    stream
        .write_all(&len.to_be_bytes())
        .await
        .map_err(|e| AppError::Network(format!("failed to write message length: {e}")))?;
    stream
        .write_all(frame)
        .await
        .map_err(|e| AppError::Network(format!("failed to write message payload: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_all_variants() {
        let messages = vec![
            PeerMessage::FileOffer {
                protocol_version: Some("3.0.0".into()),
                files: vec![FileInfo {
                    name: "test.txt".into(),
                    size: 1024,
                    relative_path: None,
                }],
            },
            PeerMessage::FileAccept,
            PeerMessage::FileDecline,
            PeerMessage::FileChunk {
                file_index: 0,
                chunk_index: 42,
                data: vec![1, 2, 3, 4],
                nonce: [0u8; 12],
            },
            PeerMessage::FileComplete {
                file_index: 0,
                sha256: [0xAB; 32],
            },
            PeerMessage::FileVerified { file_index: 0 },
            PeerMessage::TransferComplete,
            PeerMessage::Cancel {
                reason: "test".into(),
            },
        ];

        for msg in &messages {
            let encoded = rmp_serde::to_vec(msg).unwrap();
            let decoded: PeerMessage = rmp_serde::from_slice(&encoded).unwrap();
            // Just verify it doesn't panic — we can't easily PartialEq an enum with Vec<u8>
            let re_encoded = rmp_serde::to_vec(&decoded).unwrap();
            assert_eq!(encoded, re_encoded, "roundtrip failed for {msg:?}");
        }
    }
}
