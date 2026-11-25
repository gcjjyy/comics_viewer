use super::config::HostParams;
use super::hostkey::{host_key_status, HostKeyStatus};
use super::prompt::{Prompter, SecretKind};
use russh::client::{self, AuthResult, Handle};
use russh::keys::known_hosts::learn_known_hosts_path;
use russh::keys::{load_secret_key, Error as KeyError, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh_sftp::client::SftpSession;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const KEYRING_SERVICE: &str = "comics-viewer";

pub struct Conn {
    pub sftp: SftpSession,
    _ssh: Handle<HostCheck>,
}

pub struct HostCheck {
    host: String,
    port: u16,
    known_hosts: PathBuf,
    prompter: Arc<dyn Prompter>,
    /// Set when the server key differs from known_hosts, so connect() can explain the refusal.
    mismatch: Arc<std::sync::atomic::AtomicBool>,
}

impl client::Handler for HostCheck {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = server else { return Ok(false) };
        match host_key_status(&self.known_hosts, &self.host, self.port, key) {
            HostKeyStatus::Known => Ok(true),
            HostKeyStatus::Mismatch => {
                self.mismatch.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(false)
            }
            HostKeyStatus::Unknown => {
                let fp = key.fingerprint(HashAlg::Sha256).to_string();
                if !self.prompter.confirm_host_key(self.host.clone(), fp).await {
                    return Ok(false);
                }
                let _ = learn_known_hosts_path(&self.host, self.port, key, &self.known_hosts);
                Ok(true)
            }
        }
    }
}

fn keyring_get(account: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, account).ok()?.get_password().ok()
}

fn keyring_set(account: &str, secret: &str) {
    if let Ok(e) = keyring::Entry::new(KEYRING_SERVICE, account) {
        let _ = e.set_password(secret);
    }
}

fn keyring_forget(account: &str) {
    if let Ok(e) = keyring::Entry::new(KEYRING_SERVICE, account) {
        let _ = e.delete_credential();
    }
}

fn public_key_of(identity: &Path) -> Option<PublicKey> {
    let text = std::fs::read_to_string(identity.with_extension("pub")).ok()?;
    PublicKey::from_openssh(text.trim()).ok()
}

/// Connects, verifies the host key against ~/.ssh/known_hosts, authenticates and opens SFTP.
pub async fn connect(params: &HostParams, home: &Path, prompter: Arc<dyn Prompter>) -> Result<Conn, String> {
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        ..Default::default()
    });
    let mismatch = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handler = HostCheck {
        host: params.hostname.clone(),
        port: params.port,
        known_hosts: home.join(".ssh").join("known_hosts"),
        prompter: prompter.clone(),
        mismatch: mismatch.clone(),
    };
    let host = &params.alias;
    let mut ssh = match client::connect(config, (params.hostname.as_str(), params.port), handler).await {
        Ok(h) => h,
        Err(_) if mismatch.load(std::sync::atomic::Ordering::SeqCst) => {
            return Err(format!(
                "'{host}' 서버의 호스트 키가 known_hosts에 저장된 것과 다릅니다. 서버가 바뀐 게 맞다면 터미널에서 `ssh-keygen -R {}` 실행 후 다시 시도하세요.",
                params.hostname
            ))
        }
        Err(russh::Error::UnknownKey) => return Err(format!("'{host}' 서버 접속을 취소했습니다.")),
        Err(e) => return Err(format!("'{host}' 서버에 접속하지 못했습니다: {e}")),
    };

    if !authenticate(&mut ssh, params, prompter).await? {
        return Err(format!("'{host}' 서버 로그인에 실패했습니다. 키나 비밀번호를 확인하세요."));
    }

    let channel = ssh.channel_open_session().await.map_err(|e| format!("SFTP 채널을 열지 못했습니다: {e}"))?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| format!("SFTP를 시작하지 못했습니다: {e}"))?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| format!("SFTP를 시작하지 못했습니다: {e}"))?;
    Ok(Conn { sftp, _ssh: ssh })
}

async fn rsa_hash(ssh: &Handle<HostCheck>, key: &PublicKey) -> Option<HashAlg> {
    if key.algorithm().is_rsa() {
        ssh.best_supported_rsa_hash().await.ok().flatten().flatten()
    } else {
        None
    }
}

async fn authenticate(ssh: &mut Handle<HostCheck>, p: &HostParams, prompter: Arc<dyn Prompter>) -> Result<bool, String> {
    let user = p.user.as_str();
    let allowed: Vec<PublicKey> = p.identity_files.iter().filter_map(|f| public_key_of(f)).collect();

    // 1. ssh-agent
    if let Some(mut agent) = agent_client().await {
        if let Ok(ids) = agent.request_identities().await {
            for id in ids {
                let key = id.public_key().into_owned();
                if p.identities_only && !allowed.iter().any(|a| a.key_data() == key.key_data()) {
                    continue;
                }
                let hash = rsa_hash(ssh, &key).await;
                if let Ok(AuthResult::Success) = ssh.authenticate_publickey_with(user, key, hash, &mut agent).await {
                    return Ok(true);
                }
            }
        }
    }

    // 2. identity files (asking for the passphrase when encrypted)
    for path in &p.identity_files {
        let account = path.to_string_lossy().into_owned();
        let key = match load_secret_key(path, None) {
            Ok(k) => Some(k),
            Err(KeyError::KeyIsEncrypted) => {
                let mut found = keyring_get(&account).and_then(|pw| load_secret_key(path, Some(&pw)).ok());
                if found.is_none() {
                    keyring_forget(&account);
                    let kind = SecretKind::KeyPassphrase { key_path: account.clone() };
                    if let Some((pw, remember)) = prompter.ask_secret(kind, p.alias.clone()).await {
                        found = load_secret_key(path, Some(&pw)).ok();
                        if found.is_some() && remember {
                            keyring_set(&account, &pw);
                        }
                    }
                }
                found
            }
            Err(_) => None,
        };
        let Some(key) = key else { continue };
        let hash = rsa_hash(ssh, key.public_key()).await;
        let res = ssh.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await;
        if let Ok(AuthResult::Success) = res {
            return Ok(true);
        }
    }

    // 3. password
    let account = format!("password:{}", p.alias);
    if let Some(pw) = keyring_get(&account) {
        if let Ok(AuthResult::Success) = ssh.authenticate_password(user, pw).await {
            return Ok(true);
        }
        keyring_forget(&account);
    }
    for _ in 0..3 {
        let Some((pw, remember)) = prompter.ask_secret(SecretKind::Password, p.alias.clone()).await else {
            return Ok(false);
        };
        match ssh.authenticate_password(user, pw.clone()).await {
            Ok(AuthResult::Success) => {
                if remember {
                    keyring_set(&account, &pw);
                }
                return Ok(true);
            }
            Ok(_) => continue,
            Err(e) => return Err(format!("로그인 중 오류: {e}")),
        }
    }
    Ok(false)
}

#[cfg(unix)]
async fn agent_client() -> Option<russh::keys::agent::client::AgentClient<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send>> {
    russh::keys::agent::client::AgentClient::connect_env().await.ok()
}

#[cfg(windows)]
async fn agent_client() -> Option<russh::keys::agent::client::AgentClient<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send>> {
    russh::keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await.ok()
}
