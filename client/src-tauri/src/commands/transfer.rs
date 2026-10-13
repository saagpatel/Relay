use std::collections::HashMap;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::sync::{oneshot, Mutex};
use tracing::info;

use crate::transfer::session::TransferSession;

/// Type alias for the shared session store.
pub type SessionStore = Arc<Mutex<HashMap<String, Arc<TransferSession>>>>;

/// Type alias for pending accept/decline channels.
pub type AcceptChannelStore = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

/// Removes a session and pending accept channel whenever its runner task exits,
/// including cancellation and unwinding paths.
pub struct SessionCleanupGuard {
    sessions: SessionStore,
    accepts: Option<AcceptChannelStore>,
    session_id: String,
}

impl SessionCleanupGuard {
    pub fn new(
        sessions: SessionStore,
        accepts: Option<AcceptChannelStore>,
        session_id: String,
    ) -> Self {
        Self {
            sessions,
            accepts,
            session_id,
        }
    }
}

impl Drop for SessionCleanupGuard {
    fn drop(&mut self) {
        let sessions = self.sessions.clone();
        let accepts = self.accepts.clone();
        let session_id = self.session_id.clone();
        tokio::spawn(async move {
            remove_session(&sessions, accepts.as_ref(), &session_id).await;
        });
    }
}

/// Create the default stores to be managed by Tauri.
pub fn create_stores() -> (SessionStore, AcceptChannelStore) {
    (
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
    )
}

pub async fn remove_session(
    sessions: &SessionStore,
    accepts: Option<&AcceptChannelStore>,
    session_id: &str,
) {
    sessions.lock().await.remove(session_id);
    if let Some(accepts) = accepts {
        accepts.lock().await.remove(session_id);
    }
}

/// Cancel an active transfer.
#[tauri::command]
pub async fn cancel_transfer(app: AppHandle, session_id: String) -> Result<(), String> {
    let store = app.state::<SessionStore>().inner().clone();
    let sessions = store.lock().await;

    if let Some(session) = sessions.get(&session_id) {
        info!("cancelling transfer {session_id}");
        session.cancel();
        Ok(())
    } else {
        Err(format!("session not found: {session_id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{
        code::TransferCode,
        session::{TransferRole, TransferSession},
    };

    #[tokio::test]
    async fn dropped_session_guard_removes_session_and_accept_channel() {
        let (sessions, accepts) = create_stores();
        let id = "session-test".to_owned();
        let (accept_tx, _accept_rx) = oneshot::channel();
        sessions.lock().await.insert(
            id.clone(),
            Arc::new(TransferSession::new(
                TransferRole::Sender,
                TransferCode::generate(),
            )),
        );
        accepts.lock().await.insert(id.clone(), accept_tx);
        drop(SessionCleanupGuard::new(
            sessions.clone(),
            Some(accepts.clone()),
            id.clone(),
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if !sessions.lock().await.contains_key(&id)
                    && !accepts.lock().await.contains_key(&id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!sessions.lock().await.contains_key(&id));
        assert!(!accepts.lock().await.contains_key(&id));
    }
}
