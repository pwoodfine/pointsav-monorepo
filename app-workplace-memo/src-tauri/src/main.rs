// Workplace*Memo — Sovereign Document Editor
// Copyright © 2026 PointSav Digital Systems
// Licensed under the European Union Public Licence v1.2 (EUPL-1.2)

// Prevents a console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

// ─── IPC Commands ────────────────────────────────────────────────────────────
//
// The IPC surface is intentionally minimal: four commands only.
// No shell access, no arbitrary file system traversal, no network commands.
// CSP is set to connect-src: 'none' — zero outbound connections.

/// Open a native OS file picker and return the contents of the selected
/// .html document file as a UTF-8 string.
#[tauri::command]
async fn open_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    // v2: dialog moved to tauri-plugin-dialog (DialogExt). The async command runs
    // off the main thread, so blocking_pick_file() is safe here (the plugin
    // re-dispatches UI work to the main thread internally).
    let file_path = app
        .dialog()
        .file()
        .set_title("Open Document")
        .add_filter("Workplace Memo Documents", &["html"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    match file_path {
        Some(fp) => {
            let path = fp.into_path().map_err(|e| e.to_string())?;
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            Ok(Some(content))
        }
        None => Ok(None), // User cancelled
    }
}

/// Open a native OS save picker and write the provided HTML content to disk.
/// Returns the path where the file was saved, or None if the user cancelled.
#[tauri::command]
async fn save_file(app: tauri::AppHandle, content: String, suggested_name: Option<String>) -> Result<Option<String>, String> {
    let mut builder = app
        .dialog()
        .file()
        .set_title("Save Document")
        .add_filter("Workplace Memo Documents", &["html"]);

    if let Some(name) = suggested_name {
        builder = builder.set_file_name(name);
    } else {
        builder = builder.set_file_name("document.html");
    }

    let save_path = builder.blocking_save_file();

    match save_path {
        Some(fp) => {
            let mut path = fp.into_path().map_err(|e| e.to_string())?;
            // Ensure the file has a .html extension
            if path.extension().is_none() || path.extension().unwrap() != "html" {
                path.set_extension("html");
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
/// Used by the font manager to locate locally-downloaded fonts.
#[tauri::command]
fn get_app_data_dir(app_handle: tauri::AppHandle) -> Result<String, String> {
    app_handle
        .path()
        .app_data_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|_| "Could not resolve app data directory".to_string())
}

/// Read a font file from the application data directory and return it as a
/// base64-encoded string suitable for embedding in a @font-face data URI.
///
/// The path argument must be a filename only (no directory components).
/// This command will refuse to read files outside the app data fonts directory.
#[tauri::command]
async fn read_font_file(
    app_handle: tauri::AppHandle,
    filename: String,
) -> Result<String, String> {
    // Validate: filename must not contain path separators
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("Invalid font filename: path traversal not permitted".to_string());
    }

    // Only allow .woff2 and .woff font files
    let allowed_extensions = ["woff2", "woff"];
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !allowed_extensions.contains(&ext) {
        return Err(format!("Invalid font file type: .{} not permitted", ext));
    }

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| "Could not resolve app data directory".to_string())?;

    let fonts_dir = app_data_dir.join("fonts");
    let font_path = fonts_dir.join(&filename);

    // Security: ensure the resolved path is within the fonts directory
    let canonical_path = std::fs::canonicalize(&font_path)
        .map_err(|_| format!("Font file not found: {}", filename))?;
    let canonical_fonts_dir = std::fs::canonicalize(&fonts_dir)
        .map_err(|_| "Fonts directory does not exist".to_string())?;

    if !canonical_path.starts_with(&canonical_fonts_dir) {
        return Err("Access denied: path traversal detected".to_string());
    }

    let bytes = std::fs::read(&canonical_path)
        .map_err(|e| format!("Failed to read font file: {}", e))?;

    Ok(base64::encode(&bytes))
}

// ─── Application Entry Point ─────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_file,
            save_file,
            get_app_data_dir,
            read_font_file,
        ])
        .setup(|app| {
            // Create the fonts directory in app data on first run
            // v2: path_resolver() -> path(), and app_data_dir() returns Result.
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                let fonts_dir = app_data_dir.join("fonts");
                if !fonts_dir.exists() {
                    std::fs::create_dir_all(&fonts_dir).ok();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running workplace-memo");
}
