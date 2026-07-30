// Workplace*PDF — sovereign desktop PDF viewer and print tool
// Copyright © 2026 PointSav Digital Systems
// Licensed under the Apache License, Version 2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use base64::Engine as _;
use pdfium_render::prelude::*;
use tauri_plugin_dialog::DialogExt;

/// Rendered-page width bounds (pixels). The frontend requests widths inside
/// this range for zoom and print rendering.
const MIN_RENDER_WIDTH: i32 = 200;
const MAX_RENDER_WIDTH: i32 = 4000;
const DEFAULT_RENDER_WIDTH: i32 = 1100;

#[derive(serde::Serialize)]
struct DocumentInfo {
    file_name: String,
    path: String,
    page_count: u32,
}

/// Requests handled by the dedicated PDF worker thread.
enum PdfRequest {
    /// Load the PDF at `path`, replacing any currently open document.
    Open {
        path: PathBuf,
        reply: Sender<Result<DocumentInfo, String>>,
    },
    /// Render page `page_index` (0-based) at `target_width` pixels wide;
    /// reply with a `data:image/png;base64,…` URI.
    Render {
        page_index: u32,
        target_width: i32,
        reply: Sender<Result<String, String>>,
    },
    /// Page count of the currently open document.
    PageCount { reply: Sender<Result<u32, String>> },
}

/// Tauri-managed handle to the PDF worker thread.
///
/// pdfium-render 0.9 binds PDFium through a process-wide `OnceCell` (binding a
/// second time is an error, and `Pdfium::new` asserts), and its `Pdfium` /
/// `PdfDocument` types are `!Send`, so they cannot live in Tauri managed state
/// or cross `spawn_blocking` threads. A single dedicated worker thread owns
/// the one `Pdfium` instance and the currently open document; IPC commands
/// talk to it over mpsc channels. Requests are served strictly in order, which
/// also serialises access to the (non-thread-safe) PDFium C library.
struct PdfWorkerHandle {
    tx: Mutex<Sender<PdfRequest>>,
}

impl PdfWorkerHandle {
    fn sender(&self) -> Result<Sender<PdfRequest>, String> {
        Ok(self
            .tx
            .lock()
            .map_err(|_| "PDF worker handle poisoned".to_string())?
            .clone())
    }
}

/// Send one request to the worker and block until it replies.
/// Call only from a blocking-safe thread (`tauri::async_runtime::spawn_blocking`).
fn send_request<T>(
    tx: Sender<PdfRequest>,
    make: impl FnOnce(Sender<Result<T, String>>) -> PdfRequest,
) -> Result<T, String> {
    let (reply_tx, reply_rx) = channel();
    tx.send(make(reply_tx))
        .map_err(|_| "PDF worker is not running".to_string())?;
    reply_rx
        .recv()
        .map_err(|_| "PDF worker dropped the request".to_string())?
}

/// Locate and bind the PDFium dynamic library. Called at most once per process.
///
/// Search order:
/// 1. `$PDFIUM_DYNAMIC_LIB_PATH` (directory containing the library)
/// 2. the executable's own directory
/// 3. `../Frameworks` relative to the executable (macOS .app bundle layout)
/// 4. `./pdfium` relative to the CWD (dev layout: `src-tauri/pdfium/`)
/// 5. the system library search path
fn init_pdfium() -> Result<Pdfium, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("PDFIUM_DYNAMIC_LIB_PATH") {
        candidates.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
            candidates.push(dir.join("../Frameworks"));
        }
    }
    candidates.push(PathBuf::from("./pdfium"));

    for dir in &candidates {
        let lib = Pdfium::pdfium_platform_library_name_at_path(dir);
        if lib.exists() {
            return Pdfium::bind_to_library(&lib).map(Pdfium::new).map_err(|e| {
                format!(
                    "Found a PDFium library at {} but could not bind to it: {:?}",
                    lib.display(),
                    e
                )
            });
        }
    }

    Pdfium::bind_to_system_library().map(Pdfium::new).map_err(|e| {
        format!(
            "No PDFium library found. Searched $PDFIUM_DYNAMIC_LIB_PATH, the executable \
             directory, ../Frameworks, ./pdfium/, and the system library path. Download a \
             prebuilt library from https://github.com/bblanchon/pdfium-binaries and place \
             it in one of those locations (see src-tauri/pdfium/README.md). Error: {:?}",
            e
        )
    })
}

fn handle_open(
    pdfium: &mut Option<&'static Pdfium>,
    document: &mut Option<PdfDocument<'static>>,
    path: PathBuf,
) -> Result<DocumentInfo, String> {
    if pdfium.is_none() {
        // Leaked to 'static: the PDFium binding is process-wide and irrevocable
        // in pdfium-render 0.9, so the engine must live for the process lifetime
        // anyway. Bound lazily on first open so a missing library surfaces as a
        // normal error message instead of a startup crash.
        *pdfium = Some(Box::leak(Box::new(init_pdfium()?)));
    }
    let engine = pdfium.expect("pdfium initialised above");

    // Drop (and thereby close) any current document before opening the new one.
    *document = None;

    let doc = engine.load_pdf_from_file(&path, None).map_err(|e| {
        let detail = format!("{:?}", e);
        let hint = if detail.contains("Password") {
            " — the file appears to be password-protected, which is not yet supported"
        } else {
            ""
        };
        format!("Could not open {}: {}{}", path.display(), detail, hint)
    })?;

    let page_count = doc.pages().len().max(0) as u32;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let path_display = path.display().to_string();

    *document = Some(doc);

    Ok(DocumentInfo {
        file_name,
        path: path_display,
        page_count,
    })
}

