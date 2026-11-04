use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::SecureRandom;

use crate::error::{AppError, AppResult};

/// Encrypts file chunks with AES-256-GCM. Chunk nonces are unique within an
/// offer because the protocol binds the file and chunk indices into the nonce.
pub struct ChunkEncryptor {
    key: LessSafeKey,
    file_index: u32,
    chunk_index: u64,
}

impl ChunkEncryptor {
    pub fn new(key_bytes: &[u8; 32], file_index: u32) -> AppResult<Self> {
        let unbound = UnboundKey::new(&AES_256_GCM, key_bytes)
            .map_err(|_| AppError::Crypto("failed to create AES-256-GCM key".into()))?;
        Ok(Self {
            key: LessSafeKey::new(unbound),
            file_index,
            chunk_index: 0,
        })
    }

    pub fn encrypt_chunk(&mut self, plaintext: &[u8]) -> AppResult<(Vec<u8>, [u8; 12])> {
        if self.chunk_index > u32::MAX as u64 {
            return Err(AppError::Transfer("too many chunks in file".into()));
        }
        let chunk_index = self.chunk_index as u32;
        let nonce_bytes = make_nonce(self.file_index, chunk_index);
        let chunk_aad = aad(self.file_index, chunk_index);
        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::from(&chunk_aad[..]),
                &mut in_out,
            )
            .map_err(|_| AppError::Crypto("AES-GCM encryption failed".into()))?;
        self.chunk_index += 1;
        Ok((in_out, nonce_bytes))
    }

    /// Encrypt a small signaling payload with a fresh random nonce.
    pub fn encrypt_one(self, plaintext: &[u8]) -> AppResult<(Vec<u8>, [u8; 12])> {
        let mut nonce = [0; 12];
        ring::rand::SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| AppError::Crypto("failed to generate nonce".into()))?;
        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::empty(),
                &mut in_out,
            )
            .map_err(|_| AppError::Crypto("AES-GCM encryption failed".into()))?;
        Ok((in_out, nonce))
    }
}

pub struct ChunkDecryptor {
    key: LessSafeKey,
}

impl ChunkDecryptor {
    pub fn new(key_bytes: &[u8; 32]) -> AppResult<Self> {
        let unbound = UnboundKey::new(&AES_256_GCM, key_bytes)
            .map_err(|_| AppError::Crypto("failed to create AES-256-GCM key".into()))?;
        Ok(Self {
            key: LessSafeKey::new(unbound),
        })
    }

    pub fn decrypt_one(self, ciphertext: &[u8], nonce: &[u8; 12]) -> AppResult<Vec<u8>> {
        self.decrypt_with_aad(ciphertext, nonce, Aad::from(&[][..]))
    }

    pub fn decrypt_chunk(
        &self,
        ciphertext: &[u8],
        nonce: &[u8; 12],
        file_index: u32,
        chunk_index: u32,
    ) -> AppResult<Vec<u8>> {
        if *nonce != make_nonce(file_index, chunk_index) {
            return Err(AppError::Crypto(
                "chunk nonce does not match its indices".into(),
            ));
        }
        let chunk_aad = aad(file_index, chunk_index);
        self.decrypt_with_aad(ciphertext, nonce, Aad::from(&chunk_aad[..]))
    }

    fn decrypt_with_aad(
        &self,
        ciphertext: &[u8],
        nonce: &[u8; 12],
        aad: Aad<&[u8]>,
    ) -> AppResult<Vec<u8>> {
        let mut in_out = ciphertext.to_vec();
        let plaintext = self
            .key
            .open_in_place(Nonce::assume_unique_for_key(*nonce), aad, &mut in_out)
            .map_err(|_| {
                AppError::Crypto("AES-GCM decryption failed (tampered or wrong key)".into())
            })?;
        Ok(plaintext.to_vec())
    }
}

fn make_nonce(file_index: u32, chunk_index: u32) -> [u8; 12] {
    let mut nonce = [0; 12];
    nonce[..4].copy_from_slice(&file_index.to_be_bytes());
    nonce[4..].copy_from_slice(&(chunk_index as u64).to_be_bytes());
    nonce
}

fn aad(file_index: u32, chunk_index: u32) -> [u8; 8] {
    let mut aad = [0; 8];
    aad[..4].copy_from_slice(&file_index.to_be_bytes());
    aad[4..].copy_from_slice(&chunk_index.to_be_bytes());
    aad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_nonces_are_unique_across_files() {
        let key = [7; 32];
        let mut first = ChunkEncryptor::new(&key, 0).unwrap();
        let mut second = ChunkEncryptor::new(&key, 1).unwrap();
        let (_, nonce_a) = first.encrypt_chunk(b"a").unwrap();
        let (_, nonce_b) = second.encrypt_chunk(b"b").unwrap();
        assert_ne!(nonce_a, nonce_b);
    }

    #[test]
    fn chunk_roundtrip_and_indices_are_authenticated() {
        let key = [42; 32];
        let mut enc = ChunkEncryptor::new(&key, 3).unwrap();
        let dec = ChunkDecryptor::new(&key).unwrap();
        let (ciphertext, nonce) = enc.encrypt_chunk(b"payload").unwrap();
        assert_eq!(
            dec.decrypt_chunk(&ciphertext, &nonce, 3, 0).unwrap(),
            b"payload"
        );
        assert!(dec.decrypt_chunk(&ciphertext, &nonce, 4, 0).is_err());
    }
}
