use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use quinn::{Connection, Endpoint, ServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::error::{AppError, AppResult};

/// SHA-256 of a DER certificate: what peers exchange during key confirmation.
pub fn fingerprint(cert: &CertificateDer<'_>) -> [u8; 32] {
    Sha256::digest(cert.as_ref()).into()
}

/// A QUIC endpoint that can both listen (accept) and connect.
///
/// Each endpoint has a fresh self-signed certificate. Peers learn each
/// other's certificate fingerprint through the key-confirmation step
/// (`SignalingClient::exchange_cert_fingerprint`), which is authenticated by
/// the SPAKE2 key, and then pin it: the receiver (QUIC client) accepts only
/// the sender's certificate, and the sender (QUIC server) requires a client
/// certificate and accepts only the receiver's. Handshake signatures are
/// verified, so presenting a copy of a certificate without its key fails.
pub struct QuicEndpoint {
    endpoint: Endpoint,
    cert_der: CertificateDer<'static>,
    key_der: PrivatePkcs8KeyDer<'static>,
    cert_fingerprint: [u8; 32],
    /// The client fingerprint the server side accepts; None rejects everyone.
    expected_client: Arc<RwLock<Option<[u8; 32]>>>,
}

impl QuicEndpoint {
    /// Create a new QUIC endpoint bound to `0.0.0.0:{port}`.
    /// Use port 0 for OS-assigned. It rejects every incoming connection until
    /// `pin_peer` names the client certificate to accept.
    pub async fn new(port: u16) -> AppResult<Self> {
        let subject_alt_names = vec!["relay.local".to_string()];
        let cert_params = rcgen::CertificateParams::new(subject_alt_names)
            .map_err(|e| AppError::Crypto(format!("cert params: {e}")))?;
        let key_pair =
            rcgen::KeyPair::generate().map_err(|e| AppError::Crypto(format!("keygen: {e}")))?;
        let cert = cert_params
            .self_signed(&key_pair)
            .map_err(|e| AppError::Crypto(format!("self-sign: {e}")))?;

        let cert_der = CertificateDer::from(cert.der().to_vec());
        let key_der = PrivatePkcs8KeyDer::from(key_pair.serialize_der());
        let cert_fingerprint = fingerprint(&cert_der);

        let expected_client = Arc::new(RwLock::new(None));
        let server_crypto = rustls::ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(PinnedClientVerifier {
                expected: expected_client.clone(),
                algorithms: algorithms(),
            }))
            .with_single_cert(vec![cert_der.clone()], key_der.clone_key().into())
            .map_err(|e| AppError::Crypto(format!("server TLS config: {e}")))?;

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
                .map_err(|e| AppError::Crypto(format!("QUIC server config: {e}")))?,
        ));

        let addr: SocketAddr = format!("0.0.0.0:{port}")
            .parse()
            .map_err(|e| AppError::Network(format!("bad bind address: {e}")))?;
        let endpoint = Endpoint::server(server_config, addr)
            .map_err(|e| AppError::Network(format!("failed to bind QUIC endpoint: {e}")))?;

        info!(
            "QUIC endpoint listening on {}",
            endpoint
                .local_addr()
                .map_err(|e| AppError::Network(e.to_string()))?
        );

        Ok(Self {
            endpoint,
            cert_der,
            key_der,
            cert_fingerprint,
            expected_client,
        })
    }

    /// Accept connections only from the client whose certificate has this
    /// fingerprint (the peer's, from key confirmation).
    pub fn pin_peer(&self, peer_fingerprint: [u8; 32]) -> AppResult<()> {
        let mut expected = self
            .expected_client
            .write()
            .map_err(|_| AppError::Crypto("pinned fingerprint lock poisoned".into()))?;
        *expected = Some(peer_fingerprint);
        Ok(())
    }

    /// Accept the next connection from the pinned peer. Handshakes that fail
    /// pinning (an impostor or a stray client) are logged and skipped; callers
    /// bound the wait with a timeout.
    pub async fn accept_peer(&self) -> AppResult<Connection> {
        loop {
            let incoming = self
                .endpoint
                .accept()
                .await
                .ok_or_else(|| AppError::Network("endpoint closed".into()))?;
            let remote = incoming.remote_address();
            match incoming.await {
                Ok(conn) => {
                    info!(
                        "accepted pinned QUIC connection from {}",
                        conn.remote_address()
                    );
                    return Ok(conn);
                }
                Err(e) => warn!("rejected QUIC connection from {remote}: {e}"),
            }
        }
    }

    /// Connect to the peer at `addr`, accepting only a server certificate with
    /// `peer_fingerprint` and presenting our own certificate for the peer to pin.
    pub async fn connect(
        &self,
        addr: SocketAddr,
        peer_fingerprint: [u8; 32],
    ) -> AppResult<Connection> {
        let client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinnedServerVerifier {
                expected: peer_fingerprint,
                algorithms: algorithms(),
            }))
            .with_client_auth_cert(
                vec![self.cert_der.clone()],
                PrivateKeyDer::Pkcs8(self.key_der.clone_key()),
            )
            .map_err(|e| AppError::Crypto(format!("client TLS config: {e}")))?;

        let client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
                .map_err(|e| AppError::Crypto(format!("QUIC client config: {e}")))?,
        ));

        let conn = self
            .endpoint
            .connect_with(client_config, addr, "relay.local")
            .map_err(|e| AppError::Network(format!("connect: {e}")))?
            .await
            .map_err(|e| AppError::Network(format!("connection failed: {e}")))?;

        info!("connected to pinned peer at {addr}");
        Ok(conn)
    }

    /// Our certificate (public; its fingerprint is what peers pin).
    pub fn cert_der(&self) -> CertificateDer<'static> {
        self.cert_der.clone()
    }

    /// SHA-256 fingerprint of our certificate.
    pub fn cert_fingerprint(&self) -> [u8; 32] {
        self.cert_fingerprint
    }

    /// Local address the endpoint is bound to.
    pub fn local_addr(&self) -> AppResult<SocketAddr> {
        self.endpoint
            .local_addr()
            .map_err(|e| AppError::Network(e.to_string()))
    }
}

