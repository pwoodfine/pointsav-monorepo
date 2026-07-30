/**
 * Workplace*Memo — editor.js
 * Main application controller: document state, file operations, menu wiring,
 * word count, save state indicator.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * This file runs last in the script loading order (see index.html).
 * All other modules (toolbar, templates, fonts, export, pagination) are
 * expected to be initialised before this runs.
 */

'use strict';

/* ─── Document State ─────────────────────────────────────────────────────── */

const State = {
  title:        'Untitled',
  savedPath:    null,       // last path we saved to (null = never saved)
  isDirty:      false,      // unsaved changes?
  pageSize:     'A4',       // 'A4' | 'Letter'
  marginsMm:    25,         // uniform margin in mm
  templateKey:  'corporate',
};

/* ─── DOM refs ───────────────────────────────────────────────────────────── */

const canvas       = document.getElementById('document-canvas');
const titleDisplay = document.getElementById('doc-title-display');
const saveStateEl  = document.getElementById('save-state');
const statusPage   = document.getElementById('status-page');
const statusWords  = document.getElementById('status-words');
const statusTemplate = document.getElementById('status-template');
const statusPlatform = document.getElementById('status-platform');

/* ─── Tauri IPC bridge ───────────────────────────────────────────────────── */

/**
 * Returns true if we are running inside a Tauri WebView.
 * Falls back gracefully to browser-only behaviour for development in a
 * standard browser (e.g. when testing print output via Firefox).
 */
function isTauri() {
  return typeof window.__TAURI__ !== 'undefined';
}

async function tauriInvoke(cmd, args = {}) {
  if (!isTauri()) {
    console.warn(`[bridge] Tauri not available — skipping command: ${cmd}`);
    return null;
  }
  return window.__TAURI__.core.invoke(cmd, args); // v2: invoke moved to __TAURI__.core
}

/* ─── File operations ────────────────────────────────────────────────────── */

async function newDocument() {
  if (State.isDirty) {
    if (!confirm('You have unsaved changes. Create a new document anyway?')) return;
  }
  canvas.innerHTML = '<h1>Untitled Document</h1><p>Begin typing your document here.</p>';
  State.title = 'Untitled';
  State.savedPath = null;
  markClean();
  updateTitleDisplay();
  updateWordCount();
  if (window.WorkplacePagination) WorkplacePagination.refresh();
}

async function openDocument() {
  if (State.isDirty) {
    if (!confirm('You have unsaved changes. Open a different document anyway?')) return;
  }

  const htmlContent = await tauriInvoke('open_file');
  if (!htmlContent) return; // user cancelled or error

  // Extract the body content from the opened HTML document
  const parser = new DOMParser();
  const doc = parser.parseFromString(htmlContent, 'text/html');

  // The exported file wraps content in a <body>; extract just the inner HTML
  const body = doc.body;
  if (body) {
    canvas.innerHTML = body.innerHTML;
  } else {
    canvas.innerHTML = htmlContent; // fallback: use raw content
  }

  // Try to extract title from <h1> or <title>
  const h1 = canvas.querySelector('h1');
  if (h1) {
    State.title = h1.textContent.trim() || 'Untitled';
  } else {
    State.title = doc.title || 'Untitled';
  }

  State.savedPath = null; // We don't track the opened path for Save (only Save As)
  markClean();
  updateTitleDisplay();
  updateWordCount();
  if (window.WorkplacePagination) WorkplacePagination.refresh();
}

async function saveDocument() {
  // If we have a saved path, overwrite it; otherwise prompt Save As
  if (State.savedPath) {
    await writeToPath(State.savedPath);
  } else {
    await saveDocumentAs();
  }
}

async function saveDocumentAs() {
  const html = window.WorkplaceExport
    ? WorkplaceExport.assembleHTML()
    : canvas.innerHTML;

  const suggestedName = State.title.replace(/[^a-zA-Z0-9\s\-_]/g, '') + '.html';
  const savedPath = await tauriInvoke('save_file', { content: html, suggestedName });

  if (savedPath) {
    State.savedPath = savedPath;
    markClean();
  }
}

