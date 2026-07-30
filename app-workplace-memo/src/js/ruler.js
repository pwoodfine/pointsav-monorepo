/**
 * Workplace*Memo — ruler.js
 * Draws the document ruler with margin indicators and draggable handles.
 * Shows mm (A4) or inch (Letter) units based on document page size.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 */

'use strict';

const rulerCanvas = document.getElementById('ruler-canvas');
const ctx         = rulerCanvas ? rulerCanvas.getContext('2d') : null;

// Ruler colours (match app.css chrome variables)
const RULER_BG        = '#28282f';
const RULER_TICK      = '#666';
const RULER_TICK_LONG = '#999';
const RULER_TEXT      = '#888';
const MARGIN_FILL     = 'rgba(200,169,110,0.12)';
const MARGIN_HANDLE   = '#c8a96e';

/* ─── Ruler state ────────────────────────────────────────────────────────── */

const RulerState = {
  pageWidthPx:  794,   // A4 at 96dpi
  marginPx:     96,    // ~25mm at 96dpi
  unit:         'mm',  // 'mm' | 'in'
  dragging:     null,  // 'left' | 'right' | null
  dragStartX:   0,
  dragStartMargin: 0,
};

/* ─── Draw ruler ─────────────────────────────────────────────────────────── */

function drawRuler() {
  if (!ctx || !rulerCanvas) return;

  const W = rulerCanvas.offsetWidth;
  const H = rulerCanvas.height;
  rulerCanvas.width = W;  // reset to clear

  ctx.fillStyle = RULER_BG;
  ctx.fillRect(0, 0, W, H);

  // Centre of the ruler = centre of viewport
  const centreX = W / 2;
  // Left edge of page within the ruler
  const pageLeft = centreX - RulerState.pageWidthPx / 2;

  // Draw margin fill zones
  ctx.fillStyle = MARGIN_FILL;
  ctx.fillRect(pageLeft, 0, RulerState.marginPx, H);
  ctx.fillRect(pageLeft + RulerState.pageWidthPx - RulerState.marginPx, 0, RulerState.marginPx, H);

  // Draw tick marks
  const isInch = RulerState.unit === 'in';
  const unitPx = isInch ? 96 : 37.795;   // 96px = 1 inch; 37.795px = 10mm
  const majorEvery = isInch ? 1 : 10;     // label every 1 inch or 10mm
  const minorPer   = isInch ? 8 : 5;      // minor ticks: 8 per inch, every 2mm

  const totalUnits = Math.ceil(RulerState.pageWidthPx / unitPx) + 2;
  const originMajor = Math.floor((W / 2 - pageLeft) / unitPx);

  for (let i = -originMajor - 1; i < totalUnits + 2; i++) {
    const x = pageLeft + i * unitPx;
    if (x < 0 || x > W) continue;

    // Major tick
    ctx.strokeStyle = RULER_TICK_LONG;
    ctx.lineWidth   = 1;
    ctx.beginPath();
    ctx.moveTo(x, H - 6);
    ctx.lineTo(x, H);
    ctx.stroke();

    // Label
    const labelVal = i * majorEvery;
    if (labelVal >= 0) {
      ctx.fillStyle  = RULER_TEXT;
      ctx.font       = '9px -apple-system, sans-serif';
      ctx.textAlign  = 'center';
      ctx.fillText(String(labelVal), x, H - 8);
    }

    // Minor ticks
    const minorStep = unitPx / minorPer;
    for (let m = 1; m < minorPer; m++) {
      const mx = x + m * minorStep;
      if (mx < 0 || mx > W) continue;
      const tickH = (m === minorPer / 2) ? 4 : 2;
      ctx.strokeStyle = RULER_TICK;
      ctx.beginPath();
      ctx.moveTo(mx, H - tickH);
      ctx.lineTo(mx, H);
      ctx.stroke();
    }
  }

  // Margin handle lines
  const leftHandleX  = pageLeft + RulerState.marginPx;
  const rightHandleX = pageLeft + RulerState.pageWidthPx - RulerState.marginPx;

  [leftHandleX, rightHandleX].forEach(hx => {
    ctx.strokeStyle = MARGIN_HANDLE;
    ctx.lineWidth   = 1.5;
    ctx.beginPath();
    ctx.moveTo(hx, 0);
    ctx.lineTo(hx, H);
    ctx.stroke();

    // Handle triangle indicator
    ctx.fillStyle = MARGIN_HANDLE;
    ctx.beginPath();
    ctx.moveTo(hx - 4, 0);
    ctx.lineTo(hx + 4, 0);
    ctx.lineTo(hx,     6);
    ctx.closePath();
    ctx.fill();
  });
}

/* ─── Drag handle interaction ────────────────────────────────────────────── */

function getHandleAtX(clientX) {
  if (!rulerCanvas) return null;
  const rect     = rulerCanvas.getBoundingClientRect();
  const x        = clientX - rect.left;
  const centreX  = rect.width / 2;
  const pageLeft = centreX - RulerState.pageWidthPx / 2;
  const leftHX   = pageLeft + RulerState.marginPx;
  const rightHX  = pageLeft + RulerState.pageWidthPx - RulerState.marginPx;

  if (Math.abs(x - leftHX)  < 6) return 'left';
  if (Math.abs(x - rightHX) < 6) return 'right';
  return null;
}

if (rulerCanvas) {
  rulerCanvas.addEventListener('mousedown', (e) => {
    const handle = getHandleAtX(e.clientX);
    if (!handle) return;
    RulerState.dragging = handle;
    RulerState.dragStartX = e.clientX;
    RulerState.dragStartMargin = RulerState.marginPx;
    e.preventDefault();
  });

  rulerCanvas.addEventListener('mousemove', (e) => {
    const handle = getHandleAtX(e.clientX);
    rulerCanvas.style.cursor = handle ? 'ew-resize' : 'default';
  });
}

document.addEventListener('mousemove', (e) => {
  if (!RulerState.dragging) return;
  const delta = e.clientX - RulerState.dragStartX;
  const direction = RulerState.dragging === 'left' ? 1 : -1;
  const newMarginPx = Math.max(30, Math.min(
    RulerState.pageWidthPx / 2 - 100,
    RulerState.dragStartMargin + direction * delta
  ));

  RulerState.marginPx = newMarginPx;

  // Apply to canvas immediately
  const canvas = document.getElementById('document-canvas');
  if (canvas) canvas.style.padding = `${newMarginPx}px`;

  // Update state
  const mm = Math.round(newMarginPx / 3.7795);
  if (window.WorkplaceEditor) window.WorkplaceEditor.State.marginsMm = mm;

  drawRuler();
});

document.addEventListener('mouseup', () => {
  if (RulerState.dragging) {
    RulerState.dragging = null;
    if (window.WorkplacePagination) WorkplacePagination.refresh();
  }
});

/* ─── Resize observer ────────────────────────────────────────────────────── */

if (rulerCanvas && typeof ResizeObserver !== 'undefined') {
  new ResizeObserver(() => drawRuler()).observe(rulerCanvas);
} else {
  window.addEventListener('resize', drawRuler);
}

/* ─── Initialise ─────────────────────────────────────────────────────────── */

document.addEventListener('DOMContentLoaded', drawRuler);

window.WorkplaceRuler = {
  draw:     drawRuler,
  setState: (key, val) => { RulerState[key] = val; drawRuler(); },
};