impl Drop for QuicEndpoint {
    fn drop(&mut self) {
        // Use wait_idle=false to avoid blocking in drop.
        // The connection should drain naturally via QUIC's protocol.
        self.endpoint.close(0u32.into(), b"done");
    }
}

fn algorithms() -> WebPkiSupportedAlgorithms {
    rustls::crypto::ring::default_provider().signature_verification_algorithms
}

fn check_pin(cert: &CertificateDer<'_>, expected: Option<[u8; 32]>) -> Result<(), rustls::Error> {
    // Constant-time comparison is not needed: fingerprints are public.
    match expected {
        Some(fp) if fingerprint(cert) == fp => Ok(()),
        Some(_) => Err(rustls::Error::General(
            "peer certificate does not match the confirmed fingerprint".into(),
        )),
        None => Err(rustls::Error::General(
            "no peer fingerprint pinned yet".into(),
        )),
    }
}

/// Server side: the receiver's certificate, by fingerprint.
#[derive(Debug)]
struct PinnedClientVerifier {
    expected: Arc<RwLock<Option<[u8; 32]>>>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl ClientCertVerifier for PinnedClientVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        let expected = *self
            .expected
            .read()
            .map_err(|_| rustls::Error::General("pinned fingerprint lock poisoned".into()))?;
        check_pin(end_entity, expected)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

/// Client side: the sender's certificate, by fingerprint.
#[derive(Debug)]
struct PinnedServerVerifier {
    expected: [u8; 32],
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PinnedServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        check_pin(end_entity, Some(self.expected))?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
