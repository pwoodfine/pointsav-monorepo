// Workplace*Workbench — privategit development workbench shell
// Copyright © 2026 PointSav Digital Systems
// Licensed under the Apache License, Version 2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use tauri::Manager;

const DEFAULT_PORT: u16 = 3000;
const CONFIG_FILENAME: &str = "workbench-config.json";

/// On-disk shape of `workbench-config.json`. Read/written via the shared
/// `workplace-shell-chrome` crate — see that crate's README for why this
/// moved out of a hand-rolled `fs`/`serde_json` pair (2026-07-14 retrofit,
/// closing the duplication documented in
/// `DESIGN-RESEARCH-workplace-shell-chrome.draft.md`).
#[derive(serde::Serialize, serde::Deserialize)]
struct WorkbenchConfig {
    port: u16,
}

fn load_port(app_data_dir: &PathBuf) -> u16 {
    workplace_shell_chrome::load_config::<WorkbenchConfig>(app_data_dir, CONFIG_FILENAME)
        .map(|cfg| cfg.port)
        .unwrap_or(DEFAULT_PORT)
}

#[tauri::command]
fn get_workbench_url(app_handle: tauri::AppHandle) -> String {
    let port = app_handle
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| load_port(&dir))
        .unwrap_or(DEFAULT_PORT);
    format!("http://127.0.0.1:{}", port)
}

#[tauri::command]
fn set_workbench_port(app_handle: tauri::AppHandle, port: u16) -> Result<(), String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .ok()
        .ok_or("Cannot resolve app data directory")?;
    workplace_shell_chrome::save_config(&dir, CONFIG_FILENAME, &WorkbenchConfig { port })
}

/// True once `workbench-config.json` exists in the app data dir — used by the
/// frontend to decide whether to show the first-run port-configuration dialog.
#[tauri::command]
fn has_workbench_config(app_handle: tauri::AppHandle) -> bool {
    app_handle
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| workplace_shell_chrome::has_config(&dir, CONFIG_FILENAME))
        .unwrap_or(false)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_workbench_url,
            set_workbench_port,
            has_workbench_config
        ])
        .setup(|app| {
            // v2: path_resolver() -> path(), and app_data_dir() returns Result, not Option.
            if let Ok(dir) = app.path().app_data_dir() {
                workplace_shell_chrome::ensure_app_data_dir(&dir).ok();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running workplace-workbench");
}
