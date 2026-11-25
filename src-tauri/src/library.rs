use crate::archive::ZipIndex;
use crate::source::Source;
use serde::{Deserialize, Serialize};
use crate::natsort;
use std::collections::BTreeMap;
use std::io;

#[derive(Debug, Clone, PartialEq)]
pub struct FoundZip {
    pub rel_path: String,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VolumeRef {
    pub key: String,
    pub zip_path: String,
    pub inner_dir: Option<String>,
    pub title: String,
    pub page_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Work {
    pub id: String,
    pub title: String,
    pub volumes: Vec<VolumeRef>,
}

/// Recursively lists .zip/.cbz files. Hidden folders are skipped; unreadable subfolders are ignored.
pub fn find_zips(src: &dyn Source) -> io::Result<Vec<FoundZip>> {
    let mut out = Vec::new();
    let mut stack = vec![String::new()];
    let mut first = true;
    while let Some(dir) = stack.pop() {
        let entries = match src.list_dir(&dir) {
            Ok(e) => e,
            Err(e) if first => return Err(e),
            Err(_) => continue,
        };
        first = false;
        for e in entries {
            if e.name.starts_with('.') {
                continue;
            }
            let rel = if dir.is_empty() { e.name.clone() } else { format!("{dir}/{}", e.name) };
            if e.is_dir {
                stack.push(rel);
            } else if is_zip(&e.name) {
                out.push(FoundZip { rel_path: rel, size: e.size, mtime: e.mtime });
            }
        }
    }
    Ok(out)
}

fn is_zip(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("cbz"))
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

fn file_stem(rel: &str) -> &str {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    file.rsplit_once('.').map_or(file, |(s, _)| s)
}

/// Groups zips into works: a folder holding zips is a work; a zip at the library root is its own work.
/// Zips with several image folders expand into one volume per folder. Broken or empty zips are left out.
pub fn group_works(zips: &[FoundZip], index_of: &dyn Fn(&FoundZip) -> Option<ZipIndex>) -> Vec<Work> {
    let mut by_work: BTreeMap<String, Vec<&FoundZip>> = BTreeMap::new();
    for z in zips {
        let p = parent(&z.rel_path);
        let id = if p.is_empty() { z.rel_path.clone() } else { p.to_string() };
        by_work.entry(id).or_default().push(z);
    }
    let mut works = Vec::new();
    for (id, mut members) in by_work {
        members.sort_by(|a, b| natsort::cmp(&a.rel_path, &b.rel_path));
        let mut volumes = Vec::new();
        for z in members {
            let Some(index) = index_of(z) else { continue };
            for v in index.volumes {
                let (key, title) = match &v.inner_dir {
                    Some(d) => (format!("{}#{d}", z.rel_path), d.rsplit('/').next().unwrap_or(d).to_string()),
                    None => (z.rel_path.clone(), file_stem(&z.rel_path).to_string()),
                };
                volumes.push(VolumeRef {
                    key,
                    zip_path: z.rel_path.clone(),
                    inner_dir: v.inner_dir,
                    title,
                    page_count: v.pages.len() as u32,
                });
            }
        }
        if volumes.is_empty() {
            continue;
        }
        let title = if parent(&id).is_empty() && is_zip(&id) && !id.contains('/') {
            file_stem(&id).to_string()
        } else {
            id.rsplit('/').next().unwrap_or(&id).to_string()
        };
        works.push(Work { id, title, volumes });
    }
    works.sort_by(|a, b| natsort::cmp(&a.title, &b.title));
    works
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ArchiveVolume, PageRef};
    use crate::source::{DirEntry, ReadSeek};
    use std::collections::HashMap;

    /// In-memory tree: path -> is_dir. Files have size 1.
    struct FakeSource(Vec<(&'static str, bool)>);

    impl Source for FakeSource {
        fn list_dir(&self, rel: &str) -> io::Result<Vec<DirEntry>> {
            Ok(self
                .0
                .iter()
                .filter(|(p, _)| p.rsplit_once('/').map_or("", |(d, _)| d) == rel)
                .map(|(p, d)| DirEntry { name: p.rsplit('/').next().unwrap().into(), is_dir: *d, size: 1, mtime: 1 })
                .collect())
        }
        fn open(&self, _rel: &str) -> io::Result<Box<dyn ReadSeek>> {
            unreachable!()
        }
    }

    fn vols(dirs: &[&str]) -> ZipIndex {
        let page = || vec![PageRef { entry_index: 0, name: "1.jpg".into() }];
        ZipIndex {
            volumes: if dirs.is_empty() {
                vec![ArchiveVolume { inner_dir: None, pages: page() }]
            } else {
                dirs.iter().map(|d| ArchiveVolume { inner_dir: Some(d.to_string()), pages: page() }).collect()
            },
        }
    }

    fn fixture() -> (FakeSource, HashMap<&'static str, Option<ZipIndex>>) {
        let kingdom: Vec<String> = (526..=635).map(|n| format!("킹덤 (KINGDIM)  ({n})")).collect();
        let kingdom2: Vec<String> = (636..=642).map(|n| format!("킹덤{n}화")).collect();
        let src = FakeSource(vec![
            ("호문쿨루스.zip", false),
            ("생존게임", true),
            ("생존게임/생존게임 01.zip", false),
            ("생존게임/생존게임 02.zip", false),
            ("생존게임/생존게임 10.zip", false),
            ("이토준지", true),
            ("이토준지/[완] 소용돌이 (1-3)", true),
            ("이토준지/[완] 소용돌이 (1-3)/소용돌이 01권.zip", false),
            ("이토준지/[완] 소용돌이 (1-3)/소용돌이 02권.zip", false),
            ("이토준지/[완] 소용돌이 (1-3)/소용돌이 03권 (완결).zip", false),
            ("킹덤", true),
            ("킹덤/킹덤636-642.zip", false),
            ("킹덤/킹덤 526~635화.zip", false),
            ("동물의 숲", true),
            ("동물의 숲/a.jpg", false),
            ("broken.zip", false),
            ("empty.CBZ", false),
            (".hidden", true),
            (".hidden/x.zip", false),
        ]);
        let mut idx: HashMap<&'static str, Option<ZipIndex>> = HashMap::new();
        idx.insert("호문쿨루스.zip", Some(vols(&["호문쿨루스 01권", "호문쿨루스 02권", "호문쿨루스 03권"])));
        for p in ["생존게임/생존게임 01.zip", "생존게임/생존게임 02.zip", "생존게임/생존게임 10.zip",
                  "이토준지/[완] 소용돌이 (1-3)/소용돌이 01권.zip", "이토준지/[완] 소용돌이 (1-3)/소용돌이 02권.zip",
                  "이토준지/[완] 소용돌이 (1-3)/소용돌이 03권 (완결).zip"] {
            idx.insert(p, Some(vols(&[])));
        }
        idx.insert("킹덤/킹덤 526~635화.zip", Some(vols(&kingdom.iter().map(String::as_str).collect::<Vec<_>>())));
        idx.insert("킹덤/킹덤636-642.zip", Some(vols(&kingdom2.iter().map(String::as_str).collect::<Vec<_>>())));
        idx.insert("broken.zip", None);
        idx.insert("empty.CBZ", Some(ZipIndex::default()));
        (src, idx)
    }

    fn works() -> Vec<Work> {
        let (src, idx) = fixture();
        let zips = find_zips(&src).unwrap();
        group_works(&zips, &|z: &FoundZip| idx.get(z.rel_path.as_str()).cloned().flatten())
    }

    #[test]
    fn find_zips_skips_non_zip_and_hidden() {
        let (src, _) = fixture();
        let mut paths: Vec<String> = find_zips(&src).unwrap().into_iter().map(|z| z.rel_path).collect();
        paths.sort();
        assert!(paths.contains(&"empty.CBZ".to_string()));
        assert!(!paths.iter().any(|p| p.ends_with(".jpg") || p.starts_with(".hidden")));
        assert_eq!(paths.len(), 11);
    }

    #[test]
    fn work_titles_exclude_broken_empty_and_image_folders() {
        let titles: Vec<String> = works().into_iter().map(|w| w.title).collect();
        assert_eq!(titles, ["[완] 소용돌이 (1-3)", "생존게임", "킹덤", "호문쿨루스"]);
    }

    #[test]
    fn volumes_in_natural_order_with_titles() {
        let w = works();
        let survival = w.iter().find(|w| w.title == "생존게임").unwrap();
        assert_eq!(survival.id, "생존게임");
        let titles: Vec<&str> = survival.volumes.iter().map(|v| v.title.as_str()).collect();
        assert_eq!(titles, ["생존게임 01", "생존게임 02", "생존게임 10"]);
        assert_eq!(survival.volumes[0].key, "생존게임/생존게임 01.zip");
        assert_eq!(survival.volumes[0].page_count, 1);
    }

    #[test]
    fn multi_volume_zips_expand_in_folder_work() {
        let w = works();
        let kingdom = w.iter().find(|w| w.title == "킹덤").unwrap();
        assert_eq!(kingdom.volumes.len(), 117);
        assert_eq!(kingdom.volumes[0].key, "킹덤/킹덤 526~635화.zip#킹덤 (KINGDIM)  (526)");
        assert_eq!(kingdom.volumes[0].title, "킹덤 (KINGDIM)  (526)");
        assert_eq!(kingdom.volumes[110].title, "킹덤636화");
    }

    #[test]
    fn root_zip_is_its_own_work() {
        let w = works();
        let h = w.iter().find(|w| w.title == "호문쿨루스").unwrap();
        assert_eq!(h.id, "호문쿨루스.zip");
        assert_eq!(h.volumes.len(), 3);
        assert_eq!(h.volumes[0].inner_dir.as_deref(), Some("호문쿨루스 01권"));
    }

    #[test]
    #[ignore]
    fn sample_folder_works() {
        let Ok(dir) = std::env::var("COMICS_SAMPLE_DIR") else { return };
        let src = crate::source::local::LocalSource::new(dir.into());
        let zips = find_zips(&src).unwrap();
        let w = group_works(&zips, &|z: &FoundZip| ZipIndex::read(src.open(&z.rel_path).ok()?).ok());
        let get = |t: &str| w.iter().find(|w| w.title == t);
        assert_eq!(get("슬램덩크 완전판").unwrap().volumes.len(), 25);
        assert!(get("드래곤볼 완전판").is_some());
        assert!(get("번역판").is_some());
        assert!(get("동물의 숲 만화").is_none());
    }
}
