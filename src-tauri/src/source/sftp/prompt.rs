use std::future::Future;
use std::pin::Pin;

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[derive(Debug, Clone, PartialEq)]
pub enum SecretKind {
    KeyPassphrase { key_path: String },
    Password,
}

/// Asks the person at the screen. Implemented over Tauri events in the app, by fakes in tests.
pub trait Prompter: Send + Sync {
    fn confirm_host_key(&self, host: String, fingerprint: String) -> BoxFut<bool>;
    /// Returns (secret, remember) or None when cancelled.
    fn ask_secret(&self, kind: SecretKind, host: String) -> BoxFut<Option<(String, bool)>>;
}
