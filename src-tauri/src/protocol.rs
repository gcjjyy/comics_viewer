use crate::keys::{parse_comic_path, ComicPath};
use crate::state::AppState;
use crate::thumbs;
use std::sync::Arc;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let state = ctx.app_handle().state::<Arc<AppState>>().inner().clone();
    let path = request.uri().path().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let result = match parse_comic_path(&path) {
            Some(ComicPath::Page { library_id, key, index }) => state.read_page(library_id, &key, index),
            Some(ComicPath::Thumb { library_id, key }) => thumbs::ensure(&state, library_id, &key)
                .and_then(|p| std::fs::read(p).map_err(|e| e.to_string()))
                .map(|b| (b, "image/jpeg")),
            None => Err("bad path".into()),
        };
        let response = match result {
            Ok((body, mime)) => Response::builder()
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(body),
            Err(e) => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(e.into_bytes()),
        };
        responder.respond(response.unwrap());
    });
}
