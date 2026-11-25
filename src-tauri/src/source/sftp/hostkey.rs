use russh::keys::{check_known_hosts_path, Error as KeyError, PublicKey};
use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum HostKeyStatus {
    Known,
    Unknown,
    Mismatch,
}

/// Looks `host`/`port` up in an OpenSSH known_hosts file (hashed entries and [host]:port included).
pub fn host_key_status(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> HostKeyStatus {
    match check_known_hosts_path(host, port, key, known_hosts) {
        Ok(true) => HostKeyStatus::Known,
        Err(KeyError::KeyChanged { .. }) => HostKeyStatus::Mismatch,
        _ => HostKeyStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::known_hosts::learn_known_hosts_path;
    use russh::keys::parse_public_key_base64;

    const K1: &str = "AAAAC3NzaC1lZDI1NTE5AAAAILcgxF7aksB16xJ8kvQyeHO5K3zPwcviEt0JIWwcQcOT";
    const K2: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIF3ySrxdDmf6fPgAxrF3k0BeoS6og9wVrrl5mFHIyFJT";

    #[test]
    fn unknown_known_and_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        let k1 = parse_public_key_base64(K1).unwrap();
        let k2 = parse_public_key_base64(K2).unwrap();
        assert_eq!(host_key_status(&path, "nas.local", 22, &k1), HostKeyStatus::Unknown);
        learn_known_hosts_path("nas.local", 22, &k1, &path).unwrap();
        assert_eq!(host_key_status(&path, "nas.local", 22, &k1), HostKeyStatus::Known);
        assert_eq!(host_key_status(&path, "nas.local", 22, &k2), HostKeyStatus::Mismatch);
        assert_eq!(host_key_status(&path, "nas.local", 2222, &k1), HostKeyStatus::Unknown);
        learn_known_hosts_path("nas.local", 2222, &k1, &path).unwrap();
        assert_eq!(host_key_status(&path, "nas.local", 2222, &k1), HostKeyStatus::Known);
    }
}
