// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Workplace*Workbench — privategit development workbench shell
// Copyright © 2026 PointSav Digital Systems
// Licensed under the Apache License, Version 2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;
use tauri::Manager;

const DEFAULT_PORT: u16 = 3000;
const CONFIG_FILENAME: &str = "workbench-config.json";

fn load_port(app_data_dir: &PathBuf) -> u16 {
    let config_path = app_data_dir.join(CONFIG_FILENAME);
    let Ok(content) = fs::read_to_string(&config_path) else {
        return DEFAULT_PORT;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return DEFAULT_PORT;
    };
    json["port"].as_u64().unwrap_or(DEFAULT_PORT as u64) as u16
}

#[tauri::command]
fn get_workbench_url(app_handle: tauri::AppHandle) -> String {
    let port = app_handle
        .path_resolver()
        .app_data_dir()
        .map(|dir| load_port(&dir))
        .unwrap_or(DEFAULT_PORT);
    format!("http://127.0.0.1:{}", port)
}

#[tauri::command]
fn set_workbench_port(app_handle: tauri::AppHandle, port: u16) -> Result<(), String> {
    let dir = app_handle
        .path_resolver()
        .app_data_dir()
        .ok_or("Cannot resolve app data directory")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let config = serde_json::json!({ "port": port });
    fs::write(dir.join(CONFIG_FILENAME), config.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_workbench_url, set_workbench_port])
        .setup(|app| {
            if let Some(dir) = app.path_resolver().app_data_dir() {
                fs::create_dir_all(&dir).ok();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running workplace-workbench");
}
