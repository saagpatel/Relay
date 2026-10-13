// Receiver pipeline — orchestrates the full receive flow.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::crypto::aes_gcm::ChunkDecryptor;
use crate::error::{AppError, AppResult};
use crate::network::transport::Transport;
use crate::protocol::messages::PeerMessage;
use crate::protocol::reassembler::FileReassembler;
use crate::protocol::version::{
    is_peer_protocol_compatible, COMPATIBILITY_POLICY, CURRENT_PROTOCOL_VERSION,
};
use crate::transfer::progress::{FileOfferInfo, ProgressEvent, ProgressTracker};

/// Run the receiver pipeline over an established transport (QUIC or relay).
pub async fn run_receive(
    session_id: String,
    save_dir: PathBuf,
    transport: &mut Transport,
    encryption_key: [u8; 32],
    progress_tx: mpsc::UnboundedSender<ProgressEvent>,
    accept_rx: oneshot::Receiver<bool>,
    cancel: tokio_util::sync::CancellationToken,
) -> AppResult<()> {
    info!("receiver: waiting for file offer");
    progress_tx
        .send(ProgressEvent::StateChanged {
            session_id: session_id.clone(),
            state: "transferring".into(),
        })
        .ok();

    // Receive file offer
    let offer = transport.recv_peer_message().await?;
    let files = match offer {
        PeerMessage::FileOffer {
            protocol_version,
            files,
        } => {
            let peer_version = protocol_version
                .as_deref()
                .ok_or_else(|| AppError::Transfer("peer protocol version is required".into()))?;
            if !is_peer_protocol_compatible(peer_version) {
                return Err(AppError::Transfer(format!(
                    "incompatible peer protocol version '{peer_version}' (local {CURRENT_PROTOCOL_VERSION}, policy {COMPATIBILITY_POLICY})"
                )));
            }
            files
        }
        _ => return Err(AppError::Transfer("expected FileOffer message".into())),
    };

    info!("receiver: got offer for {} file(s)", files.len());
    validate_offer(&files)?;

    // Notify frontend about the offer
    let offer_infos: Vec<FileOfferInfo> = files
        .iter()
        .map(|f| FileOfferInfo {
            name: f.name.clone(),
            size: f.size,
            relative_path: f.relative_path.clone(),
        })
        .collect();
    progress_tx
        .send(ProgressEvent::FileOffer {
            session_id: session_id.clone(),
            files: offer_infos,
        })
        .ok();

    // Wait for user acceptance
    let accepted = tokio::select! {
        result = accept_rx => result.unwrap_or(false),
        _ = cancel.cancelled() => false,
    };

    if !accepted {
        transport
            .send_peer_message(&PeerMessage::FileDecline)
            .await?;
        return Err(AppError::Cancelled);
    }

    transport
        .send_peer_message(&PeerMessage::FileAccept)
        .await?;
    progress_tx
        .send(ProgressEvent::StateChanged {
            session_id: session_id.clone(),
            state: "transferring".into(),
        })
        .ok();

    let total_bytes: u64 = files.iter().map(|f| f.size).sum();
    let mut tracker = ProgressTracker::new(total_bytes);

    let destinations = validate_destinations(&save_dir, &files).await?;
    let mut reassemblers: Vec<Option<FileReassembler>> = (0..files.len()).map(|_| None).collect();
    let mut next_chunk = vec![0u32; files.len()];
    let mut verified = vec![false; files.len()];

    // Receive chunks until TransferComplete
    loop {
        let msg = tokio::select! {
            result = transport.recv_peer_message() => result?,
            _ = cancel.cancelled() => {
                transport.send_peer_message(&PeerMessage::Cancel {
                    reason: "cancelled by receiver".into(),
                }).await.ok();
                drop(reassemblers);
                return Err(AppError::Cancelled);
            },
        };

        match msg {
            PeerMessage::FileChunk {
                file_index,
                chunk_index,
                data,
                nonce,
                ..
            } => {
                let idx = usize::from(file_index);
                let expected_chunk = *checked_file_mut(&mut next_chunk, file_index)?;
                if verified[idx] {
                    return Err(AppError::Transfer(
                        "chunk received after file verification".into(),
                    ));
                }
                if chunk_index != expected_chunk {
                    return Err(AppError::Transfer(format!(
                        "invalid chunk index {chunk_index}; expected {expected_chunk}"
                    )));
                }
                if data.len() < 16 || data.len() > crate::protocol::chunker::CHUNK_SIZE + 16 {
                    return Err(AppError::Transfer("invalid encrypted chunk size".into()));
                }
                if reassemblers[idx].is_none() {
                    reassemblers[idx] = Some(
                        FileReassembler::new(
                            &destinations[idx],
                            ChunkDecryptor::new(&encryption_key)?,
                        )
                        .await?,
                    );
                }
                let reassembler = reassemblers[idx]
                    .as_mut()
                    .ok_or_else(|| AppError::Transfer("file already completed".into()))?;
                let plaintext_size = reassembler
                    .write_chunk(&data, &nonce, u32::from(file_index), chunk_index)
                    .await?;
                if reassembler.bytes_written() > files[idx].size {
                    return Err(AppError::Transfer(
                        "received more bytes than offered".into(),
                    ));
                }
                next_chunk[idx] = expected_chunk
                    .checked_add(1)
                    .ok_or_else(|| AppError::Transfer("chunk index overflow".into()))?;
                tracker.update(plaintext_size as u64);
                progress_tx
                    .send(ProgressEvent::TransferProgress {
                        session_id: session_id.clone(),
                        bytes_transferred: tracker.bytes_transferred(),
                        bytes_total: tracker.bytes_total(),
                        speed_bps: tracker.speed_bps(),
                        eta_seconds: tracker.eta_seconds(),
                        current_file: files[idx].name.clone(),
                        percent: tracker.percent(),
                    })
                    .ok();
            }
            PeerMessage::FileComplete { file_index, sha256 } => {
                let idx = usize::from(file_index);
                let slot = checked_file_mut(&mut reassemblers, file_index)?;
                if verified[idx] {
                    return Err(AppError::Transfer("file already completed".into()));
                }
                if slot.is_none() {
                    *slot = Some(
                        FileReassembler::new(
                            &destinations[idx],
                            ChunkDecryptor::new(&encryption_key)?,
                        )
                        .await?,
                    );
                }
                let reassembler = slot
                    .take()
                    .ok_or_else(|| AppError::Transfer("file already completed".into()))?;
                reassembler.verify(&sha256, files[idx].size).await?;
                verified[idx] = true;
                info!("receiver: file '{}' verified", files[idx].name);

                transport
                    .send_peer_message(&PeerMessage::FileVerified { file_index })
                    .await?;

                progress_tx
                    .send(ProgressEvent::FileCompleted {
                        session_id: session_id.clone(),
                        name: files[idx].name.clone(),
                    })
                    .ok();
            }
            PeerMessage::TransferComplete => {
                require_all_verified(&verified)?;
                info!("receiver: transfer complete");
                break;
            }
            PeerMessage::Cancel { reason } => {
                warn!("receiver: sender cancelled: {reason}");
                return Err(AppError::Transfer(format!("sender cancelled: {reason}")));
            }
            _ => {
                return Err(AppError::Transfer(
                    "unexpected message during transfer".into(),
                ));
            }
        }
    }

    progress_tx
        .send(ProgressEvent::TransferComplete {
            session_id,
            duration_seconds: tracker.elapsed_seconds(),
            average_speed: tracker.average_speed(),
            total_bytes,
            file_count: files.len() as u32,
        })
        .ok();

    Ok(())
}

