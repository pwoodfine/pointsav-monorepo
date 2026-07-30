/**
 * Workplace*Proforma — formula-bar.js
 * Wires the formula bar to the current cell selection.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * Clicking into the formula bar or editing its contents commits the value
 * to the currently selected cell on Enter.
 */

'use strict';

window.WorkplaceFormulaBar = (function () {

  const input = document.getElementById('formula-input');
  const cellRef = document.getElementById('cell-ref');

  function onSelectionChange() {
    const sel = WorkplaceGrid.getSelected();
    const ref = WorkplaceEngine.coord(sel.col, sel.row);
    cellRef.textContent = ref;

    const cell = WorkplaceEngine.getCell(ref);
    if (!cell) {
      input.value = '';
    } else if (cell.formula) {
      input.value = '=' + cell.formula;
    } else if (cell.raw !== undefined) {
      input.value = String(cell.raw);
    } else {
      input.value = '';
    }
  }

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      const sel = WorkplaceGrid.getSelected();
      const ref = WorkplaceEngine.coord(sel.col, sel.row);
      WorkplaceEngine.setCell(ref, input.value);
      WorkplaceEngine.evaluateAll();
      if (window.WorkplaceApp) WorkplaceApp.markDirty();
      WorkplaceGrid.render();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onSelectionChange();
      input.blur();
    }
  });

  return { onSelectionChange };

})();