fn handle_render(
    document: Option<&PdfDocument<'static>>,
    page_index: u32,
    target_width: i32,
) -> Result<String, String> {
    let doc = document.ok_or_else(|| "No PDF is open".to_string())?;

    let index = PdfPageIndex::try_from(page_index)
        .map_err(|_| format!("Page index {} out of range", page_index))?;
    let page = doc
        .pages()
        .get(index)
        .map_err(|e| format!("Could not load page {}: {:?}", page_index + 1, e))?;

    let width = target_width.clamp(MIN_RENDER_WIDTH, MAX_RENDER_WIDTH);
    let bitmap = page
        .render_with_config(&PdfRenderConfig::new().set_target_width(width))
        .map_err(|e| format!("Could not render page {}: {:?}", page_index + 1, e))?;
    let image = bitmap
        .as_image()
        .map_err(|e| format!("Could not read page {} bitmap: {:?}", page_index + 1, e))?;

    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Could not encode page {} as PNG: {}", page_index + 1, e))?;

    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ))
}

/// Worker thread main loop. Owns the process's single `Pdfium` instance and
/// the currently open document. Exits when the last request sender is dropped.
fn worker_loop(rx: Receiver<PdfRequest>) {
    let mut pdfium: Option<&'static Pdfium> = None;
    let mut document: Option<PdfDocument<'static>> = None;

    for request in rx {
        match request {
            PdfRequest::Open { path, reply } => {
                let result = handle_open(&mut pdfium, &mut document, path);
                let _ = reply.send(result);
            }
            PdfRequest::Render {
                page_index,
                target_width,
                reply,
            } => {
                let _ = reply.send(handle_render(document.as_ref(), page_index, target_width));
            }
            PdfRequest::PageCount { reply } => {
                let result = document
                    .as_ref()
                    .map(|doc| doc.pages().len().max(0) as u32)
                    .ok_or_else(|| "No PDF is open".to_string());
                let _ = reply.send(result);
            }
        }
    }
}

/// Show the OS file picker, then load the selected PDF into the worker.
/// Returns `None` (not an error) if the user cancels the dialog.
#[tauri::command]
async fn open_pdf(
    app: tauri::AppHandle,
    state: tauri::State<'_, PdfWorkerHandle>,
) -> Result<Option<DocumentInfo>, String> {
    // v2: the dialog API moved to tauri-plugin-dialog (DialogExt on AppHandle).
    // blocking_pick_file() must not run on the main thread; spawn_blocking gives
    // it a safe thread and the plugin re-dispatches UI work to the main thread.
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Open PDF")
            .add_filter("PDF documents", &["pdf"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| format!("File dialog task failed: {}", e))?;

    let Some(file_path) = picked else {
        return Ok(None);
    };
    // v2 returns a FilePath enum (native path or content URI); resolve to a PathBuf.
    let path = file_path.into_path().map_err(|e| e.to_string())?;

    let tx = state.sender()?;
    tauri::async_runtime::spawn_blocking(move || {
        send_request(tx, |reply| PdfRequest::Open { path, reply })
    })
    .await
    .map_err(|e| format!("PDF worker task failed: {}", e))?
    .map(Some)
}

/// Render one page (0-based index) to a PNG data URI at the given pixel width.
#[tauri::command]
async fn render_page(
    page_index: u32,
    target_width: Option<i32>,
    state: tauri::State<'_, PdfWorkerHandle>,
) -> Result<String, String> {
    let tx = state.sender()?;
    let width = target_width.unwrap_or(DEFAULT_RENDER_WIDTH);
    tauri::async_runtime::spawn_blocking(move || {
        send_request(tx, |reply| PdfRequest::Render {
            page_index,
            target_width: width,
            reply,
        })
    })
    .await
    .map_err(|e| format!("PDF worker task failed: {}", e))?
}

/// Page count of the currently open document.
#[tauri::command]
async fn get_page_count(state: tauri::State<'_, PdfWorkerHandle>) -> Result<u32, String> {
    let tx = state.sender()?;
    tauri::async_runtime::spawn_blocking(move || {
        send_request(tx, |reply| PdfRequest::PageCount { reply })
    })
    .await
    .map_err(|e| format!("PDF worker task failed: {}", e))?
}

fn main() {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("pdf-worker".into())
        .spawn(move || worker_loop(rx))
        .expect("failed to spawn PDF worker thread");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(PdfWorkerHandle { tx: Mutex::new(tx) })
        .invoke_handler(tauri::generate_handler![
            open_pdf,
            render_page,
            get_page_count
        ])
        .run(tauri::generate_context!())
        .expect("error while running workplace-pdf");
}
