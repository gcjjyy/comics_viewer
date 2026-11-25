use serde::{Deserialize, Serialize};
use crate::encoding::decode_name;
use crate::natsort;
use std::collections::BTreeMap;
use std::io::{Read, Seek};

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PageRef {
    pub entry_index: usize,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ArchiveVolume {
    pub inner_dir: Option<String>,
    pub pages: Vec<PageRef>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct ZipIndex {
    pub volumes: Vec<ArchiveVolume>,
}

impl ZipIndex {
    pub fn read<R: Read + Seek>(r: R) -> Result<ZipIndex, ArchiveError> {
        Self::from_archive(&mut zip::ZipArchive::new(r)?)
    }

    /// Groups image entries by parent folder; two or more image folders make one volume per folder
    /// (root-level images are then dropped), otherwise all images form a single volume.
    pub fn from_archive<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<ZipIndex, ArchiveError> {
        let mut groups: BTreeMap<String, Vec<PageRef>> = BTreeMap::new();
        for (entry_index, raw) in zip.file_names_raw().enumerate() {
            let name = decode_name(raw, false);
            if !is_page(&name) {
                continue;
            }
            let dir = name.rsplit_once('/').map_or("", |(d, _)| d).to_string();
            groups.entry(dir).or_default().push(PageRef { entry_index, name });
        }
        let folder_count = groups.keys().filter(|d| !d.is_empty()).count();
        let volumes = if folder_count >= 2 {
            // Root-level images in a multi-volume zip are set covers, not a volume.
            groups.remove("");
            let mut v: Vec<ArchiveVolume> = groups
                .into_iter()
                .map(|(dir, mut pages)| {
                    pages.sort_by(|a, b| natsort::cmp(&a.name, &b.name));
                    ArchiveVolume { inner_dir: Some(dir), pages }
                })
                .collect();
            v.sort_by(|a, b| natsort::cmp(a.inner_dir.as_deref().unwrap(), b.inner_dir.as_deref().unwrap()));
            v
        } else {
            let mut pages: Vec<PageRef> = groups.into_values().flatten().collect();
            pages.sort_by(|a, b| natsort::cmp(&a.name, &b.name));
            if pages.is_empty() { vec![] } else { vec![ArchiveVolume { inner_dir: None, pages }] }
        };
        Ok(ZipIndex { volumes })
    }
}

pub fn read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, entry_index: usize) -> Result<Vec<u8>, ArchiveError> {
    let mut file = zip.by_index(entry_index)?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

const IMAGE_EXTS: [&str; 6] = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];

fn is_page(name: &str) -> bool {
    if name.ends_with('/') || name.starts_with("__MACOSX/") || name.contains("/__MACOSX/") {
        return false;
    }
    let file = name.rsplit('/').next().unwrap_or(name);
    if file.starts_with("._") {
        return false;
    }
    file.rsplit_once('.')
        .is_some_and(|(_, ext)| IMAGE_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockcache::{tests::MemRange, BlockReader, BLOCK_SIZE, MAX_BLOCKS};
    use crate::testutil::{cp949, make_zip, JPG};
    use std::io::Cursor;

    fn dirs(idx: &ZipIndex) -> Vec<Option<String>> {
        idx.volumes.iter().map(|v| v.inner_dir.clone()).collect()
    }

    #[test]
    fn multi_folder_zip_is_volumes_in_natural_order() {
        let n = [
            cp949("슬램덩크 완전판 10권/001.jpg"),
            cp949("슬램덩크 완전판 02권/002.jpg"),
            cp949("슬램덩크 완전판 02권/001.jpg"),
            cp949("슬램덩크 완전판 01권/001.jpg"),
            cp949("슬램덩크 완전판 01권/Thumbs.db"),
            b"readme.txt".to_vec(),
        ];
        let z = make_zip(&[
            (&n[0], false, JPG), (&n[1], false, JPG), (&n[2], false, JPG),
            (&n[3], false, JPG), (&n[4], false, b"x"), (&n[5], false, b"x"),
        ]);
        let idx = ZipIndex::read(Cursor::new(z)).unwrap();
        assert_eq!(
            dirs(&idx),
            [Some("슬램덩크 완전판 01권".into()), Some("슬램덩크 완전판 02권".into()), Some("슬램덩크 완전판 10권".into())]
        );
        let names: Vec<_> = idx.volumes[1].pages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["슬램덩크 완전판 02권/001.jpg", "슬램덩크 완전판 02권/002.jpg"]);
        assert_eq!(idx.volumes[1].pages[0].entry_index, 2);
    }

    #[test]
    fn root_images_dropped_when_multiple_folders() {
        let z = make_zip(&[(b"[Cover].jpg", false, JPG), (b"v1/1.jpg", false, JPG), (b"v2/1.jpg", false, JPG)]);
        let idx = ZipIndex::read(Cursor::new(z)).unwrap();
        assert_eq!(dirs(&idx), [Some("v1".into()), Some("v2".into())]);
    }

    #[test]
    fn root_image_and_single_folder_merge_into_one_volume() {
        let z = make_zip(&[(b"cover.jpg", false, JPG), (b"01/1.jpg", false, JPG)]);
        let idx = ZipIndex::read(Cursor::new(z)).unwrap();
        assert_eq!(dirs(&idx), [None]);
        assert_eq!(idx.volumes[0].pages.len(), 2);
    }

    #[test]
    fn single_folder_is_one_volume() {
        let z = make_zip(&[(b"01/a.jpg", false, JPG), (b"01/b.jpg", false, JPG)]);
        let idx = ZipIndex::read(Cursor::new(z)).unwrap();
        assert_eq!(dirs(&idx), [None]);
        assert_eq!(idx.volumes[0].pages.len(), 2);
    }

    #[test]
    fn root_images_are_one_volume() {
        let z = make_zip(&[(b"2.jpg", false, JPG), (b"10.jpg", false, JPG), (b"1.jpg", false, JPG)]);
        let idx = ZipIndex::read(Cursor::new(z)).unwrap();
        assert_eq!(dirs(&idx), [None]);
        let names: Vec<_> = idx.volumes[0].pages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["1.jpg", "2.jpg", "10.jpg"]);
    }

    #[test]
    fn macos_resource_forks_are_ignored() {
        let z = make_zip(&[
            (b"01/001.jpg", false, JPG),
            (b"__MACOSX/01/._001.jpg", false, b"x"),
            (b"01/._001.jpg", false, b"x"),
        ]);
        let idx = ZipIndex::read(Cursor::new(z)).unwrap();
        assert_eq!(dirs(&idx), [None]);
        assert_eq!(idx.volumes[0].pages.len(), 1);
    }

    #[test]
    fn no_images_means_no_volumes() {
        let z = make_zip(&[(b"a.txt", false, b"x"), (b"Thumbs.db", false, b"x")]);
        assert!(ZipIndex::read(Cursor::new(z)).unwrap().volumes.is_empty());
    }

    #[test]
    fn image_extensions_case_insensitive() {
        let z = make_zip(&[(b"A.JPG", false, JPG), (b"b.png", false, JPG), (b"c.webp", false, JPG), (b"d.Jpeg", false, JPG)]);
        assert_eq!(ZipIndex::read(Cursor::new(z)).unwrap().volumes[0].pages.len(), 4);
    }

    #[test]
    fn utf8_flagged_names() {
        let z = make_zip(&[("총몽 - 1/001.jpg".as_bytes(), true, JPG), ("총몽 - 2/001.jpg".as_bytes(), true, JPG)]);
        assert_eq!(dirs(&ZipIndex::read(Cursor::new(z)).unwrap()), [Some("총몽 - 1".into()), Some("총몽 - 2".into())]);
    }

    #[test]
    fn read_entry_returns_bytes() {
        let z = make_zip(&[(b"x/1.jpg", false, JPG)]);
        let mut zip = zip::ZipArchive::new(Cursor::new(z)).unwrap();
        assert_eq!(read_entry(&mut zip, 0).unwrap(), JPG);
    }

    #[test]
    fn opening_one_page_fetches_small_fraction() {
        let big = vec![7u8; 1_000_000];
        let names: Vec<Vec<u8>> = (0..200).map(|i| format!("v/{i:03}.jpg").into_bytes()).collect();
        let entries: Vec<(&[u8], bool, &[u8])> = names.iter().map(|n| (n.as_slice(), false, big.as_slice())).collect();
        let z = make_zip(&entries);
        let total = z.len() as u64;
        let reader = BlockReader::new(MemRange::new(z), BLOCK_SIZE, MAX_BLOCKS);
        let mut zip = zip::ZipArchive::new(reader).unwrap();
        let idx = ZipIndex::from_archive(&mut zip).unwrap();
        let first = idx.volumes[0].pages[0].entry_index;
        assert_eq!(read_entry(&mut zip, first).unwrap().len(), 1_000_000);
        let fetched = zip.into_inner().bytes_fetched();
        assert!(fetched < total / 10, "fetched {fetched} of {total}");
    }

    #[test]
    #[ignore]
    fn sample_folder_indexes() {
        let Ok(dir) = std::env::var("COMICS_SAMPLE_DIR") else { return };
        let count = |rel: &str| {
            let f = std::fs::File::open(std::path::Path::new(&dir).join(rel)).unwrap();
            ZipIndex::read(f).unwrap().volumes.len()
        };
        assert_eq!(count("슬램덩크 완전판.zip"), 25); // 24 volumes + artbook; root [Cover].jpg dropped
        assert_eq!(count("킹덤/킹덤 526~635화.zip"), 110);
        assert_eq!(count("생존게임/생존게임 01.zip"), 1);
    }
}