/// Sanitize a relative path for folder transfers.
/// Each component is validated individually: no `..`, no absolute paths, no null bytes.
/// Returns the sanitized relative path.
pub fn sanitize_path(rel_path: &str) -> AppResult<PathBuf> {
    let path = Path::new(rel_path);

    // Reject absolute paths
    if path.is_absolute() || rel_path.starts_with('/') || rel_path.starts_with('\\') {
        return Err(AppError::Transfer(format!(
            "absolute path not allowed: {rel_path}"
        )));
    }

    // Reject null bytes
    if rel_path.contains('\0') {
        return Err(AppError::Transfer("null byte in path".into()));
    }

    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(c) => {
                let s = c.to_string_lossy();
                // Sanitize each component: remove path separators
                let clean = s.replace(['/', '\\'], "_").replace('\0', "");
                if !clean.is_empty() {
                    safe.push(&clean);
                }
            }
            std::path::Component::ParentDir => {
                return Err(AppError::Transfer(format!(
                    "path traversal not allowed: {rel_path}"
                )));
            }
            std::path::Component::CurDir => {
                // Skip "." components
                continue;
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(AppError::Transfer(format!(
                    "absolute path component not allowed: {rel_path}"
                )));
            }
        }
    }

    if safe.as_os_str().is_empty() {
        return Err(AppError::Transfer(format!(
            "path resolves to empty: {rel_path}"
        )));
    }

    Ok(safe)
}

