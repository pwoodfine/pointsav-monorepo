// Workplace*PDF — frontend controller
// Copyright © 2026 PointSav Digital Systems
// Licensed under the Apache License, Version 2.0
//
// Talks to the Rust side over Tauri IPC (withGlobalTauri):
//   open_pdf()                                -> DocumentInfo | null (cancelled)
//   render_page({ pageIndex, targetWidth })   -> "data:image/png;base64,…"
//   get_page_count()                          -> number

"use strict";

// Tauri v2: invoke moved to __TAURI__.core. The v1 window `appWindow.print()`
// was removed (v2 Window has no print method) — printing now uses DOM window.print().
const invoke = window.__TAURI__.core.invoke;

// Screen rendering width at 100% zoom; print rendering width (~150 DPI letter).
const BASE_WIDTH = 1100;
const PRINT_WIDTH = 1600;
const ZOOM_STEPS = [0.5, 0.67, 0.8, 1.0, 1.25, 1.5, 2.0, 3.0];

const state = {
  fileName: null,
  pageCount: 0,
  current: 0, // 0-based
  zoomIndex: ZOOM_STEPS.indexOf(1.0),
  busy: false,
};

// Increments on every page render request so stale responses are discarded.
let renderToken = 0;

const el = {
  openBtn: document.getElementById("open-btn"),
  emptyOpenBtn: document.getElementById("empty-open-btn"),
  prevBtn: document.getElementById("prev-btn"),
  nextBtn: document.getElementById("next-btn"),
  zoomInBtn: document.getElementById("zoom-in-btn"),
  zoomOutBtn: document.getElementById("zoom-out-btn"),
  printBtn: document.getElementById("print-btn"),
  pageLabel: document.getElementById("page-label"),
  zoomLabel: document.getElementById("zoom-label"),
  fileLabel: document.getElementById("file-label"),
  pageImage: document.getElementById("page-image"),
  emptyState: document.getElementById("empty-state"),
  statusBar: document.getElementById("status-bar"),
  errorBanner: document.getElementById("error-banner"),
  errorMessage: document.getElementById("error-message"),
  errorDismiss: document.getElementById("error-dismiss"),
  printArea: document.getElementById("print-area"),
};

function setStatus(text) {
  el.statusBar.textContent = text || "";
}

function showError(message) {
  el.errorMessage.textContent = String(message);
  el.errorBanner.classList.add("visible");
  setStatus("");
}

function clearError() {
  el.errorBanner.classList.remove("visible");
  el.errorMessage.textContent = "";
}

function zoom() {
  return ZOOM_STEPS[state.zoomIndex];
}

function hasDocument() {
  return state.pageCount > 0;
}

function updateToolbar() {
  const open = hasDocument();
  el.prevBtn.disabled = !open || state.current <= 0 || state.busy;
  el.nextBtn.disabled = !open || state.current >= state.pageCount - 1 || state.busy;
  el.zoomInBtn.disabled = !open || state.zoomIndex >= ZOOM_STEPS.length - 1;
  el.zoomOutBtn.disabled = !open || state.zoomIndex <= 0;
  el.printBtn.disabled = !open || state.busy;
  el.pageLabel.textContent = open
    ? "Page " + (state.current + 1) + " / " + state.pageCount
    : "No document";
  el.zoomLabel.textContent = Math.round(zoom() * 100) + "%";
  el.fileLabel.textContent = state.fileName || "";
  el.fileLabel.title = state.fileName || "";
}

async function showPage(index) {
  if (!hasDocument()) return;
  const clamped = Math.min(Math.max(index, 0), state.pageCount - 1);
  state.current = clamped;
  updateToolbar();

  const token = ++renderToken;
  setStatus("Rendering page " + (clamped + 1) + "…");
  try {
    const uri = await invoke("render_page", {
      pageIndex: clamped,
      targetWidth: Math.round(BASE_WIDTH * zoom()),
    });
    if (token !== renderToken) return; // superseded by a newer request
    el.pageImage.src = uri;
    el.pageImage.alt = "Page " + (clamped + 1) + " of " + state.pageCount + (state.fileName ? " — " + state.fileName : "");
    el.pageImage.hidden = false;
    el.emptyState.hidden = true;
    setStatus("");
  } catch (err) {
    if (token === renderToken) showError(err);
  } finally {
    if (token === renderToken) updateToolbar();
  }
}

