// Transport abstraction — wraps either a direct QUIC stream or a relayed WebSocket connection.
//
// Both sender and receiver pipelines use `Transport` instead of raw QUIC streams,
// allowing seamless fallback from direct QUIC to relay mode.

use crate::error::{AppError, AppResult};
use crate::network::relay::RelayStream;

use quinn::{RecvStream, SendStream};

/// A bidirectional transport for exchanging PeerMessages.
pub enum Transport {
    /// Direct QUIC connection (LAN or public IP).
    Direct { send: SendStream, recv: RecvStream },
    /// Relayed through the signaling server's WebSocket.
    Relayed { ws: Box<RelayStream> },
}

impl Transport {
    /// Send one sealed frame. Callers go through `network::secure::SecureChannel`;
    /// nothing is ever sent unauthenticated.
    pub(crate) async fn send_frame(&mut self, frame: Vec<u8>) -> AppResult<()> {
        match self {
            Transport::Direct { send, .. } => {
                crate::protocol::messages::write_frame(send, &frame).await
            }
            Transport::Relayed { ws } => ws.send_frame(frame).await,
        }
    }

    /// Receive one sealed frame.
    pub(crate) async fn recv_frame(&mut self) -> AppResult<Vec<u8>> {
        match self {
            Transport::Direct { recv, .. } => crate::protocol::messages::read_frame(recv).await,
            Transport::Relayed { ws } => ws.recv_frame().await,
        }
    }

    /// Signal that we're done sending (QUIC finish / WebSocket close).
    pub async fn finish_send(&mut self) -> AppResult<()> {
        match self {
            Transport::Direct { send, .. } => {
                send.finish()
                    .map_err(|e| AppError::Network(format!("failed to finish stream: {e}")))?;
                Ok(())
            }
            Transport::Relayed { ws } => ws.close().await,
        }
    }
}
