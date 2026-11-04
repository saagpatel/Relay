use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use crate::crypto::aes_gcm::ChunkDecryptor;
use crate::crypto::checksum::StreamingChecksum;
use crate::error::{AppError, AppResult};

/// Receives encrypted chunks into an owned same-directory partial file.
pub struct FileReassembler {
    file: Option<tokio::fs::File>,
    target: PathBuf,
    partial: PathBuf,
    decryptor: ChunkDecryptor,
    checksum: Option<StreamingChecksum>,
    bytes_written: u64,
}

impl FileReassembler {
    pub async fn new(target: &Path, decryptor: ChunkDecryptor) -> AppResult<Self> {
        let parent = target
            .parent()
            .ok_or_else(|| AppError::Transfer("destination has no parent".into()))?;
        tokio::fs::create_dir_all(parent).await?;
        if tokio::fs::symlink_metadata(target).await.is_ok() {
            return Err(AppError::Transfer(format!(
                "destination already exists: {}",
                target.display()
            )));
        }
        let name = target
            .file_name()
            .ok_or_else(|| AppError::Transfer("invalid destination name".into()))?
            .to_string_lossy();
        let (file, partial) = loop {
            let partial = parent.join(format!(".{name}.{}.part", uuid::Uuid::new_v4()));
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
                .await
            {
                Ok(file) => break (file, partial),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        };
        Ok(Self {
            file: Some(file),
            target: target.to_owned(),
            partial,
            decryptor,
            checksum: Some(StreamingChecksum::new()),
            bytes_written: 0,
        })
    }

    /// Decrypt and write one chunk. Returns the plaintext byte count.
    pub async fn write_chunk(
        &mut self,
        ciphertext: &[u8],
        nonce: &[u8; 12],
        file_index: u32,
        chunk_index: u32,
    ) -> AppResult<usize> {
        let plaintext = self
            .decryptor
            .decrypt_chunk(ciphertext, nonce, file_index, chunk_index)?;
        self.checksum
            .as_mut()
            .ok_or_else(|| AppError::Transfer("file already verified".into()))?
            .update(&plaintext);
        self.file
            .as_mut()
            .ok_or_else(|| AppError::Transfer("file is closed".into()))?
            .write_all(&plaintext)
            .await?;
        self.bytes_written += plaintext.len() as u64;
        Ok(plaintext.len())
    }

    /// Flush, verify size and checksum, then atomically publish without replacing
    /// an existing destination. Dropping on every error removes only our partial.
    pub async fn verify(mut self, expected: &[u8; 32], expected_size: u64) -> AppResult<()> {
        let mut file = self
            .file
            .take()
            .ok_or_else(|| AppError::Transfer("file is closed".into()))?;
        file.flush().await?;
        file.sync_all().await?;
        let metadata = file.metadata().await?;
        if metadata.len() != expected_size || self.bytes_written != expected_size {
            return Err(AppError::Transfer(format!(
                "received size mismatch: expected {expected_size}, got {}",
                metadata.len()
            )));
        }
        let actual = self
            .checksum
            .take()
            .ok_or_else(|| AppError::Transfer("file already verified".into()))?
            .finalize();
        if actual != *expected {
            return Err(AppError::ChecksumMismatch(format!(
                "expected {}, got {}",
                hex(&expected[..8]),
                hex(&actual[..8])
            )));
        }
        drop(file);
        tokio::fs::hard_link(&self.partial, &self.target).await?;
        tokio::fs::remove_file(&self.partial).await?;
        self.partial.clear();
        Ok(())
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

impl Drop for FileReassembler {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.partial.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.partial);
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{aes_gcm::ChunkEncryptor, checksum::StreamingChecksum};

    #[tokio::test]
    async fn publishes_only_verified_data_and_preserves_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("output.txt");
        tokio::fs::write(&target, b"keep me").await.unwrap();
        let result = FileReassembler::new(&target, ChunkDecryptor::new(&[1; 32]).unwrap()).await;
        assert!(result.is_err());
        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"keep me");
    }

    #[tokio::test]
    async fn checksum_failure_removes_only_our_partial() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("output.txt");
        let key = [8; 32];
        let mut enc = ChunkEncryptor::new(&key, 0).unwrap();
        let (cipher, nonce) = enc.encrypt_chunk(b"payload").unwrap();
        let mut file = FileReassembler::new(&target, ChunkDecryptor::new(&key).unwrap())
            .await
            .unwrap();
        file.write_chunk(&cipher, &nonce, 0, 0).await.unwrap();
        assert!(file.verify(&[0; 32], 7).await.is_err());
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn wrong_size_and_cancel_leave_no_output() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("output.txt");
        let key = [9; 32];
        let mut enc = ChunkEncryptor::new(&key, 0).unwrap();
        let (cipher, nonce) = enc.encrypt_chunk(b"payload").unwrap();
        let mut file = FileReassembler::new(&target, ChunkDecryptor::new(&key).unwrap())
            .await
            .unwrap();
        file.write_chunk(&cipher, &nonce, 0, 0).await.unwrap();
        let mut checksum = StreamingChecksum::new();
        checksum.update(b"payload");
        assert!(file.verify(&checksum.finalize(), 8).await.is_err());
        assert!(!target.exists());
        {
            let _cancelled = FileReassembler::new(&target, ChunkDecryptor::new(&key).unwrap())
                .await
                .unwrap();
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn write_setup_failure_leaves_no_partial_or_modified_parent() {
        let dir = tempfile::tempdir().unwrap();
        let parent_file = dir.path().join("parent");
        tokio::fs::write(&parent_file, b"keep").await.unwrap();
        let target = parent_file.join("output.txt");
        assert!(
            FileReassembler::new(&target, ChunkDecryptor::new(&[3; 32]).unwrap())
                .await
                .is_err()
        );
        assert_eq!(tokio::fs::read(parent_file).await.unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
