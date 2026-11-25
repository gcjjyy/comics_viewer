use crate::archive::ZipIndex;
use crate::direction::Direction;
use crate::library::{VolumeRef, Work};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

pub type Result<T> = rusqlite::Result<T>;

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LibraryKind {
    Local,
    Sftp,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Library {
    pub id: i64,
    pub kind: LibraryKind,
    pub name: String,
    pub host: Option<String>,
    pub root: String,
    pub direction: Option<Direction>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct BookState {
    pub page: u32,
    pub page_count: u32,
    pub shift: bool,
    pub direction: Option<Direction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecentBook {
    pub library_id: i64,
    pub key: String,
    pub state: BookState,
}

pub struct Db {
    conn: Connection,
}

fn dir_col(d: Option<Direction>) -> Option<&'static str> {
    d.map(Direction::as_str)
}

fn dir_from(s: Option<String>) -> Option<Direction> {
    s.as_deref().and_then(Direction::parse)
}

fn to_json<T: Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Db { conn })
    }

    pub fn add_library(&self, kind: LibraryKind, name: &str, host: Option<&str>, root: &str) -> Result<i64> {
        let kind = match kind {
            LibraryKind::Local => "local",
            LibraryKind::Sftp => "sftp",
        };
        self.conn.execute(
            "INSERT INTO libraries (kind, name, host, root) VALUES (?1, ?2, ?3, ?4)",
            params![kind, name, host, root],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_library(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM libraries WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn libraries(&self) -> Result<Vec<Library>> {
        let mut stmt = self.conn.prepare("SELECT id, kind, name, host, root, direction FROM libraries ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(1)?;
            Ok(Library {
                id: r.get(0)?,
                kind: if kind == "sftp" { LibraryKind::Sftp } else { LibraryKind::Local },
                name: r.get(2)?,
                host: r.get(3)?,
                root: r.get(4)?,
                direction: dir_from(r.get(5)?),
            })
        })?;
        rows.collect()
    }

    pub fn set_library_direction(&self, id: i64, d: Option<Direction>) -> Result<()> {
        self.conn.execute("UPDATE libraries SET direction = ?2 WHERE id = ?1", params![id, dir_col(d)])?;
        Ok(())
    }

    pub fn set_folder_direction(&self, lib: i64, work_id: &str, d: Option<Direction>) -> Result<()> {
        match d {
            Some(d) => self.conn.execute(
                "INSERT INTO folder_settings (library_id, work_id, direction) VALUES (?1, ?2, ?3)
                 ON CONFLICT (library_id, work_id) DO UPDATE SET direction = excluded.direction",
                params![lib, work_id, d.as_str()],
            )?,
            None => self.conn.execute(
                "DELETE FROM folder_settings WHERE library_id = ?1 AND work_id = ?2",
                params![lib, work_id],
            )?,
        };
        Ok(())
    }

    pub fn folder_direction(&self, lib: i64, work_id: &str) -> Result<Option<Direction>> {
        let d: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT direction FROM folder_settings WHERE library_id = ?1 AND work_id = ?2",
                params![lib, work_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(dir_from(d.flatten()))
    }

    pub fn book_state(&self, lib: i64, key: &str) -> Result<Option<BookState>> {
        self.conn
            .query_row(
                "SELECT page, page_count, shift, direction FROM book_state WHERE library_id = ?1 AND volume_key = ?2",
                params![lib, key],
                |r| {
                    Ok(BookState {
                        page: r.get(0)?,
                        page_count: r.get(1)?,
                        shift: r.get::<_, i64>(2)? != 0,
                        direction: dir_from(r.get(3)?),
                    })
                },
            )
            .optional()
    }

    pub fn save_page(&self, lib: i64, key: &str, page: u32, page_count: u32) -> Result<()> {
        self.conn.execute(
            "INSERT INTO book_state (library_id, volume_key, page, page_count, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (library_id, volume_key) DO UPDATE
             SET page = excluded.page, page_count = excluded.page_count, updated_at = excluded.updated_at",
            params![lib, key, page, page_count, now()],
        )?;
        Ok(())
    }

    pub fn set_book_shift(&self, lib: i64, key: &str, shift: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO book_state (library_id, volume_key, shift) VALUES (?1, ?2, ?3)
             ON CONFLICT (library_id, volume_key) DO UPDATE SET shift = excluded.shift",
            params![lib, key, shift as i64],
        )?;
        Ok(())
    }

    pub fn set_book_direction(&self, lib: i64, key: &str, d: Option<Direction>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO book_state (library_id, volume_key, direction) VALUES (?1, ?2, ?3)
             ON CONFLICT (library_id, volume_key) DO UPDATE SET direction = excluded.direction",
            params![lib, key, dir_col(d)],
        )?;
        Ok(())
    }

    pub fn recent_books(&self, limit: u32) -> Result<Vec<RecentBook>> {
        let mut stmt = self.conn.prepare(
            "SELECT library_id, volume_key, page, page_count, shift, direction FROM book_state
             WHERE updated_at > 0 ORDER BY updated_at DESC, rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(RecentBook {
                library_id: r.get(0)?,
                key: r.get(1)?,
                state: BookState {
                    page: r.get(2)?,
                    page_count: r.get(3)?,
                    shift: r.get::<_, i64>(4)? != 0,
                    direction: dir_from(r.get(5)?),
                },
            })
        })?;
        rows.collect()
    }

    pub fn global_direction(&self) -> Result<Direction> {
        let v: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = 'direction'", [], |r| r.get(0))
            .optional()?;
        Ok(dir_from(v).unwrap_or(Direction::Rtl))
    }

    pub fn set_global_direction(&self, d: Direction) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES ('direction', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [d.as_str()],
        )?;
        Ok(())
    }

    pub fn zip_index_cached(&self, lib: i64, rel: &str, size: u64, mtime: i64) -> Result<Option<ZipIndex>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT index_json FROM zip_cache WHERE library_id = ?1 AND rel_path = ?2 AND size = ?3 AND mtime = ?4",
                params![lib, rel, size as i64, mtime],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }

    pub fn put_zip_index(&self, lib: i64, rel: &str, size: u64, mtime: i64, index: &ZipIndex) -> Result<()> {
        self.conn.execute(
            "INSERT INTO zip_cache (library_id, rel_path, size, mtime, index_json) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (library_id, rel_path) DO UPDATE
             SET size = excluded.size, mtime = excluded.mtime, index_json = excluded.index_json",
            params![lib, rel, size as i64, mtime, to_json(index)?],
        )?;
        Ok(())
    }

    pub fn zip_cached_latest(&self, lib: i64, rel: &str) -> Result<Option<(u64, i64, ZipIndex)>> {
        let row: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT size, mtime, index_json FROM zip_cache WHERE library_id = ?1 AND rel_path = ?2",
                params![lib, rel],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(row.and_then(|(size, mtime, j)| Some((size as u64, mtime, serde_json::from_str(&j).ok()?))))
    }

    pub fn book_states(&self, lib: i64) -> Result<std::collections::HashMap<String, BookState>> {
        let mut stmt = self
            .conn
            .prepare("SELECT volume_key, page, page_count, shift, direction FROM book_state WHERE library_id = ?1")?;
        let rows = stmt.query_map([lib], |r| {
            Ok((
                r.get::<_, String>(0)?,
                BookState {
                    page: r.get(1)?,
                    page_count: r.get(2)?,
                    shift: r.get::<_, i64>(3)? != 0,
                    direction: dir_from(r.get(4)?),
                },
            ))
        })?;
        rows.collect()
    }

    pub fn works_cached(&self, lib: i64) -> Result<Option<Vec<Work>>> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT works_json FROM works_cache WHERE library_id = ?1", [lib], |r| r.get(0))
            .optional()?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }

    pub fn put_works(&self, lib: i64, works: &[Work]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO works_cache (library_id, works_json) VALUES (?1, ?2)
             ON CONFLICT (library_id) DO UPDATE SET works_json = excluded.works_json",
            params![lib, to_json(&works)?],
        )?;
        Ok(())
    }
}

