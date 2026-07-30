/**
 * Workplace*Proforma — toolbar.js
 * Formatting buttons, gridlines, recalculate, and toolbar actions.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 */

'use strict';

window.WorkplaceToolbar = (function () {

  function applyFormat(fmt) {
    const sel = WorkplaceGrid.getSelected();
    const ref = WorkplaceEngine.coord(sel.col, sel.row);
    const cell = WorkplaceEngine.getCell(ref);
    if (!cell) return;

    const map = {
      general:  'number-2dp',
      currency: 'currency-0dp',
      percent:  'percent-1dp',
      comma:    'number-0dp',
    };

    cell.format = map[fmt] || 'number-2dp';
    WorkplaceGrid.render();
    if (window.WorkplaceApp) WorkplaceApp.markDirty();
  }

  function adjustDecimals(delta) {
    const sel = WorkplaceGrid.getSelected();
    const ref = WorkplaceEngine.coord(sel.col, sel.row);
    const cell = WorkplaceEngine.getCell(ref);
    if (!cell) return;

    const current = cell.format || 'number-2dp';
    const m = current.match(/^(.+?)-(\d+)dp$/);
    if (!m) return;

    const base = m[1];
    let dp = parseInt(m[2], 10) + delta;
    if (dp < 0) dp = 0;
    if (dp > 6) dp = 6;

    cell.format = `${base}-${dp}dp`;
    WorkplaceGrid.render();
    if (window.WorkplaceApp) WorkplaceApp.markDirty();
  }

  /* ─── Wire number format buttons ─────────────────────────────────────── */

  document.querySelectorAll('#toolbar [data-fmt]').forEach(btn => {
    btn.addEventListener('click', () => applyFormat(btn.dataset.fmt));
  });

  /* ─── Decimal places ─────────────────────────────────────────────────── */

  document.getElementById('dec-increase').addEventListener('click', () => adjustDecimals(1));
  document.getElementById('dec-decrease').addEventListener('click', () => adjustDecimals(-1));

  /* ─── Alignment (visual-only for MVP; tracked in cell.align later) ───── */

  document.querySelectorAll('#toolbar [data-align]').forEach(btn => {
    btn.addEventListener('click', () => {
      const sel = WorkplaceGrid.getSelected();
      const ref = WorkplaceEngine.coord(sel.col, sel.row);
      const cell = WorkplaceEngine.getCell(ref);
      if (cell) {
        cell.align = btn.dataset.align;
        WorkplaceGrid.render();
        if (window.WorkplaceApp) WorkplaceApp.markDirty();
      }
    });
  });

  /* ─── Bold / italic / underline (visual for MVP) ─────────────────────── */

  document.querySelectorAll('#toolbar [data-style]').forEach(btn => {
    btn.addEventListener('click', () => {
      const sel = WorkplaceGrid.getSelected();
      const ref = WorkplaceEngine.coord(sel.col, sel.row);
      const cell = WorkplaceEngine.getCell(ref);
      if (cell) {
        cell[btn.dataset.style] = !cell[btn.dataset.style];
        btn.classList.toggle('active', !!cell[btn.dataset.style]);
        if (window.WorkplaceApp) WorkplaceApp.markDirty();
      }
    });
  });

  /* ─── Gridlines ──────────────────────────────────────────────────────── */

  document.getElementById('btn-gridlines').addEventListener('click', () => {
    WorkplaceGrid.toggleGridlines();
  });

  /* ─── Recalculate ────────────────────────────────────────────────────── */

  document.getElementById('btn-recalc').addEventListener('click', () => {
    WorkplaceEngine.evaluateAll();
    WorkplaceGrid.render();
    document.getElementById('status-engine').textContent = 'Recalculated';
    setTimeout(() => {
      document.getElementById('status-engine').textContent = 'Ready';
    }, 1200);
  });

  /* ─── Font size ──────────────────────────────────────────────────────── */

  document.getElementById('font-size').addEventListener('change', (e) => {
    document.documentElement.style.setProperty('--grid-size', e.target.value + 'px');
  });

  /* ─── Print / Export ─────────────────────────────────────────────────── */

  document.getElementById('btn-print').addEventListener('click', () => {
    window.WorkplaceExport && WorkplaceExport.print();
  });

  document.getElementById('btn-export-xlsx').addEventListener('click', () => {
    window.WorkplaceExport && WorkplaceExport.exportXlsx();
  });

})();
