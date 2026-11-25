//! Bridges SSH questions (host key, passphrase, password) to the UI via the
//! `ssh-prompt` event and the `ssh_prompt_reply` command.
use crate::source::sftp::prompt::{BoxFut, Prompter, SecretKind};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

pub struct Reply {
    pub accept: bool,
    pub secret: Option<String>,
    pub remember: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PromptEvent {
    id: u64,
    kind: &'static str,
    host: String,
    fingerprint: Option<String>,
    key_path: Option<String>,
}

pub struct TauriPrompter {
    app: AppHandle,
    next: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
}

impl TauriPrompter {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self { app, next: AtomicU64::new(1), pending: Mutex::new(HashMap::new()) })
    }

    pub fn reply(&self, id: u64, reply: Reply) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
            let _ = tx.send(reply);
        }
    }

    fn ask(self: &Arc<Self>, ev: PromptEvent) -> BoxFut<Option<Reply>> {
        let (tx, rx) = oneshot::channel();
        let id = ev.id;
        self.pending.lock().unwrap().insert(id, tx);
        let _ = self.app.emit("ssh-prompt", ev);
        let me = self.clone();
        Box::pin(async move {
            let r = tokio::time::timeout(Duration::from_secs(300), rx).await.ok().and_then(|r| r.ok());
            me.pending.lock().unwrap().remove(&id);
            r
        })
    }
}

/// Wrapper so the trait can take `&self` while asks need `Arc<Self>`.
pub struct Bridge(pub Arc<TauriPrompter>);

impl Prompter for Bridge {
    fn confirm_host_key(&self, host: String, fingerprint: String) -> BoxFut<bool> {
        let id = self.0.next.fetch_add(1, Ordering::SeqCst);
        let fut = self.0.ask(PromptEvent { id, kind: "hostKey", host, fingerprint: Some(fingerprint), key_path: None });
        Box::pin(async move { fut.await.is_some_and(|r| r.accept) })
    }

    fn ask_secret(&self, kind: SecretKind, host: String) -> BoxFut<Option<(String, bool)>> {
        let id = self.0.next.fetch_add(1, Ordering::SeqCst);
        let (kind, key_path) = match kind {
            SecretKind::KeyPassphrase { key_path } => ("passphrase", Some(key_path)),
            SecretKind::Password => ("password", None),
        };
        let fut = self.0.ask(PromptEvent { id, kind, host, fingerprint: None, key_path });
        Box::pin(async move {
            let r = fut.await?;
            if !r.accept {
                return None;
            }
            Some((r.secret?, r.remember))
        })
    }
}
