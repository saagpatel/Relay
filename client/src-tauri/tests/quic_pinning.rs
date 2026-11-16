// QUIC certificate pinning to the fingerprints confirmed during key
// confirmation. Before this, both sides accepted any certificate, and the
// sender accepted whichever QUIC client connected first.

use std::net::SocketAddr;
use std::time::Duration;

use relay_lib::network::quic::QuicEndpoint;

fn loopback(ep: &QuicEndpoint) -> SocketAddr {
    format!("127.0.0.1:{}", ep.local_addr().unwrap().port())
        .parse()
        .unwrap()
}

/// The receiver must refuse a QUIC server whose certificate is not the
/// sender's confirmed one, even if that server would accept the receiver.
#[tokio::test]
async fn receiver_rejects_impostor_server() {
    let sender = QuicEndpoint::new(0).await.unwrap();
    let receiver = QuicEndpoint::new(0).await.unwrap();
    let impostor = QuicEndpoint::new(0).await.unwrap();
    impostor.pin_peer(receiver.cert_fingerprint()).unwrap();
    let impostor_addr = loopback(&impostor);
    let accept = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(5), impostor.accept_peer()).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        receiver.connect(impostor_addr, sender.cert_fingerprint()),
    )
    .await
    .expect("connect must not hang");
    accept.abort();
    assert!(
        result.is_err(),
        "receiver accepted an impostor's certificate"
    );
}

/// The sender must not hand its transfer to a client that is not the
/// confirmed receiver, even one that connects first and knows the sender's
/// (public) fingerprint. The real receiver still gets through afterwards.
#[tokio::test]
async fn sender_rejects_impostor_client_and_accepts_receiver() {
    let sender = QuicEndpoint::new(0).await.unwrap();
    let receiver = QuicEndpoint::new(0).await.unwrap();
    let impostor = QuicEndpoint::new(0).await.unwrap();
    sender.pin_peer(receiver.cert_fingerprint()).unwrap();
    let sender_addr = loopback(&sender);
    let sender_fp = sender.cert_fingerprint();
    let receiver_port = receiver.local_addr().unwrap().port();

    let accept = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(10), sender.accept_peer())
            .await
            .expect("sender never accepted the receiver")
            .unwrap()
            .remote_address()
    });

    // The impostor goes first; its handshake must be refused by the sender.
    let impostor_conn = impostor.connect(sender_addr, sender_fp).await;
    if let Ok(conn) = &impostor_conn {
        // TLS 1.3 lets a client finish before the server checks its
        // certificate; the server then closes the connection.
        let closed = tokio::time::timeout(Duration::from_secs(5), conn.closed()).await;
        assert!(closed.is_ok(), "sender kept the impostor's connection open");
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let _conn = receiver.connect(sender_addr, sender_fp).await.unwrap();
    let accepted_from = accept.await.unwrap();
    assert_eq!(
        accepted_from.port(),
        receiver_port,
        "sender accepted a connection that was not the receiver's"
    );
}

/// Before `pin_peer`, the sender accepts nobody.
#[tokio::test]
async fn unpinned_sender_accepts_nobody() {
    let sender = QuicEndpoint::new(0).await.unwrap();
    let client = QuicEndpoint::new(0).await.unwrap();
    let sender_addr = loopback(&sender);
    let sender_fp = sender.cert_fingerprint();

    let accept = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(3), sender.accept_peer())
            .await
            .is_ok()
    });
    let _ = client.connect(sender_addr, sender_fp).await;
    assert!(
        !accept.await.unwrap(),
        "an unpinned sender accepted a connection"
    );
}

/// Fingerprints and certificates are public. An impostor presenting the
/// receiver's exact certificate, but signing the handshake with a different
/// key, must still be refused: pinning relies on real signature checks.
#[tokio::test]
async fn copied_certificate_without_its_key_is_refused() {
    use std::sync::Arc;

    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::client::ResolvesClientCert;
    use rustls::pki_types::{
        CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime,
    };
    use rustls::sign::CertifiedKey;
    use rustls::{DigitallySignedStruct, SignatureScheme};

    #[derive(Debug)]
    struct AnyServer;
    impl ServerCertVerifier for AnyServer {
        fn verify_server_cert(
            &self,
            _: &CertificateDer<'_>,
            _: &[CertificateDer<'_>],
            _: &ServerName<'_>,
            _: &[u8],
            _: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    /// Presents the stolen certificate with the impostor's own key.
    #[derive(Debug)]
    struct Stolen(Arc<CertifiedKey>);
    impl ResolvesClientCert for Stolen {
        fn resolve(&self, _: &[&[u8]], _: &[SignatureScheme]) -> Option<Arc<CertifiedKey>> {
            Some(self.0.clone())
        }
        fn has_certs(&self) -> bool {
            true
        }
    }

    let sender = QuicEndpoint::new(0).await.unwrap();
    let receiver = QuicEndpoint::new(0).await.unwrap();
    sender.pin_peer(receiver.cert_fingerprint()).unwrap();
    let sender_addr = loopback(&sender);

    let impostor_key = rcgen::KeyPair::generate().unwrap();
    let signing = rustls::crypto::ring::sign::any_supported_type(&PrivateKeyDer::Pkcs8(
        PrivatePkcs8KeyDer::from(impostor_key.serialize_der()),
    ))
    .unwrap();
    let stolen = Arc::new(CertifiedKey::new(vec![receiver.cert_der()], signing));
    let crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyServer))
        .with_client_cert_resolver(Arc::new(Stolen(stolen)));
    let config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap(),
    ));
    let mut impostor = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
    impostor.set_default_client_config(config);

    let accept = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(3), sender.accept_peer())
            .await
            .is_ok()
    });
    let _ = impostor.connect(sender_addr, "relay.local").unwrap().await;
    assert!(
        !accept.await.unwrap(),
        "sender accepted a copied certificate signed with the wrong key"
    );
}
