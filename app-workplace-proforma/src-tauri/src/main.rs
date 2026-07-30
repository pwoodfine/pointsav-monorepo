// Workplace*Proforma — Sovereign Spreadsheet for Institutional Analysis
// Copyright © 2026 PointSav Digital Systems
// Licensed under the European Union Public Licence v1.2 (EUPL-1.2)

// Prevents a console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

// ─── Local Service Endpoint Configuration (declaration only) ────────────────
//
// NEXT.md (Wave 2 pending) asks to "wire endpoint configuration" for the
// workspace's local-only developer services: `local-proofreader` (:9097)
// and Doorman/`service-slm` (:9092). This struct declares the configurable
// defaults and makes them available as Tauri managed state for a future
// Phase 2+ command to read.
//
// It intentionally does NOT open any connection: no HTTP client dependency
// is added to Cargo.toml, and no code path below calls out to either URL.
// Actually dialing either endpoint would require reconciling the "Never add
// a network call" / `connect-src 'none'` hard rule in this crate's
// CLAUDE.md first — that reconciliation is out of scope here and is
// flagged back rather than decided unilaterally.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct ServiceEndpoints {
    proofreader_url: String,
    doorman_url: String,
}

impl Default for ServiceEndpoints {
    fn default() -> Self {
        Self {
            // Workspace-standard localhost ports (per AGENT.md / infrastructure/).
            proofreader_url: "http://127.0.0.1:9097".to_string(),
            doorman_url: "http://127.0.0.1:9092".to_string(),
        }
    }
}

// ─── IPC Commands ────────────────────────────────────────────────────────────
//
// The IPC surface is intentionally minimal: three commands only.
// No shell access, no arbitrary file system traversal, no network commands.
// CSP is set to connect-src: 'none' — zero outbound connections.
//
// Phase 1 MVP: three commands (open_file, save_file, get_app_data_dir).
// Phase 2 adds IronCalc engine commands: evaluate_workbook, parse_formula.
// ServiceEndpoints (above) is deliberately NOT a fourth command — exposing
// it over IPC would expand the frozen 3-command (Phase 1) / 6-command
// (Phase 2 ceiling) surface for no functional gain yet.

/// Open a native OS file picker and return the contents of the selected
/// .json proforma file as a UTF-8 string.
#[tauri::command]
async fn open_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    // v2: dialog moved to tauri-plugin-dialog (DialogExt). The async command runs
    // off the main thread, so blocking_pick_file() is safe here.
    let file_path = app
        .dialog()
        .file()
        .set_title("Open Proforma")
        .add_filter("Workplace Proforma Documents", &["json"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    match file_path {
        Some(fp) => {
            let path = fp.into_path().map_err(|e| e.to_string())?;
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
async fn save_file(app: tauri::AppHandle, content: String, suggested_name: Option<String>) -> Result<Option<String>, String> {
    // Validate that we are being asked to save valid JSON. The frontend is
    // responsible for producing schema-compliant content; this is a safety
    // rail against corrupted state reaching disk.
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("Refusing to save invalid JSON: {}", e))?;

    let mut builder = app
        .dialog()
        .file()
        .set_title("Save Proforma")
        .add_filter("Workplace Proforma Documents", &["json"]);

    if let Some(name) = suggested_name {
        builder = builder.set_file_name(name);
    } else {
        builder = builder.set_file_name("proforma.json");
    }

    let save_path = builder.blocking_save_file();

    match save_path {
        Some(fp) => {
            let mut path = fp.into_path().map_err(|e| e.to_string())?;
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
        .path()
        .app_data_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|_| "Could not resolve app data directory".to_string())
}

// ─── Application Entry Point ─────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ServiceEndpoints::default())
        .invoke_handler(tauri::generate_handler![
            open_file,
            save_file,
            get_app_data_dir,
        ])
        .setup(|app| {
            // Create the templates directory in app data on first run
            // v2: path_resolver() -> path(), and app_data_dir() returns Result.
            if let Ok(app_data_dir) = app.path().app_data_dir() {
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
