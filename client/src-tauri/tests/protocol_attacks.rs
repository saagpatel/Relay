// Regression tests for the attacks found by the ProVerif model (formal/).
//
// Each test plays an on-path attacker who does NOT know the transfer code or
// the SPAKE2 key: the network between a peer and the signaling server, the
// direct QUIC path (certificates are not verified), or the relay. Before
// protocol 3.0.0 every one of these attacks succeeded.

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use relay_lib::network::relay::RelayStream;
use relay_lib::network::signaling::SignalingClient;
use relay_lib::network::transport::Transport;
use relay_lib::protocol::messages::{FileInfo, PeerMessage};
use relay_lib::protocol::version::CURRENT_PROTOCOL_VERSION;

type AttackerWs = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

/// Start a one-connection WebSocket server played by the attacker.
async fn attacker_endpoint() -> (String, tokio::task::JoinHandle<AttackerWs>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        tokio_tungstenite::accept_async(tcp).await.unwrap()
    });
    (url, handle)
}

/// A victim-side relay transport connected to the attacker.
async fn victim_transport() -> (Transport, AttackerWs) {
    let (url, handle) = attacker_endpoint().await;
    let (ws, _) = tokio_tungstenite::connect_async(format!("{url}/ws/7-guitar-palace"))
        .await
        .unwrap();
    let attacker = handle.await.unwrap();
    (
        Transport::Relayed {
            ws: Box::new(RelayStream::new(ws)),
        },
        attacker,
    )
}

/// Encode a peer message the way an unauthenticated 2.x peer frames it.
fn plaintext_frame(msg: &PeerMessage) -> Message {
    let payload = rmp_serde::to_vec(msg).unwrap();
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&payload);
    Message::Binary(frame.into())
}

async fn next_binary(ws: &mut AttackerWs) -> Option<Vec<u8>> {
    while let Some(Ok(msg)) = ws.next().await {
        if let Message::Binary(data) = msg {
            return Some(data.to_vec());
        }
    }
    None
}

/// Attack 1 (model query P4): with no key, the attacker offers a zero-length
/// file, claims sha256("") and completes. A 2.0.0 receiver stored the file
/// and reported a verified transfer.
#[tokio::test]
async fn receiver_rejects_forged_empty_file_transfer() {
    let (mut transport, mut attacker) = victim_transport().await;
    let save = tempfile::tempdir().unwrap();
    let (progress_tx, _progress_rx) = mpsc::unbounded_channel();
    let (accept_tx, accept_rx) = oneshot::channel();
    accept_tx.send(true).unwrap();

    let attack = tokio::spawn(async move {
        let empty_sha: [u8; 32] = {
            use sha2::{Digest, Sha256};
            Sha256::digest(b"").into()
        };
        for msg in [
            PeerMessage::FileOffer {
                protocol_version: Some(CURRENT_PROTOCOL_VERSION.to_string()),
                files: vec![FileInfo {
                    name: "planted.txt".into(),
                    size: 0,
                    relative_path: None,
                }],
            },
            PeerMessage::FileComplete {
                file_index: 0,
                sha256: empty_sha,
            },
            PeerMessage::TransferComplete,
        ] {
            if attacker.send(plaintext_frame(&msg)).await.is_err() {
                break;
            }
        }
        // Drain whatever the victim answers until it hangs up.
        while next_binary(&mut attacker).await.is_some() {}
    });

    // The attacker never learned the SPAKE2 output; the victim holds its own.
    let victim_key = [0x5a; 32];
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        relay_lib::transfer::receiver::run_receive(
            "victim".into(),
            save.path().to_path_buf(),
            &mut transport,
            victim_key,
            progress_tx,
            accept_rx,
            CancellationToken::new(),
        ),
    )
    .await
    .expect("receiver must not hang");
    drop(transport);
    attack.abort();

    assert!(
        result.is_err(),
        "receiver accepted a transfer forged without the key"
    );
    assert!(
        !save.path().join("planted.txt").exists(),
        "forged file was written to the destination"
    );
}

