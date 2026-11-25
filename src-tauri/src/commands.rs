use crate::db::{work_progress, LibraryKind};
use crate::direction::{resolve, Direction};
use crate::keys::{comic_url, neighbor_keys, ComicPath};
use crate::library::Work;
use crate::state::{err, AppState};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type St<'a> = State<'a, Arc<AppState>>;

async fn blocking<T: Send + 'static>(
    state: &St<'_>,
    f: impl FnOnce(&AppState) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(&s)).await.map_err(err)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    id: i64,
    kind: LibraryKind,
    name: String,
    host: Option<String>,
    root: String,
    direction: Option<Direction>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkView {
    id: String,
    title: String,
    volume_count: u32,
    read_volumes: u32,
    cover_url: String,
    direction: Direction,
    folder_direction: Option<Direction>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeView {
    key: String,
    title: String,
    page_count: u32,
    page: Option<u32>,
    cover_url: String,
    direction: Direction,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentView {
    library_id: i64,
    key: String,
    title: String,
    work_id: String,
    work_title: String,
    page: u32,
    page_count: u32,
    cover_url: String,
    direction: Direction,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedVolume {
    key: String,
    title: String,
    work_id: String,
    work_title: String,
    pages: Vec<String>,
    page: u32,
    shift: bool,
    direction: Direction,
    /// Direction from library/folder/global settings, ignoring this book's own override.
    inherited_direction: Direction,
    prev_key: Option<String>,
    next_key: Option<String>,
    remote: bool,
}

fn thumb(lib: i64, key: &str) -> String {
    comic_url(&ComicPath::Thumb { library_id: lib, key: key.to_string() })
}

fn work_views(s: &AppState, lib: i64, works: &[Work]) -> Result<Vec<WorkView>, String> {
    let db = s.db.lock().unwrap();
    let global = db.global_direction().map_err(err)?;
    let lib_dir = db.libraries().map_err(err)?.into_iter().find(|l| l.id == lib).and_then(|l| l.direction);
    let states = db.book_states(lib).map_err(err)?;
    works
        .iter()
        .map(|w| {
            let folder = db.folder_direction(lib, &w.id).map_err(err)?;
            let (read, total) = work_progress(&w.volumes, |k| states.get(k).cloned());
            Ok(WorkView {
                id: w.id.clone(),
                title: w.title.clone(),
                volume_count: total,
                read_volumes: read,
                cover_url: thumb(lib, &w.volumes[0].key),
                direction: resolve(global, lib_dir, folder, None),
                folder_direction: folder,
            })
        })
        .collect()
}

fn find_volume<'a>(works: &'a [Work], key: &str) -> Option<(&'a Work, usize)> {
    works.iter().find_map(|w| w.volumes.iter().position(|v| v.key == key).map(|i| (w, i)))
}

/// (effective direction, inherited direction without the book override, book state)
fn book_direction(
    s: &AppState,
    lib: i64,
    work_id: &str,
    key: &str,
) -> Result<(Direction, Direction, Option<crate::db::BookState>), String> {
    let db = s.db.lock().unwrap();
    let global = db.global_direction().map_err(err)?;
    let lib_dir = db.libraries().map_err(err)?.into_iter().find(|l| l.id == lib).and_then(|l| l.direction);
    let folder = db.folder_direction(lib, work_id).map_err(err)?;
    let st = db.book_state(lib, key).map_err(err)?;
    let inherited = resolve(global, lib_dir, folder, None);
    Ok((resolve(global, lib_dir, folder, st.as_ref().and_then(|b| b.direction)), inherited, st))
}

#[tauri::command]
pub async fn list_libraries(state: St<'_>) -> Result<Vec<LibraryView>, String> {
    blocking(&state, |s| {
        Ok(s.db
            .lock()
            .unwrap()
            .libraries()
            .map_err(err)?
            .into_iter()
            .map(|l| LibraryView { id: l.id, kind: l.kind, name: l.name, host: l.host, root: l.root, direction: l.direction })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn add_local_library(state: St<'_>, path: String) -> Result<i64, String> {
    blocking(&state, move |s| {
        let name = std::path::Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
        s.db.lock().unwrap().add_library(LibraryKind::Local, &name, None, &path).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn remove_library(state: St<'_>, id: i64) -> Result<(), String> {
    blocking(&state, move |s| {
        s.forget(id);
        s.db.lock().unwrap().remove_library(id).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn list_works(state: St<'_>, library_id: i64) -> Result<Vec<WorkView>, String> {
    blocking(&state, move |s| {
        let works = s.works(library_id)?;
        work_views(s, library_id, &works)
    })
    .await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LibraryUpdated {
    library_id: i64,
    error: Option<String>,
}

/// Rescans in the background; emits `library-updated` when the result differs or the scan fails.
#[tauri::command]
pub async fn refresh_library(app: AppHandle, state: St<'_>, library_id: i64) -> Result<(), String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let before = s.db.lock().unwrap().works_cached(library_id).ok().flatten();
        let payload = match s.scan(library_id) {
            Ok(after) if Some(&after) == before.as_ref() => return,
            Ok(_) => LibraryUpdated { library_id, error: None },
            Err(e) => {
                s.forget(library_id);
                LibraryUpdated { library_id, error: Some(e) }
            }
        };
        let _ = app.emit("library-updated", payload);
    });
    Ok(())
}

#[tauri::command]
pub async fn list_volumes(state: St<'_>, library_id: i64, work_id: String) -> Result<Vec<VolumeView>, String> {
    blocking(&state, move |s| {
        let works = s.works(library_id)?;
        let work = works.iter().find(|w| w.id == work_id).ok_or("작품을 찾을 수 없습니다")?;
        let db = s.db.lock().unwrap();
        let global = db.global_direction().map_err(err)?;
        let lib_dir = db.libraries().map_err(err)?.into_iter().find(|l| l.id == library_id).and_then(|l| l.direction);
        let folder = db.folder_direction(library_id, &work.id).map_err(err)?;
        let states = db.book_states(library_id).map_err(err)?;
        Ok(work
            .volumes
            .iter()
            .map(|v| {
                let st = states.get(&v.key);
                VolumeView {
                    key: v.key.clone(),
                    title: v.title.clone(),
                    page_count: v.page_count,
                    page: st.filter(|b| b.page > 0 || b.page_count > 0).map(|b| b.page),
                    cover_url: thumb(library_id, &v.key),
                    direction: resolve(global, lib_dir, folder, st.and_then(|b| b.direction)),
                }
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn list_recent(state: St<'_>) -> Result<Vec<RecentView>, String> {
    blocking(&state, |s| {
        let recent = s.db.lock().unwrap().recent_books(20).map_err(err)?;
        let mut out = Vec::new();
        for r in recent {
            let Ok(works) = s.works(r.library_id) else { continue };
            let Some((work, i)) = find_volume(&works, &r.key) else { continue };
            let (direction, _, _) = book_direction(s, r.library_id, &work.id, &r.key)?;
            out.push(RecentView {
                library_id: r.library_id,
                key: r.key.clone(),
                title: work.volumes[i].title.clone(),
                work_id: work.id.clone(),
                work_title: work.title.clone(),
                page: r.state.page,
                page_count: work.volumes[i].page_count,
                cover_url: thumb(r.library_id, &r.key),
                direction,
            });
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn open_volume(state: St<'_>, library_id: i64, key: String) -> Result<OpenedVolume, String> {
    blocking(&state, move |s| {
        let works = s.works(library_id)?;
        let (work, i) = find_volume(&works, &key).ok_or("권을 찾을 수 없습니다")?;
        let vol = &work.volumes[i];
        let (direction, inherited_direction, st) = book_direction(s, library_id, &work.id, &key)?;
        let (prev_key, next_key) = neighbor_keys(&works, &key);
        let remote = s
            .db
            .lock()
            .unwrap()
            .libraries()
            .map_err(err)?
            .iter()
            .any(|l| l.id == library_id && l.kind == LibraryKind::Sftp);
        let page = st.as_ref().map_or(0, |b| b.page).min(vol.page_count.saturating_sub(1));
        Ok(OpenedVolume {
            key: key.clone(),
            title: vol.title.clone(),
            work_id: work.id.clone(),
            work_title: work.title.clone(),
            pages: (0..vol.page_count as usize)
                .map(|index| comic_url(&ComicPath::Page { library_id, key: key.clone(), index }))
                .collect(),
            page,
            shift: st.is_some_and(|b| b.shift),
            direction,
            inherited_direction,
            prev_key,
            next_key,
            remote,
        })
    })
    .await
}

#[tauri::command]
pub async fn save_progress(state: St<'_>, library_id: i64, key: String, page: u32, page_count: u32) -> Result<(), String> {
    blocking(&state, move |s| s.db.lock().unwrap().save_page(library_id, &key, page, page_count).map_err(err)).await
}

#[tauri::command]
pub async fn set_shift(state: St<'_>, library_id: i64, key: String, shift: bool) -> Result<(), String> {
    blocking(&state, move |s| s.db.lock().unwrap().set_book_shift(library_id, &key, shift).map_err(err)).await
}

/// scope: "global" | "library" | "folder" (target = work id) | "book" (target = volume key).
#[tauri::command]
pub async fn set_direction(
    state: St<'_>,
    scope: String,
    library_id: Option<i64>,
    target: Option<String>,
    direction: Option<Direction>,
) -> Result<(), String> {
    blocking(&state, move |s| {
        let db = s.db.lock().unwrap();
        let lib = || library_id.ok_or_else(|| "library_id 누락".to_string());
        let tgt = || target.clone().ok_or_else(|| "target 누락".to_string());
        match scope.as_str() {
            "global" => db.set_global_direction(direction.unwrap_or(Direction::Rtl)).map_err(err),
            "library" => db.set_library_direction(lib()?, direction).map_err(err),
            "folder" => db.set_folder_direction(lib()?, &tgt()?, direction).map_err(err),
            "book" => db.set_book_direction(lib()?, &tgt()?, direction).map_err(err),
            _ => Err("알 수 없는 범위".into()),
        }
    })
    .await
}

#[tauri::command]
pub async fn global_direction(state: St<'_>) -> Result<Direction, String> {
    blocking(&state, |s| s.db.lock().unwrap().global_direction().map_err(err)).await
}