/// Sanitize a flat filename: remove path separators, reject traversal attacks.
fn sanitize_filename(name: &str) -> String {
    let name = name
        .replace(['/', '\\'], "_")
        .replace("..", "_")
        .replace('\0', "");
    if name.is_empty() {
        "unnamed_file".to_string()
    } else {
        name
    }
}

fn validate_offer(files: &[crate::protocol::messages::FileInfo]) -> AppResult<()> {
    const MAX_FILES: usize = 4096;
    const MAX_METADATA_BYTES: usize = 1024 * 1024;
    if files.is_empty() || files.len() > MAX_FILES || files.len() > u16::MAX as usize + 1 {
        return Err(AppError::Transfer(
            "invalid or excessive file count in offer".into(),
        ));
    }
    let metadata_bytes: usize = files
        .iter()
        .map(|f| f.name.len() + f.relative_path.as_ref().map_or(0, String::len))
        .sum();
    if metadata_bytes > MAX_METADATA_BYTES {
        return Err(AppError::Transfer(
            "file offer metadata exceeds limit".into(),
        ));
    }
    files
        .iter()
        .try_fold(0u64, |total, file| total.checked_add(file.size))
        .ok_or_else(|| AppError::Transfer("file offer total size overflow".into()))?;
    Ok(())
}

async fn validate_destinations(
    save_dir: &Path,
    files: &[crate::protocol::messages::FileInfo],
) -> AppResult<Vec<PathBuf>> {
    tokio::fs::create_dir_all(save_dir).await?;
    let root = tokio::fs::canonicalize(save_dir).await?;
    let mut seen = HashSet::new();
    let mut destinations = Vec::with_capacity(files.len());
    for file in files {
        let rel = match &file.relative_path {
            Some(path) => sanitize_path(path)?,
            None => PathBuf::from(sanitize_filename(&file.name)),
        };
        let target = root.join(rel);
        if !seen.insert(target.clone()) {
            return Err(AppError::Transfer(format!(
                "duplicate destination: {}",
                target.display()
            )));
        }
        let parent = target
            .parent()
            .ok_or_else(|| AppError::Transfer("destination has no parent".into()))?;
        tokio::fs::create_dir_all(parent).await?;
        let canonical_parent = tokio::fs::canonicalize(parent).await?;
        if !canonical_parent.starts_with(&root) {
            return Err(AppError::Transfer(
                "destination escapes save directory through a symlink".into(),
            ));
        }
        if tokio::fs::symlink_metadata(&target).await.is_ok() {
            return Err(AppError::Transfer(format!(
                "destination already exists: {}",
                target.display()
            )));
        }
        destinations.push(target);
    }
    Ok(destinations)
}

fn checked_file_mut<T>(files: &mut [T], index: u16) -> AppResult<&mut T> {
    files
        .get_mut(usize::from(index))
        .ok_or_else(|| AppError::Transfer(format!("invalid file index: {index}")))
}