/// (finished volumes, total volumes). A volume is finished when its last page was reached.
pub fn work_progress(volumes: &[VolumeRef], state: impl Fn(&str) -> Option<BookState>) -> (u32, u32) {
    let done = volumes
        .iter()
        // Saved page is the first page of a spread, so the last spread starts at count-2.
        .filter(|v| state(&v.key).is_some_and(|s| v.page_count > 0 && s.page + 2 >= v.page_count))
        .count();
    (done as u32, volumes.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ArchiveVolume, PageRef};

    fn db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).unwrap();
        (dir, db)
    }

    fn vol(key: &str, pages: u32) -> VolumeRef {
        VolumeRef { key: key.into(), zip_path: key.into(), inner_dir: None, title: key.into(), page_count: pages }
    }

    #[test]
    fn global_direction_defaults_to_rtl_and_persists() {
        let (dir, db) = db();
        assert_eq!(db.global_direction().unwrap(), Direction::Rtl);
        db.set_global_direction(Direction::Ltr).unwrap();
        drop(db);
        let db = Db::open(&dir.path().join("t.db")).unwrap();
        assert_eq!(db.global_direction().unwrap(), Direction::Ltr);
    }

    #[test]
    fn libraries_roundtrip_with_direction() {
        let (_d, db) = db();
        let a = db.add_library(LibraryKind::Local, "만화", None, "/x/만화").unwrap();
        let b = db.add_library(LibraryKind::Sftp, "nas", Some("nas"), "/data").unwrap();
        db.set_library_direction(b, Some(Direction::Ltr)).unwrap();
        let libs = db.libraries().unwrap();
        assert_eq!(libs.len(), 2);
        assert_eq!((libs[0].id, libs[0].kind, libs[0].direction), (a, LibraryKind::Local, None));
        assert_eq!((libs[1].host.as_deref(), libs[1].direction), (Some("nas"), Some(Direction::Ltr)));
    }

    #[test]
    fn book_state_saves_and_overwrites() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        assert_eq!(db.book_state(lib, "k").unwrap(), None);
        db.save_page(lib, "k", 3, 100).unwrap();
        db.save_page(lib, "k", 7, 100).unwrap();
        db.set_book_shift(lib, "k", true).unwrap();
        db.set_book_direction(lib, "k", Some(Direction::Ltr)).unwrap();
        assert_eq!(
            db.book_state(lib, "k").unwrap(),
            Some(BookState { page: 7, page_count: 100, shift: true, direction: Some(Direction::Ltr) })
        );
    }

    #[test]
    fn shift_without_reading_is_not_recent() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        db.set_book_shift(lib, "only-shift", true).unwrap();
        db.save_page(lib, "read", 1, 10).unwrap();
        let recent: Vec<String> = db.recent_books(10).unwrap().into_iter().map(|r| r.key).collect();
        assert_eq!(recent, ["read"]);
    }

    #[test]
    fn folder_direction_set_and_clear() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        db.set_folder_direction(lib, "생존게임", Some(Direction::Ltr)).unwrap();
        assert_eq!(db.folder_direction(lib, "생존게임").unwrap(), Some(Direction::Ltr));
        db.set_folder_direction(lib, "생존게임", None).unwrap();
        assert_eq!(db.folder_direction(lib, "생존게임").unwrap(), None);
    }

    #[test]
    fn zip_cache_invalidated_by_size_or_mtime() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        let idx = ZipIndex { volumes: vec![ArchiveVolume { inner_dir: None, pages: vec![PageRef { entry_index: 0, name: "1.jpg".into() }] }] };
        db.put_zip_index(lib, "a.zip", 10, 5, &idx).unwrap();
        assert_eq!(db.zip_index_cached(lib, "a.zip", 10, 5).unwrap(), Some(idx));
        assert_eq!(db.zip_index_cached(lib, "a.zip", 11, 5).unwrap(), None);
        assert_eq!(db.zip_index_cached(lib, "a.zip", 10, 6).unwrap(), None);
    }

    #[test]
    fn zip_cached_latest_ignores_stat() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        assert_eq!(db.zip_cached_latest(lib, "a.zip").unwrap(), None);
        db.put_zip_index(lib, "a.zip", 10, 5, &ZipIndex::default()).unwrap();
        assert_eq!(db.zip_cached_latest(lib, "a.zip").unwrap(), Some((10, 5, ZipIndex::default())));
    }

    #[test]
    fn book_states_for_library() {
        let (_d, db) = db();
        let a = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        let b = db.add_library(LibraryKind::Local, "b", None, "/b").unwrap();
        db.save_page(a, "k1", 2, 9).unwrap();
        db.save_page(b, "k2", 1, 9).unwrap();
        let m = db.book_states(a).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m["k1"].page, 2);
    }

    #[test]
    fn works_cache_roundtrip() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        assert_eq!(db.works_cached(lib).unwrap(), None);
        let works = vec![Work { id: "w".into(), title: "w".into(), volumes: vec![vol("w/1.zip", 3)] }];
        db.put_works(lib, &works).unwrap();
        assert_eq!(db.works_cached(lib).unwrap(), Some(works));
    }

    #[test]
    fn remove_library_cascades() {
        let (_d, db) = db();
        let lib = db.add_library(LibraryKind::Local, "a", None, "/a").unwrap();
        db.save_page(lib, "k", 1, 2).unwrap();
        db.put_works(lib, &[]).unwrap();
        db.remove_library(lib).unwrap();
        let lib2 = db.add_library(LibraryKind::Local, "b", None, "/b").unwrap();
        assert!(db.libraries().unwrap().iter().all(|l| l.id != lib));
        assert_eq!(db.book_state(lib, "k").unwrap(), None);
        assert_eq!(db.works_cached(lib).unwrap(), None);
        assert_ne!(lib, lib2);
    }

    #[test]
    fn work_progress_counts_finished_volumes_and_ignores_orphans() {
        let vols = [vol("1", 10), vol("2", 10), vol("3", 10)];
        let state = |k: &str| match k {
            "1" => Some(BookState { page: 9, page_count: 10, shift: false, direction: None }),
            "2" => Some(BookState { page: 7, page_count: 10, shift: false, direction: None }),
            "3" => Some(BookState { page: 8, page_count: 10, shift: false, direction: None }),
            "gone" => Some(BookState { page: 9, page_count: 10, shift: false, direction: None }),
            _ => None,
        };
        assert_eq!(work_progress(&vols, state), (2, 3));
    }
}