/// Attack 2 (model query P5): the attacker answers the sender's offer with
/// FileAccept and every FileComplete with FileVerified. A 2.0.0 sender
/// reported a delivered, verified transfer although no receiver got a byte.
#[tokio::test]
async fn sender_rejects_forged_delivery_confirmation() {
    let (mut transport, mut attacker) = victim_transport().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret.txt");
    std::fs::write(&path, b"quarterly numbers").unwrap();
    let (progress_tx, _progress_rx) = mpsc::unbounded_channel();

    let attack = tokio::spawn(async move {
        // offer
        if next_binary(&mut attacker).await.is_none() {
            return;
        }
        if attacker
            .send(plaintext_frame(&PeerMessage::FileAccept))
            .await
            .is_err()
        {
            return;
        }
        // chunk(s) and FileComplete: acknowledge as soon as the stream pauses
        let mut seen = 0;
        while let Ok(Some(_)) = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            next_binary(&mut attacker),
        )
        .await
        {
            seen += 1;
            if seen >= 2 {
                break;
            }
        }
        let _ = attacker
            .send(plaintext_frame(&PeerMessage::FileVerified {
                file_index: 0,
            }))
            .await;
        while next_binary(&mut attacker).await.is_some() {}
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        relay_lib::transfer::sender::run_send(
            "victim".into(),
            vec![path],
            vec![FileInfo {
                name: "secret.txt".into(),
                size: 17,
                relative_path: None,
            }],
            &mut transport,
            [0x5a; 32],
            progress_tx,
            CancellationToken::new(),
        ),
    )
    .await
    .expect("sender must not hang");
    drop(transport);
    attack.abort();

    assert!(
        result.is_err(),
        "sender accepted a delivery confirmation forged without the key"
    );
}

/// Attack 3 (model queries P3a/P3b): the attacker reflects a peer's own
/// encrypted certificate fingerprint back to it. Both directions used one key
/// with no role binding, so a 2.0.0 peer "confirmed" a key nobody else held.
#[tokio::test]
async fn key_confirmation_rejects_reflected_message() {
    let (url, handle) = attacker_endpoint().await;
    let reflector = tokio::spawn(async move {
        let mut ws = handle.await.unwrap();
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(_) = msg {
                if ws.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut client = SignalingClient::connect(&url, "7-guitar-palace")
        .await
        .unwrap();
    client.register("receiver", None).await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.exchange_cert_fingerprint(&[0x11; 32], &[0x5a; 32]),
    )
    .await
    .expect("confirmation must not hang");
    reflector.abort();

    assert!(
        result.is_err(),
        "a reflected confirmation message was accepted as the peer's"
    );
}

/// Found by the model-vs-code review: frames are read into memory before they
/// are authenticated, and the limit was 256 MiB, so anyone who reached the
/// QUIC listener or the relay could force huge allocations. A frame larger
/// than one sealed chunk or offer must be refused before it is buffered.
#[tokio::test]
async fn oversized_unauthenticated_frame_is_refused() {
    let (url, handle) = attacker_endpoint().await;
    let (ws, _) = tokio_tungstenite::connect_async(format!("{url}/ws/7-guitar-palace"))
        .await
        .unwrap();
    let mut attacker = handle.await.unwrap();
    let mut victim = RelayStream::new(ws);

    let len = 8 * 1024 * 1024u32;
    let mut frame = len.to_be_bytes().to_vec();
    frame.resize(4 + len as usize, 0);
    let attack = tokio::spawn(async move {
        let _ = attacker.send(Message::Binary(frame.into())).await;
        attacker
    });

    let result = tokio::time::timeout(std::time::Duration::from_secs(10), victim.recv_frame())
        .await
        .expect("victim must not hang");
    attack.abort();
    assert!(
        result.is_err(),
        "an 8 MiB unauthenticated frame was accepted"
    );
    assert!(relay_lib::protocol::messages::MAX_FRAME_BYTES < len as usize);
}
