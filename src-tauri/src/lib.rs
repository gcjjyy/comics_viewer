pub mod archive;
pub mod blockcache;
pub mod commands;
pub mod db;
pub mod direction;
pub mod encoding;
pub mod keys;
pub mod library;
pub mod natsort;
pub mod prompt_bridge;
pub mod protocol;
pub mod source;
pub mod state;
#[cfg(test)]
mod testutil;
pub mod thumbs;

use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("comic", protocol::handle)
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let thumbs = app.path().app_cache_dir()?.join("thumbs");
            std::fs::create_dir_all(&data)?;
            std::fs::create_dir_all(&thumbs)?;
            let db = db::Db::open(&data.join("comics.db"))?;
            let st = Arc::new(state::AppState::new(db, thumbs));
            let bridge = prompt_bridge::TauriPrompter::new(app.handle().clone());
            st.set_remote(state::Remote {
                prompter: Arc::new(prompt_bridge::Bridge(bridge.clone())),
                rt: tauri::async_runtime::handle().inner().clone(),
                home: app.path().home_dir()?,
                local_user: std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default(),
            });
            app.manage(st);
            app.manage(bridge);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_libraries,
            commands::add_local_library,
            commands::remove_library,
            commands::list_works,
            commands::refresh_library,
            commands::list_volumes,
            commands::list_recent,
            commands::open_volume,
            commands::save_progress,
            commands::set_shift,
            commands::set_direction,
            commands::global_direction,
            commands::list_ssh_hosts,
            commands::sftp_list_dir,
            commands::add_sftp_library,
            commands::reconnect_library,
            commands::ssh_prompt_reply,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
