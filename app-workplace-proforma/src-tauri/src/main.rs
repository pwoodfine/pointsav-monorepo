// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Workplace*Proforma — Sovereign Spreadsheet for Institutional Analysis
// Copyright © 2026 PointSav Digital Systems
// Licensed under the European Union Public Licence v1.2 (EUPL-1.2)

// Prevents a console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::api::dialog;

// ─── IPC Commands ────────────────────────────────────────────────────────────
//
// The IPC surface is intentionally minimal: three commands only.
// No shell access, no arbitrary file system traversal, no network commands.
// CSP is set to connect-src: 'none' — zero outbound connections.
//
// Phase 1 MVP: three commands (open_file, save_file, get_app_data_dir).
// Phase 2 adds IronCalc engine commands: evaluate_workbook, parse_formula.

/// Open a native OS file picker and return the contents of the selected
/// .json proforma file as a UTF-8 string.
#[tauri::command]
async fn open_file(_window: tauri::Window) -> Result<Option<String>, String> {
    let file_path = dialog::blocking::FileDialogBuilder::new()
        .set_title("Open Proforma")
        .add_filter("Workplace Proforma Documents", &["json"])
        .add_filter("All Files", &["*"])
        .pick_file();

    match file_path {
        Some(path) => {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read file: {}", e))?;

            // Basic validation: ensure this is parseable JSON before returning.
            // A full schema validation happens in the frontend.
            serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| format!("File is not valid JSON: {}", e))?;

            Ok(Some(content))
        }
        None => Ok(None), // User cancelled
    }
}

/// Open a native OS save picker and write the provided JSON content to disk.
/// Returns the path where the file was saved, or None if the user cancelled.
#[tauri::command]
async fn save_file(content: String, suggested_name: Option<String>) -> Result<Option<String>, String> {
    // Validate that we are being asked to save valid JSON. The frontend is
    // responsible for producing schema-compliant content; this is a safety
    // rail against corrupted state reaching disk.
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("Refusing to save invalid JSON: {}", e))?;

    let mut builder = dialog::blocking::FileDialogBuilder::new()
        .set_title("Save Proforma")
        .add_filter("Workplace Proforma Documents", &["json"]);

    if let Some(name) = suggested_name {
        builder = builder.set_file_name(&name);
    } else {
        builder = builder.set_file_name("proforma.json");
    }

    let save_path = builder.save_file();

    match save_path {
        Some(mut path) => {
            // Ensure the file has a .json extension
            if path.extension().is_none() || path.extension().unwrap() != "json" {
                path.set_extension("json");
            }

            // Security: canonicalise the parent directory to prevent path traversal
            let parent = path
                .parent()
                .ok_or_else(|| "Invalid save path: no parent directory".to_string())?;
            let canonical_parent = std::fs::canonicalize(parent)
                .map_err(|e| format!("Invalid save path: {}", e))?;
            let safe_path = canonical_parent.join(path.file_name().unwrap());

            std::fs::write(&safe_path, content.as_bytes())
                .map_err(|e| format!("Failed to write file: {}", e))?;

            Ok(Some(safe_path.to_string_lossy().to_string()))
        }
        None => Ok(None), // User cancelled
    }
}

/// Return the application data directory path.
/// Used by the templates manager to locate locally-stored template files.
#[tauri::command]
fn get_app_data_dir(app_handle: tauri::AppHandle) -> Result<String, String> {
    app_handle
        .path_resolver()
        .app_data_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not resolve app data directory".to_string())
}

// ─── Application Entry Point ─────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            open_file,
            save_file,
            get_app_data_dir,
        ])
        .setup(|app| {
            // Create the templates directory in app data on first run
            if let Some(app_data_dir) = app.path_resolver().app_data_dir() {
                let templates_dir = app_data_dir.join("templates");
                if !templates_dir.exists() {
                    std::fs::create_dir_all(&templates_dir).ok();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running workplace-proforma");
}
