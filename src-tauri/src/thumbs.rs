use crate::keys::split_key;
use crate::state::{err, AppState};
use image::codecs::jpeg::JpegEncoder;
use sha1::{Digest, Sha1};
use std::path::PathBuf;

pub const THUMB_WIDTH: u32 = 300;

/// Returns the cached cover thumbnail of a volume, creating it from the first page if needed.
pub fn ensure(state: &AppState, lib: i64, key: &str) -> Result<PathBuf, String> {
    let (zip_path, _) = split_key(key);
    let (size, mtime) = state
        .db
        .lock()
        .unwrap()
        .zip_cached_latest(lib, zip_path)
        .map_err(err)?
        .map_or((0, 0), |(s, m, _)| (s, m));
    let name = hex::encode(Sha1::digest(format!("{lib}|{key}|{size}|{mtime}").as_bytes()));
    let path = state.thumb_dir.join(format!("{name}.jpg"));
    if path.exists() {
        return Ok(path);
    }
    state.thumb_gate.run(|| {
        if path.exists() {
            return Ok(path.clone());
        }
        let (bytes, _) = state.read_page(lib, key, 0)?;
        let img = image::load_from_memory(&bytes).map_err(err)?;
        let h = (img.height() as u64 * THUMB_WIDTH as u64 / img.width().max(1) as u64).max(1) as u32;
        let rgb = img.resize_exact(THUMB_WIDTH, h, image::imageops::FilterType::Triangle).to_rgb8();
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 80).encode_image(&rgb).map_err(err)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &out).map_err(err)?;
        std::fs::rename(&tmp, &path).map_err(err)?;
        Ok(path.clone())
    })
}
