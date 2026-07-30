// Workplace*Presentation — Sovereign Presentation Tool
// Copyright © 2026 PointSav Digital Systems
// Licensed under the European Union Public Licence v1.2 (EUPL-1.2)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        // v2: the frontend calls these plugins' JS APIs directly (t.dialog.*, t.fs.*),
        // so both must be registered here and permitted in capabilities/default.json.
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running workplace-presentation");
}