async function writeToPath(path) {
  const html = window.WorkplaceExport
    ? WorkplaceExport.assembleHTML()
    : canvas.innerHTML;

  // Use save_file with a fixed path by passing it as the suggested_name.
  // In Tauri v1, there is no "write without dialogue" command exposed by default.
  // For now, we re-open the save dialogue pre-populated. Phase 2 will add a
  // dedicated write_to_path command in Rust for true silent save (Cmd+S).
  const savedPath = await tauriInvoke('save_file', {
    content: html,
    suggestedName: path.split('/').pop(),
  });
  if (savedPath) markClean();
}

/* ─── Dirty state tracking ───────────────────────────────────────────────── */

function markDirty() {
  if (!State.isDirty) {
    State.isDirty = true;
    saveStateEl.textContent = 'Unsaved changes';
  }
}

function markClean() {
  State.isDirty = false;
  saveStateEl.textContent = '';
}

/* ─── UI updates ─────────────────────────────────────────────────────────── */

function updateTitleDisplay() {
  titleDisplay.textContent = State.title;
  document.title = `${State.title} — Workplace Memo`;
}

function updateWordCount() {
  const text = canvas.innerText || '';
  const words = text.trim().split(/\s+/).filter(Boolean).length;
  statusWords.textContent = `${words} word${words !== 1 ? 's' : ''}`;
}

function updatePageCount(current, total) {
  statusPage.textContent = `Page ${current} of ${total}`;
}

function updateTemplateDisplay(name) {
  statusTemplate.textContent = name;
}

/* ─── Keyboard shortcuts ─────────────────────────────────────────────────── */

document.addEventListener('keydown', (e) => {
  const mod = e.metaKey || e.ctrlKey;
  if (!mod) return;

  switch (e.key.toLowerCase()) {
    case 'n': e.preventDefault(); newDocument(); break;
    case 'o': e.preventDefault(); openDocument(); break;
    case 's':
      e.preventDefault();
      if (e.shiftKey) {
        saveDocumentAs();
      } else {
        saveDocument();
      }
      break;
    case 'p': e.preventDefault(); window.WorkplaceExport?.print(); break;
  }
});

/* ─── Canvas change tracking ─────────────────────────────────────────────── */

canvas.addEventListener('input', () => {
  markDirty();
  updateWordCount();
  // Update title if h1 content changes
  const h1 = canvas.querySelector('h1');
  if (h1) {
    const newTitle = h1.textContent.trim();
    if (newTitle && newTitle !== State.title) {
      State.title = newTitle;
      updateTitleDisplay();
    }
  }
});

/* ─── Menu wiring ────────────────────────────────────────────────────────── */

function positionDropdown(menu, anchor) {
  const rect = anchor.getBoundingClientRect();
  menu.style.top  = `${rect.bottom}px`;
  menu.style.left = `${rect.left}px`;
}

function openMenu(menuId, anchorEl) {
  closeAllMenus();
  const menu = document.getElementById(`menu-${menuId}-dropdown`);
  if (!menu) return;
  positionDropdown(menu, anchorEl);
  menu.classList.remove('hidden');
  document.getElementById('dropdown-overlay').classList.remove('hidden');
  anchorEl.classList.add('open');
}

function closeAllMenus() {
  document.querySelectorAll('.dropdown').forEach(d => d.classList.add('hidden'));
  document.getElementById('dropdown-overlay').classList.add('hidden');
  document.querySelectorAll('.menu-item.open').forEach(m => m.classList.remove('open'));
}

document.getElementById('menu-file').addEventListener('click', function () {
  openMenu('file', this);
});
document.getElementById('menu-insert').addEventListener('click', function () {
  openMenu('insert', this);
});
document.getElementById('menu-document').addEventListener('click', function () {
  openMenu('document', this);
});

document.getElementById('dropdown-overlay').addEventListener('click', closeAllMenus);