fn require_all_verified(verified: &[bool]) -> AppResult<()> {
    if verified.iter().all(|done| *done) {
        Ok(())
    } else {
        Err(AppError::Transfer(
            "TransferComplete received before every file was verified".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello.txt"), "hello.txt");
        assert_eq!(sanitize_filename("../etc/passwd"), "__etc_passwd");
        assert_eq!(sanitize_filename("foo/bar.txt"), "foo_bar.txt");
        assert_eq!(sanitize_filename(""), "unnamed_file");
        assert_eq!(sanitize_filename("hello\0world"), "helloworld");
    }

    #[test]
    fn test_sanitize_path_valid() {
        let p = sanitize_path("docs/readme.md").unwrap();
        assert_eq!(p, PathBuf::from("docs/readme.md"));

        let p = sanitize_path("a/b/c/file.txt").unwrap();
        assert_eq!(p, PathBuf::from("a/b/c/file.txt"));

        let p = sanitize_path("file.txt").unwrap();
        assert_eq!(p, PathBuf::from("file.txt"));
    }

    #[test]
    fn test_sanitize_path_traversal() {
        assert!(sanitize_path("../etc/passwd").is_err());
        assert!(sanitize_path("foo/../../etc/passwd").is_err());
        assert!(sanitize_path("..").is_err());
    }

    #[test]
    fn test_sanitize_path_absolute() {
        assert!(sanitize_path("/etc/passwd").is_err());
        assert!(sanitize_path("\\Windows\\System32").is_err());
    }

    #[test]
    fn test_sanitize_path_null_bytes() {
        assert!(sanitize_path("foo\0bar").is_err());
    }

    #[test]
    fn test_sanitize_path_dot_components() {
        let p = sanitize_path("./foo/./bar.txt").unwrap();
        assert_eq!(p, PathBuf::from("foo/bar.txt"));
    }

    #[test]
    fn test_sanitize_path_empty() {
        assert!(sanitize_path("").is_err());
    }

    #[test]
    fn test_sanitize_path_windows_separators() {
        let p = sanitize_path("docs\\readme.md").unwrap();
        // On Unix, backslash is treated as part of the filename and sanitized
        // On Windows, it's treated as a separator
        assert!(!p.as_os_str().is_empty());
    }

    fn info(name: &str, relative_path: Option<&str>) -> crate::protocol::messages::FileInfo {
        crate::protocol::messages::FileInfo {
            name: name.into(),
            size: 1,
            relative_path: relative_path.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn rejects_duplicate_destinations_and_existing_targets() {
        let dir = tempfile::tempdir().unwrap();
        let duplicate = [info("same.txt", None), info("same.txt", None)];
        assert!(validate_destinations(dir.path(), &duplicate).await.is_err());
        let existing = [info("present.txt", None)];
        tokio::fs::write(dir.path().join("present.txt"), b"preserve")
            .await
            .unwrap();
        assert!(validate_destinations(dir.path(), &existing).await.is_err());
        assert_eq!(
            tokio::fs::read(dir.path().join("present.txt"))
                .await
                .unwrap(),
            b"preserve"
        );
    }

    #[tokio::test]
    async fn rejects_symlink_escape_and_metadata_overflow() {
        let base = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), base.path().join("escape")).unwrap();
            assert!(
                validate_destinations(base.path(), &[info("file", Some("escape/file"))])
                    .await
                    .is_err()
            );
        }
        let huge = vec![info(&"x".repeat(1_048_577), None)];
        assert!(validate_offer(&huge).is_err());
    }

    #[test]
    fn offer_file_count_is_bounded() {
        let files: Vec<_> = (0..4097).map(|i| info(&format!("{i}"), None)).collect();
        assert!(validate_offer(&files).is_err());
    }

    #[test]
    fn checked_file_index_rejects_out_of_range() {
        let mut files = vec![10u8];
        assert!(checked_file_mut(&mut files, 1).is_err());
    }

    #[test]
    fn rejects_early_transfer_complete() {
        assert!(require_all_verified(&[true, false]).is_err());
        assert!(require_all_verified(&[true, true]).is_ok());
    }
}
