pub mod archive;
pub mod blockcache;
pub mod encoding;
pub mod natsort;
pub mod source;
#[cfg(test)]
mod testutil;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
