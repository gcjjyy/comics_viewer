//! Volume keys and comic:// URL paths.
use crate::library::Work;
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};

/// Splits "<zip>#<inner dir>" into its parts. The zip part is the shortest prefix ending in .zip/.cbz.
pub fn split_key(key: &str) -> (&str, Option<&str>) {
    let lower = key.to_ascii_lowercase();
    for (i, _) in key.match_indices('#') {
        if lower[..i].ends_with(".zip") || lower[..i].ends_with(".cbz") {
            return (&key[..i], Some(&key[i + 1..]));
        }
    }
    (key, None)
}

/// Previous and next volume keys within the work that holds `key`.
pub fn neighbor_keys(works: &[Work], key: &str) -> (Option<String>, Option<String>) {
    for w in works {
        if let Some(i) = w.volumes.iter().position(|v| v.key == key) {
            let prev = i.checked_sub(1).map(|j| w.volumes[j].key.clone());
            let next = w.volumes.get(i + 1).map(|v| v.key.clone());
            return (prev, next);
        }
    }
    (None, None)
}

#[derive(Debug, PartialEq)]
pub enum ComicPath {
    Page { library_id: i64, key: String, index: usize },
    Thumb { library_id: i64, key: String },
}

/// Parses a comic:// request path such as "/page/1/<encoded key>/3".
pub fn parse_comic_path(path: &str) -> Option<ComicPath> {
    let mut parts = path.trim_start_matches('/').split('/');
    let kind = parts.next()?;
    let library_id = parts.next()?.parse().ok()?;
    let key = percent_decode_str(parts.next()?).decode_utf8().ok()?.into_owned();
    let result = match kind {
        "page" => ComicPath::Page { library_id, key, index: parts.next()?.parse().ok()? },
        "thumb" => ComicPath::Thumb { library_id, key },
        _ => return None,
    };
    parts.next().is_none().then_some(result)
}

/// Builds the URL the webview uses for a comic:// path (platform-specific form).
pub fn comic_url(path: &ComicPath) -> String {
    let base = if cfg!(windows) { "http://comic.localhost" } else { "comic://localhost" };
    match path {
        ComicPath::Page { library_id, key, index } => format!("{base}/page/{library_id}/{}/{index}", enc(key)),
        ComicPath::Thumb { library_id, key } => format!("{base}/thumb/{library_id}/{}", enc(key)),
    }
}

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::VolumeRef;

    fn work(id: &str, keys: &[&str]) -> Work {
        Work {
            id: id.into(),
            title: id.into(),
            volumes: keys
                .iter()
                .map(|k| VolumeRef { key: k.to_string(), zip_path: k.to_string(), inner_dir: None, title: k.to_string(), page_count: 1 })
                .collect(),
        }
    }

    #[test]
    fn split_key_plain_and_inner() {
        assert_eq!(split_key("생존게임/생존게임 01.zip"), ("생존게임/생존게임 01.zip", None));
        assert_eq!(split_key("슬램덩크.zip#슬램덩크 01권"), ("슬램덩크.zip", Some("슬램덩크 01권")));
        assert_eq!(split_key("a#b.ZIP#c#d"), ("a#b.ZIP", Some("c#d")));
        assert_eq!(split_key("x.cbz"), ("x.cbz", None));
    }

    #[test]
    fn neighbors_within_work() {
        let works = [work("a", &["a/1.zip", "a/2.zip", "a/3.zip"]), work("b", &["b/1.zip"])];
        assert_eq!(neighbor_keys(&works, "a/2.zip"), (Some("a/1.zip".into()), Some("a/3.zip".into())));
        assert_eq!(neighbor_keys(&works, "a/1.zip"), (None, Some("a/2.zip".into())));
        assert_eq!(neighbor_keys(&works, "a/3.zip"), (Some("a/2.zip".into()), None));
        assert_eq!(neighbor_keys(&works, "b/1.zip"), (None, None));
        assert_eq!(neighbor_keys(&works, "gone.zip"), (None, None));
    }

    #[test]
    fn comic_path_roundtrip_with_korean_and_hash() {
        let p = ComicPath::Page { library_id: 3, key: "슬램덩크 완전판.zip#슬램덩크 01권 (완결)".into(), index: 12 };
        let url = comic_url(&p);
        let path = url.split_once("localhost").unwrap().1;
        assert_eq!(parse_comic_path(path), Some(p));
        let t = ComicPath::Thumb { library_id: 1, key: "a/b c.zip".into() };
        assert_eq!(parse_comic_path(comic_url(&t).split_once("localhost").unwrap().1), Some(t));
    }

    #[test]
    fn comic_url_platform_form() {
        let url = comic_url(&ComicPath::Thumb { library_id: 1, key: "a".into() });
        if cfg!(windows) {
            assert!(url.starts_with("http://comic.localhost/thumb/1/"));
        } else {
            assert!(url.starts_with("comic://localhost/thumb/1/"));
        }
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(parse_comic_path("/page/x/a/1"), None);
        assert_eq!(parse_comic_path("/other/1/a"), None);
        assert_eq!(parse_comic_path("/page/1/a"), None);
    }
}