// Wire dropdown actions
document.querySelectorAll('[data-action]').forEach(btn => {
  btn.addEventListener('click', (e) => {
    const action = e.currentTarget.dataset.action;
    closeAllMenus();
    handleAction(action);
  });
});

function handleAction(action) {
  switch (action) {
    case 'new':           newDocument(); break;
    case 'open':          openDocument(); break;
    case 'save':          saveDocument(); break;
    case 'save-as':       saveDocumentAs(); break;
    case 'export-html':   window.WorkplaceExport?.exportHTML(); break;
    case 'print':         window.WorkplaceExport?.print(); break;
    case 'insert-hr':
      document.execCommand('insertHorizontalRule');
      markDirty();
      break;
    case 'insert-pagebreak':
      // Insert a Paged.js-compatible page break
      document.execCommand('insertHTML',
        false,
        '<div style="break-before:page;page-break-before:always;" contenteditable="false">&nbsp;</div>');
      markDirty();
      break;
    case 'page-size-a4':     setPageSize('A4'); break;
    case 'page-size-letter': setPageSize('Letter'); break;
    case 'margins-normal':   setMargins(25); break;
    case 'margins-narrow':   setMargins(15); break;
    case 'margins-wide':     setMargins(38); break;
    case 'fonts-panel':      toggleFontsPanel(); break;
  }
}

/* ─── Page geometry ──────────────────────────────────────────────────────── */

function setPageSize(size) {
  State.pageSize = size;
  // Update canvas dimensions to match paper
  if (size === 'A4') {
    canvas.style.width    = '794px';
    canvas.style.minHeight = '1123px';
  } else { // Letter
    canvas.style.width    = '816px';
    canvas.style.minHeight = '1056px';
  }
  if (window.WorkplacePagination) WorkplacePagination.refresh();
  // Page size is embedded in the exported document's @page CSS (see export.js
  // buildPageCSS) — an un-saved change here must not be silently discardable.
  markDirty();
}

function setMargins(mm) {
  State.marginsMm = mm;
  const px = Math.round(mm * 3.7795); // 1mm ≈ 3.7795px at 96dpi
  canvas.style.padding = `${px}px`;
  if (window.WorkplacePagination) WorkplacePagination.refresh();
  // Margins are embedded in the exported document's @page CSS (see export.js
  // buildPageCSS) — an un-saved change here must not be silently discardable.
  markDirty();
}

/* ─── Fonts panel ────────────────────────────────────────────────────────── */

function toggleFontsPanel() {
  const panel = document.getElementById('fonts-panel');
  panel.classList.toggle('hidden');
}

document.getElementById('fonts-panel-close').addEventListener('click', () => {
  document.getElementById('fonts-panel').classList.add('hidden');
});

/* ─── Platform indicator ─────────────────────────────────────────────────── */

function detectPlatform() {
  if (!isTauri()) return 'Browser';
  const ua = navigator.userAgent;
  if (ua.includes('Mac'))   return 'macOS';
  if (ua.includes('Linux')) return 'Linux';
  if (ua.includes('Win'))   return 'Windows';
  return 'Desktop';
}

statusPlatform.textContent = detectPlatform();

/* ─── Toolbar action buttons ─────────────────────────────────────────────── */

document.getElementById('btn-print').addEventListener('click', () => {
  window.WorkplaceExport?.print();
});
document.getElementById('btn-export').addEventListener('click', () => {
  window.WorkplaceExport?.exportHTML();
});

/* ─── Prevent accidental close with unsaved changes ─────────────────────── */

window.addEventListener('beforeunload', (e) => {
  if (State.isDirty) {
    e.preventDefault();
    e.returnValue = '';
  }
});

/* ─── Initialise ─────────────────────────────────────────────────────────── */

updateTitleDisplay();
updateWordCount();
markClean();

// Expose state for other modules
window.WorkplaceEditor = {
  State,
  markDirty,
  markClean,
  updatePageCount,
  updateTemplateDisplay,
};

console.info('[Workplace*Memo] Editor initialised. Tauri:', isTauri());