async function openPdf() {
  if (state.busy) return;
  clearError();
  state.busy = true;
  updateToolbar();
  setStatus("Opening…");
  try {
    const info = await invoke("open_pdf");
    if (info === null) {
      // User cancelled the picker — keep whatever was open before.
      setStatus("");
      return;
    }
    state.fileName = info.file_name;
    state.pageCount = info.page_count;
    state.current = 0;
    document.title = info.file_name + " — Workplace PDF";
    if (state.pageCount === 0) {
      showError("The document opened but contains no pages.");
      return;
    }
    await showPage(0);
  } catch (err) {
    showError(err);
  } finally {
    state.busy = false;
    updateToolbar();
  }
}

async function printDocument() {
  if (!hasDocument() || state.busy) return;
  clearError();
  state.busy = true;
  updateToolbar();
  el.printArea.innerHTML = "";
  try {
    for (let i = 0; i < state.pageCount; i++) {
      setStatus("Preparing print… page " + (i + 1) + " of " + state.pageCount);
      const uri = await invoke("render_page", {
        pageIndex: i,
        targetWidth: PRINT_WIDTH,
      });
      const img = document.createElement("img");
      img.src = uri;
      el.printArea.appendChild(img);
    }
    setStatus("");
    // v2: Tauri's Window API no longer exposes print() (v1's window-print
    // allowlist feature was removed), so use the DOM print API.
    // UNVERIFIED on macOS: window.print() was historically a no-op in WKWebView;
    // if that regresses, the fix is the Tauri print plugin (not yet in deps).
    window.print();
  } catch (err) {
    showError(err);
  } finally {
    state.busy = false;
    updateToolbar();
  }
}

window.addEventListener("afterprint", function () {
  el.printArea.innerHTML = "";
});

function changeZoom(delta) {
  if (!hasDocument()) return;
  const next = Math.min(Math.max(state.zoomIndex + delta, 0), ZOOM_STEPS.length - 1);
  if (next === state.zoomIndex) return;
  state.zoomIndex = next;
  updateToolbar();
  showPage(state.current);
}

el.openBtn.addEventListener("click", openPdf);
el.emptyOpenBtn.addEventListener("click", openPdf);
el.prevBtn.addEventListener("click", function () { showPage(state.current - 1); });
el.nextBtn.addEventListener("click", function () { showPage(state.current + 1); });
el.zoomInBtn.addEventListener("click", function () { changeZoom(1); });
el.zoomOutBtn.addEventListener("click", function () { changeZoom(-1); });
el.printBtn.addEventListener("click", printDocument);
el.errorDismiss.addEventListener("click", clearError);

document.addEventListener("keydown", function (event) {
  const meta = event.metaKey || event.ctrlKey;
  if (meta && event.key.toLowerCase() === "o") {
    event.preventDefault();
    openPdf();
    return;
  }
  if (meta && event.key.toLowerCase() === "p") {
    event.preventDefault();
    printDocument();
    return;
  }
  if (!hasDocument() || state.busy) return;
  switch (event.key) {
    case "ArrowLeft":
    case "PageUp":
      event.preventDefault();
      showPage(state.current - 1);
      break;
    case "ArrowRight":
    case "PageDown":
      event.preventDefault();
      showPage(state.current + 1);
      break;
    case "Home":
      event.preventDefault();
      showPage(0);
      break;
    case "End":
      event.preventDefault();
      showPage(state.pageCount - 1);
      break;
    case "+":
    case "=":
      changeZoom(1);
      break;
    case "-":
      changeZoom(-1);
      break;
  }
});

updateToolbar();
