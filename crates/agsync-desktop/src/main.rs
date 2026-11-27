#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use agsync_core::report::{self, StatusView};

/// `agsync status` as data for the window.
#[tauri::command]
async fn status() -> Result<StatusView, String> {
    tauri::async_runtime::spawn_blocking(report::status_view)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![status])
        .run(tauri::generate_context!())
        .expect("run the agsync desktop app");
}
