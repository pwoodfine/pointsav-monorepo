/**
 * Workplace*Proforma — app.js
 * Main application controller: document state, file operations, menu wiring,
 * save state indicator, platform detection.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * This file runs last in the script loading order (see index.html).
 * All other modules (schema, engine, grid, formula-bar, toolbar, export)
 * are expected to be initialised before this runs.
 */

'use strict';

window.WorkplaceApp = (function () {

  /* ─── Document State ───────────────────────────────────────────────────── */

  const State = {
    doc:       null,      // the canonical JSON document
    savedPath: null,      // last path we saved to (null = never saved)
    isDirty:   false,     // unsaved changes?
  };

  /* ─── DOM refs ─────────────────────────────────────────────────────────── */

  const titleDisplay   = document.getElementById('doc-title-display');
  const saveStateEl    = document.getElementById('save-state');
  const statusPlatform = document.getElementById('status-platform');
  const statusEngine   = document.getElementById('status-engine');
  const statusSchema   = document.getElementById('status-schema');

  const shortcutOverlay = document.getElementById('shortcut-overlay');
  const shortcutDialog  = document.getElementById('shortcut-dialog');
  const shortcutClose   = document.getElementById('shortcut-dialog-close');
  let lastFocusedBeforeOverlay = null;

  /* ─── Tauri IPC bridge ─────────────────────────────────────────────────── */

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

  /* ─── Initial load ─────────────────────────────────────────────────────── */

  function init() {
    State.doc = WorkplaceSchema.newDocument();
    WorkplaceGrid.loadDocument(State.doc);
    updateTitleDisplay();
    markClean();

    statusSchema.textContent = `Schema ${WorkplaceSchema.SCHEMA_VERSION}`;
    statusEngine.textContent = 'Ready';
    statusPlatform.textContent = isTauri() ? 'Tauri · offline' : 'Browser · offline';
  }

  /* ─── File operations ──────────────────────────────────────────────────── */

  async function newDocument() {
    if (State.isDirty) {
      if (!confirm('You have unsaved changes. Create a new proforma anyway?')) return;
    }
    State.doc = WorkplaceSchema.newDocument();
    State.savedPath = null;
    WorkplaceGrid.loadDocument(State.doc);
    updateTitleDisplay();
    markClean();
  }

  async function openDocument() {
    if (State.isDirty) {
      if (!confirm('You have unsaved changes. Open another proforma anyway?')) return;
    }
    const content = await tauriInvoke('open_file');
    if (content === null || content === undefined) return;

    let parsed;
    try {
      parsed = JSON.parse(content);
    } catch (err) {
      alert('Could not parse file as JSON:\n' + err.message);
      return;
    }

    const validation = WorkplaceSchema.validate(parsed);
    if (!validation.ok) {
      alert('The file is not a valid proforma document:\n' + validation.error);
      return;
    }

    State.doc = parsed;
    WorkplaceGrid.loadDocument(State.doc);
    updateTitleDisplay();
    markClean();
  }

  async function saveDocument() {
    if (State.savedPath) {
      await writeToPath(State.savedPath);
    } else {
      await saveDocumentAs();
    }
  }

  async function saveDocumentAs() {
    // Update metadata before serialising
    State.doc.metadata.last_modified = WorkplaceSchema.nowIso();

    const canonical = WorkplaceSchema.canonicalise(State.doc);
    const hash = await WorkplaceSchema.sha256Hex(canonical);
    State.doc.audit.sha256 = hash;
    State.doc.audit.signed_at = State.doc.metadata.last_modified;

    const finalContent = WorkplaceSchema.canonicalise(State.doc);

    const suggestedName = (State.doc.metadata.title || 'proforma')
      .replace(/[^a-zA-Z0-9\s\-_]/g, '')
      .trim()
      .replace(/\s+/g, '-') + '.json';

    const savedPath = await tauriInvoke('save_file', {
      content: finalContent,
      suggestedName,
    });

    if (savedPath) {
      State.savedPath = savedPath;
      markClean();
    }
  }

  async function writeToPath(path) {
    // Phase 1 uses save_file even for silent overwrites; Phase 2 adds a
    // dedicated write_to_path IPC command for true silent Ctrl+S.
    State.doc.metadata.last_modified = WorkplaceSchema.nowIso();

    const canonical = WorkplaceSchema.canonicalise(State.doc);
    const hash = await WorkplaceSchema.sha256Hex(canonical);
    State.doc.audit.sha256 = hash;
    State.doc.audit.signed_at = State.doc.metadata.last_modified;

    const finalContent = WorkplaceSchema.canonicalise(State.doc);

    const savedPath = await tauriInvoke('save_file', {
      content: finalContent,
      suggestedName: path.split('/').pop(),
    });
    if (savedPath) markClean();
  }

  /* ─── Dirty state tracking ─────────────────────────────────────────────── */

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

  /* ─── UI updates ───────────────────────────────────────────────────────── */

  function updateTitleDisplay() {
    const title = State.doc?.metadata?.title || 'Untitled';
    titleDisplay.textContent = title;
    document.title = `${title} — Workplace Proforma`;
  }

  /* ─── Keyboard-shortcut help overlay ───────────────────────────────────── */
  // Additive, frontend-only surface — F1 opens/closes it; Tools ▸ Keyboard
  // Shortcuts gives a mouse-driven path to the same dialog for discoverability.
  // The roadmap named "?/F1" as the trigger, but "?" is deliberately NOT bound
  // as a global shortcut here: grid.js's keydown handler treats any single
  // printable character as "start editing the selected cell with that
  // character" whenever a dropdown/help overlay isn't open, so binding "?"
  // globally would silently hijack typing a literal "?" into a cell. F1 has
  // no such conflict (grid.js only acts on keys with length === 1) and is
  // the sole keyboard trigger implemented tonight.

  function isShortcutOverlayOpen() {
    return !shortcutOverlay.classList.contains('hidden');
  }

  function openShortcutOverlay() {
    if (isShortcutOverlayOpen()) return;
    lastFocusedBeforeOverlay = document.activeElement;
    closeAllMenus();
    shortcutOverlay.classList.remove('hidden');
    shortcutDialog.focus();
  }

  function closeShortcutOverlay() {
    if (!isShortcutOverlayOpen()) return;
    shortcutOverlay.classList.add('hidden');
    if (lastFocusedBeforeOverlay && typeof lastFocusedBeforeOverlay.focus === 'function') {
      lastFocusedBeforeOverlay.focus();
    }
    lastFocusedBeforeOverlay = null;
  }

  function toggleShortcutOverlay() {
    if (isShortcutOverlayOpen()) closeShortcutOverlay();
    else openShortcutOverlay();
  }

  shortcutClose.addEventListener('click', closeShortcutOverlay);
  shortcutOverlay.addEventListener('click', (e) => {
    if (e.target === shortcutOverlay) closeShortcutOverlay();
  });

  /* ─── Keyboard shortcuts ───────────────────────────────────────────────── */

  document.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;

    // F1 = toggle the keyboard-shortcut help overlay (no modifier required)
    if (e.key === 'F1') {
      e.preventDefault();
      toggleShortcutOverlay();
      return;
    }

    // Esc closes the help overlay when it's open, before any other handling.
    if (e.key === 'Escape' && isShortcutOverlayOpen()) {
      e.preventDefault();
      closeShortcutOverlay();
      return;
    }

    // While the help overlay is open, suppress every other document-level
    // shortcut (recalculate, save, grid navigation/editing in grid.js, etc.)
    // so it behaves like a real modal dialog — only F1/Esc act on it.
    if (isShortcutOverlayOpen()) return;

    // F9 = Recalculate (no modifier required)
    if (e.key === 'F9') {
      e.preventDefault();
      WorkplaceEngine.evaluateAll();
      WorkplaceGrid.render();
      statusEngine.textContent = 'Recalculated';
      setTimeout(() => { statusEngine.textContent = 'Ready'; }, 1200);
      return;
    }

    if (!mod) return;

    switch (e.key.toLowerCase()) {
      case 'n': e.preventDefault(); newDocument(); break;
      case 'o': e.preventDefault(); openDocument(); break;
      case 's':
        e.preventDefault();
        if (e.shiftKey) saveDocumentAs();
        else            saveDocument();
        break;
      case 'p': e.preventDefault(); window.WorkplaceExport?.print(); break;
    }
  });

  /* ─── Menu wiring ──────────────────────────────────────────────────────── */

  function positionDropdown(menu, anchor) {
    const rect = anchor.getBoundingClientRect();
    menu.style.top  = `${rect.bottom}px`;
    menu.style.left = `${rect.left}px`;
  }

  function openMenu(menuId, anchor) {
    closeAllMenus();
    const menu = document.getElementById(menuId);
    if (!menu) return;
    menu.classList.remove('hidden');
    positionDropdown(menu, anchor);
    anchor.classList.add('open');
    document.getElementById('dropdown-overlay').classList.remove('hidden');
  }

  function closeAllMenus() {
    document.querySelectorAll('.dropdown').forEach(d => d.classList.add('hidden'));
    document.querySelectorAll('.menu-item.open').forEach(i => i.classList.remove('open'));
    document.getElementById('dropdown-overlay').classList.add('hidden');
  }

  const menus = [
    { btn: 'menu-file',   dropdown: 'menu-file-dropdown' },
    { btn: 'menu-edit',   dropdown: 'menu-edit-dropdown' },
    { btn: 'menu-view',   dropdown: 'menu-view-dropdown' },
    { btn: 'menu-insert', dropdown: 'menu-insert-dropdown' },
    { btn: 'menu-format', dropdown: 'menu-format-dropdown' },
    { btn: 'menu-data',   dropdown: 'menu-data-dropdown' },
    { btn: 'menu-tools',  dropdown: 'menu-tools-dropdown' },
  ];

  menus.forEach(m => {
    const btn = document.getElementById(m.btn);
    if (!btn) return;
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const wasOpen = !document.getElementById(m.dropdown).classList.contains('hidden');
      if (wasOpen) closeAllMenus();
      else openMenu(m.dropdown, btn);
    });
  });

  document.getElementById('dropdown-overlay').addEventListener('click', closeAllMenus);

  document.querySelectorAll('.dropdown').forEach(d => {
    d.addEventListener('click', (e) => {
      const btn = e.target.closest('button[data-action]');
      if (!btn) return;
      closeAllMenus();
      handleAction(btn.dataset.action);
    });
  });

  /* ─── Actions ──────────────────────────────────────────────────────────── */

  function handleAction(action) {
    switch (action) {
      case 'new':         newDocument(); break;
      case 'open':        openDocument(); break;
      case 'save':        saveDocument(); break;
      case 'save-as':     saveDocumentAs(); break;
      case 'print':       window.WorkplaceExport?.print(); break;
      case 'export-xlsx': window.WorkplaceExport?.exportXlsx(); break;

      case 'toggle-gridlines': WorkplaceGrid.toggleGridlines(); break;

      case 'recalculate':
        WorkplaceEngine.evaluateAll();
        WorkplaceGrid.render();
        break;

      case 'keyboard-shortcuts':
        openShortcutOverlay();
        break;

      case 'schema-info':
        alert(
          `Workplace Proforma\n` +
          `Schema version: ${WorkplaceSchema.SCHEMA_VERSION}\n` +
          `Document id: ${State.doc.document_id}\n` +
          `Last saved hash: ${State.doc.audit.sha256 || '(not yet saved)'}\n\n` +
          `See docs/schema.md for the full specification.`
        );
        break;

      // Actions not implemented in MVP — flagged for Phase 2
      default:
        console.log('[app] action pending implementation:', action);
    }
  }

  /* ─── Boot ─────────────────────────────────────────────────────────────── */

  init();

  return {
    markDirty,
    markClean,
    getState: () => State,
    handleAction,
  };

})();
