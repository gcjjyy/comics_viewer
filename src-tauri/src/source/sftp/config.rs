//! Minimal ~/.ssh/config reader for the directives this app uses:
//! Host, HostName, User, Port, IdentityFile, IdentitiesOnly, HostKeyAlgorithms.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct HostParams {
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub port: u16,
    pub identity_files: Vec<PathBuf>,
    pub identities_only: bool,
    pub extra_host_key_algs: Vec<String>,
}

/// Concrete Host aliases (patterns with *, ? or ! are skipped), in file order.
pub fn list_hosts(config_text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (key, value) in lines(config_text) {
        if key == "host" {
            for p in value.split_whitespace() {
                if !p.contains(['*', '?', '!']) && !out.iter().any(|h| h == p) {
                    out.push(p.to_string());
                }
            }
        }
    }
    out
}

/// Resolves the effective parameters for `alias` (first obtained value wins, as in OpenSSH).
pub fn resolve(alias: &str, config_text: &str, home: &Path, local_user: &str) -> HostParams {
    let (mut hostname, mut user, mut port, mut only, mut algs) = (None, None, None, None, None);
    let mut identity_files = Vec::new();
    let mut active = true; // directives before the first Host apply to every host
    for (key, value) in lines(config_text) {
        if key == "host" {
            active = host_matches(&value, alias);
            continue;
        }
        if key == "match" {
            active = false;
            continue;
        }
        if !active {
            continue;
        }
        match key.as_str() {
            "hostname" => hostname = hostname.or(Some(value)),
            "user" => user = user.or(Some(value)),
            "port" => port = port.or(value.parse().ok()),
            "identityfile" => identity_files.push(expand_home(&value, home)),
            "identitiesonly" => only = only.or(Some(value.eq_ignore_ascii_case("yes"))),
            "hostkeyalgorithms" => {
                algs = algs.or(Some(
                    value.trim_start_matches(['+', '^']).split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
                ))
            }
            _ => {}
        }
    }
    if identity_files.is_empty() {
        identity_files = ["id_ed25519", "id_ecdsa", "id_rsa"]
            .iter()
            .map(|n| home.join(".ssh").join(n))
            .filter(|p| p.exists())
            .collect();
    }
    HostParams {
        alias: alias.to_string(),
        hostname: hostname.unwrap_or_else(|| alias.to_string()),
        user: user.unwrap_or_else(|| local_user.to_string()),
        port: port.unwrap_or(22),
        identity_files,
        identities_only: only.unwrap_or(false),
        extra_host_key_algs: algs.unwrap_or_default(),
    }
}

/// (lowercased keyword, value) pairs; supports "Key value" and "Key=value", strips quotes and comments.
fn lines(text: &str) -> impl Iterator<Item = (String, String)> + '_ {
    text.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let split = line.find(|c: char| c.is_whitespace() || c == '=')?;
        let key = line[..split].to_ascii_lowercase();
        let value = line[split..].trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim();
        Some((key, value.trim_matches('"').to_string()))
    })
}

fn host_matches(patterns: &str, alias: &str) -> bool {
    let mut matched = false;
    for p in patterns.split_whitespace() {
        if let Some(neg) = p.strip_prefix('!') {
            if glob(neg, alias) {
                return false;
            }
        } else if glob(p, alias) {
            matched = true;
        }
    }
    matched
}

fn glob(pattern: &str, s: &str) -> bool {
    fn go(p: &[char], s: &[char]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], s) || (!s.is_empty() && go(p, &s[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => go(&p[1..], &s[1..]),
            _ => false,
        }
    }
    go(&pattern.chars().collect::<Vec<_>>(), &s.chars().collect::<Vec<_>>())
}

fn expand_home(p: &str, home: &Path) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "Host nas\n  HostName 10.0.0.5\n  User me\n  Port 2222\n  IdentityFile ~/.ssh/nas_key\n  IdentitiesOnly yes\n\
                       \nHost old\n  HostName old.example\n  HostKeyAlgorithms +ssh-rsa\n\nHost *\n  ServerAliveInterval 30\n  User fallback\n";

    #[test]
    fn lists_concrete_hosts_only() {
        assert_eq!(list_hosts(CFG), ["nas", "old"]);
        assert_eq!(list_hosts("Host a b\nHost !c d*\nhost=e\n"), ["a", "b", "e"]);
    }

    #[test]
    fn resolves_explicit_values() {
        let p = resolve("nas", CFG, Path::new("/h"), "u");
        assert_eq!((p.hostname.as_str(), p.user.as_str(), p.port), ("10.0.0.5", "me", 2222));
        assert_eq!(p.identity_files, [PathBuf::from("/h/.ssh/nas_key")]);
        assert!(p.identities_only);
        assert!(p.extra_host_key_algs.is_empty());
    }

    #[test]
    fn wildcard_section_fills_missing_values_and_default_key() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".ssh")).unwrap();
        std::fs::write(home.path().join(".ssh/id_ed25519"), "k").unwrap();
        let o = resolve("old", CFG, home.path(), "u");
        assert_eq!(o.extra_host_key_algs, ["ssh-rsa"]);
        assert_eq!(o.port, 22);
        assert_eq!(o.user, "fallback");
        assert_eq!(o.hostname, "old.example");
        assert!(!o.identities_only);
        assert_eq!(o.identity_files, [home.path().join(".ssh/id_ed25519")]);
    }

    #[test]
    fn unknown_alias_uses_defaults() {
        let p = resolve("example.org", "", Path::new("/nonexistent"), "u");
        assert_eq!((p.hostname.as_str(), p.user.as_str(), p.port), ("example.org", "u", 22));
        assert!(p.identity_files.is_empty());
    }

    #[test]
    fn equals_syntax_quotes_and_case() {
        let cfg = "HOST box\n hostname=\"my host\"\n PORT = 2200\n";
        let p = resolve("box", cfg, Path::new("/h"), "u");
        assert_eq!((p.hostname.as_str(), p.port), ("my host", 2200));
    }

    #[test]
    #[ignore]
    fn user_config_lists_hosts() {
        let home = std::env::var("HOME").unwrap();
        let text = std::fs::read_to_string(Path::new(&home).join(".ssh/config")).unwrap();
        assert_eq!(list_hosts(&text).len(), 4);
    }
}
