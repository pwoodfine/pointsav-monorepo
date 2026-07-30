/**
 * Workplace*Memo — toolbar.js
 * Handles all toolbar formatting commands, font/size selection,
 * colour pickers, and toolbar state updates (bold active, etc.)
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 */

'use strict';

const canvas = document.getElementById('document-canvas');

/* ─── execCommand wrappers ───────────────────────────────────────────────── */

function formatDoc(cmd, value = null) {
  canvas.focus();
  document.execCommand(cmd, false, value);
  updateToolbarState();
  window.WorkplaceEditor?.markDirty();
}

/* ─── Toolbar button wiring ──────────────────────────────────────────────── */

document.querySelectorAll('[data-cmd]').forEach(btn => {
  btn.addEventListener('mousedown', (e) => {
    e.preventDefault(); // prevent canvas blur
    formatDoc(e.currentTarget.dataset.cmd);
  });
});

/* ─── Font family ────────────────────────────────────────────────────────── */

document.getElementById('font-family').addEventListener('change', function () {
  formatDoc('fontName', this.value);
});

/* ─── Font size ──────────────────────────────────────────────────────────── */

document.getElementById('font-size').addEventListener('change', function () {
  // execCommand fontSize uses 1-7 scale; we use a span wrapper for pt sizes
  const ptSize = parseInt(this.value, 10);
  // Wrap selection in a span with explicit font-size
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0) return;
  const range = sel.getRangeAt(0);
  if (range.collapsed) return;

  const span = document.createElement('span');
  span.style.fontSize = `${ptSize}pt`;
  try {
    range.surroundContents(span);
  } catch {
    // Range spans multiple elements — fall back to execCommand
    formatDoc('fontSize', '3'); // no-op sizing; user sees visual change
  }
  window.WorkplaceEditor?.markDirty();
});

/* ─── Colour pickers ─────────────────────────────────────────────────────── */

const colourPicker    = document.getElementById('colour-picker');
const highlightPicker = document.getElementById('highlight-picker');

colourPicker.addEventListener('input', function () {
  canvas.focus();
  document.execCommand('foreColor', false, this.value);
  window.WorkplaceEditor?.markDirty();
});

highlightPicker.addEventListener('input', function () {
  canvas.focus();
  document.execCommand('hiliteColor', false, this.value);
  window.WorkplaceEditor?.markDirty();
});

/* ─── Template selector ──────────────────────────────────────────────────── */

document.getElementById('template-select').addEventListener('change', function () {
  if (window.WorkplaceTemplates) {
    WorkplaceTemplates.apply(this.value);
    window.WorkplaceEditor?.updateTemplateDisplay(
      this.options[this.selectedIndex].text
    );
    window.WorkplaceEditor?.State && (window.WorkplaceEditor.State.templateKey = this.value);
    // The template key is embedded in the exported document (see export.js
    // assembleHTML) — switching templates is a real document change and must
    // not be silently discardable via an un-set dirty flag.
    window.WorkplaceEditor?.markDirty();
  }
});

/* ─── Toolbar state update (bold/italic active indicators) ───────────────── */

function updateToolbarState() {
  const commands = ['bold', 'italic', 'underline', 'strikeThrough'];
  commands.forEach(cmd => {
    const btn = document.querySelector(`[data-cmd="${cmd}"]`);
    if (!btn) return;
    try {
      btn.classList.toggle('active', document.queryCommandState(cmd));
    } catch { /* queryCommandState can throw in some WebKit versions */ }
  });
}

// Update toolbar state on selection change
document.addEventListener('selectionchange', () => {
  // Only update if selection is within the canvas
  const sel = window.getSelection();
  if (sel && canvas.contains(sel.anchorNode)) {
    updateToolbarState();
  }
});

window.WorkplaceToolbar = { updateToolbarState };
