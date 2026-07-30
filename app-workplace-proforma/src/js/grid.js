/**
 * Workplace*Proforma — grid.js
 * Grid rendering, cell selection, inline editing.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * Phase 1: blank spreadsheet canvas. 26 columns (A-Z) × 50 rows. Matches
 * Excel's "new workbook" default exactly. Cells are stored in the document
 * under sheet.cells as an A1-keyed object and loaded into the engine for
 * evaluation.
 */

'use strict';

window.WorkplaceGrid = (function () {

  const grid         = document.getElementById('proforma-grid');
  const statusCell   = document.getElementById('status-cell');
  const statusSel    = document.getElementById('status-selection');

  let currentDoc = null;
  let currentSheet = null;   // sheet id
  let selected = { col: 0, row: 0 };
  let showGridlines = true;

  /* ─── Public API ─────────────────────────────────────────────────── */

  function loadDocument(doc) {
    currentDoc = doc;
    const sheet = doc.sheets[0];
    currentSheet = sheet ? sheet.id : null;
    selected = { col: 0, row: 0 };
    loadCellsIntoEngine();
    render();
  }

  function setSheet(sheetId) {
    currentSheet = sheetId;
    loadCellsIntoEngine();
    render();
  }

  function getDocument() {
    // Sync engine state back into the document model before returning.
    syncEngineIntoDocument();
    return currentDoc;
  }

  function getSelected() {
    return selected;
  }

  function toggleGridlines() {
    showGridlines = !showGridlines;
    if (showGridlines) grid.classList.remove('no-gridlines');
    else               grid.classList.add('no-gridlines');
  }

  function getSelectionStatus() {
    const ref = WorkplaceEngine.coord(selected.col, selected.row);
    const cell = WorkplaceEngine.getCell(ref);
    if (!cell || cell.value === null || cell.value === undefined || cell.value === '') return '—';
    if (typeof cell.value === 'number') {
      return `Value: ${cell.value.toLocaleString('en-US', { maximumFractionDigits: 4 })}`;
    }
    return `Value: ${cell.value}`;
  }

  /* ─── Model ↔ Engine bridge ──────────────────────────────────────────── */

  function currentSheetObj() {
    if (!currentDoc) return null;
    return currentDoc.sheets.find(s => s.id === currentSheet) || currentDoc.sheets[0];
  }

  /**
   * Push the current sheet's stored cells into the engine for evaluation.
   */
  function loadCellsIntoEngine() {
    const sheet = currentSheetObj();
    if (!sheet) return;

    WorkplaceEngine.setYearColumns([]);
    WorkplaceEngine.setIdMap({});
    WorkplaceEngine.setCells({});

    const storedCells = sheet.cells || {};
    for (const ref in storedCells) {
      const entry = storedCells[ref];
      // setCell() re-parses the raw form, so numbers stored as numbers and
      // formulas stored with leading '=' round-trip correctly.
      if (entry && entry.raw !== undefined && entry.raw !== '') {
        WorkplaceEngine.setCell(ref, entry.raw);
      }
    }

    WorkplaceEngine.evaluateAll();
  }

  /**
   * Copy the engine's current cells back into the document model so they
   * persist across save/load. Only cells that actually contain user data
   * are written — empty cells are not stored.
   */
  function syncEngineIntoDocument() {
    const sheet = currentSheetObj();
    if (!sheet) return;
    const engineCells = WorkplaceEngine.getAllCells();
    sheet.cells = {};
    for (const ref in engineCells) {
      const c = engineCells[ref];
      if (c && c.raw !== undefined && c.raw !== '' && c.raw !== null) {
        sheet.cells[ref] = {
          raw: c.raw,
          format: c.format || null,
        };
      }
    }
  }

  /* ─── Render ─────────────────────────────────────────────────────────── */

  function render() {
    const sheet = currentSheetObj();
    if (!sheet) return;

    grid.innerHTML = '';

    const cols = sheet.columns || 26;
    const rows = sheet.rows    || 50;

    // Header row: corner + column letters A, B, C, ...
    // ARIA grid pattern: table has role="grid" (index.html); each <tr> gets
    // role="row", and header cells get role="columnheader"/"rowheader" so
    // assistive tech announces this as a real spreadsheet grid rather than a
    // generic table. Additive only — no change to layout, selection, or
    // editing behaviour.
    const thead = document.createElement('thead');
    const headRow = document.createElement('tr');
    headRow.setAttribute('role', 'row');

    const corner = document.createElement('th');
    corner.className = 'row-header';
    corner.textContent = '';
    corner.setAttribute('role', 'columnheader');
    corner.setAttribute('scope', 'col');
    headRow.appendChild(corner);

    for (let c = 0; c < cols; c++) {
      const h = document.createElement('th');
      h.className = 'col-header';
      h.textContent = WorkplaceEngine.colIndexToLetter(c);
      h.setAttribute('role', 'columnheader');
      h.setAttribute('scope', 'col');
      headRow.appendChild(h);
    }
    thead.appendChild(headRow);
    grid.appendChild(thead);

    // Body: one row per data row. Row numbers are 1-indexed to match Excel.
    const tbody = document.createElement('tbody');
    for (let r = 0; r < rows; r++) {
      const tr = document.createElement('tr');
      tr.setAttribute('role', 'row');

      const rn = document.createElement('td');
      rn.className = 'row-num';
      rn.textContent = r + 1;
      rn.setAttribute('role', 'rowheader');
      rn.setAttribute('scope', 'row');
      tr.appendChild(rn);

      for (let c = 0; c < cols; c++) {
        const td = document.createElement('td');
        td.className = 'cell';
        td.setAttribute('role', 'gridcell');
        td.setAttribute('aria-selected', 'false');
        const ref = WorkplaceEngine.coord(c, r);
        td.dataset.coord = ref;

        const cell = WorkplaceEngine.getCell(ref);
        if (cell) {
          if (typeof cell.value === 'number') {
            td.textContent = WorkplaceEngine.formatValue(cell.value, cell.format);
            if (cell.value < 0) td.classList.add('negative');
          } else if (cell.value !== null && cell.value !== undefined) {
            td.textContent = String(cell.value);
            if (typeof cell.value === 'string' && !/^-?[\d.,%$]+$/.test(cell.value)) {
              td.classList.add('text');
            }
          }
        }

        tr.appendChild(td);
      }
      tbody.appendChild(tr);
    }
    grid.appendChild(tbody);

    refreshSelection();
  }

  /* ─── Selection ──────────────────────────────────────────────────────── */

  function selectCell(col, row) {
    const sheet = currentSheetObj();
    const maxCol = (sheet && sheet.columns) ? sheet.columns - 1 : 25;
    const maxRow = (sheet && sheet.rows)    ? sheet.rows    - 1 : 49;
    selected.col = Math.max(0, Math.min(col, maxCol));
    selected.row = Math.max(0, Math.min(row, maxRow));
    refreshSelection();
  }

  function refreshSelection() {
    grid.querySelectorAll('td.cell.selected').forEach(td => {
      td.classList.remove('selected');
      td.setAttribute('aria-selected', 'false');
    });
    const ref = WorkplaceEngine.coord(selected.col, selected.row);
    const td = grid.querySelector(`td[data-coord="${ref}"]`);
    if (td) {
      td.classList.add('selected');
      td.setAttribute('aria-selected', 'true');
      // Scroll the selected cell into view if it's outside the viewport
      td.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }

    statusCell.textContent = ref;
    document.getElementById('cell-ref').textContent = ref;
    statusSel.textContent = getSelectionStatus();

    if (window.WorkplaceFormulaBar) {
      WorkplaceFormulaBar.onSelectionChange();
    }
  }

  /* ─── Cell edit ──────────────────────────────────────────────────────── */

  function beginEditSelected(initialChar) {
    const ref = WorkplaceEngine.coord(selected.col, selected.row);
    const td = grid.querySelector(`td[data-coord="${ref}"]`);
    if (!td) return;

    const cell = WorkplaceEngine.getCell(ref);
    const initial = (initialChar !== undefined)
      ? initialChar
      : (cell ? (cell.formula ? '=' + cell.formula : String(cell.raw ?? '')) : '');

    const input = document.createElement('input');
    input.className = 'cell-editor';
    input.value = initial;

    td.innerHTML = '';
    td.appendChild(input);
    input.focus();
    if (initialChar === undefined) input.select();

    function commit(move) {
      WorkplaceEngine.setCell(ref, input.value);
      WorkplaceEngine.evaluateAll();
      if (window.WorkplaceApp) WorkplaceApp.markDirty();
      render();
      if (move === 'down')  selectCell(selected.col, selected.row + 1);
      else if (move === 'right') selectCell(selected.col + 1, selected.row);
      else refreshSelection();
    }

    function cancel() {
      render();
      refreshSelection();
    }

    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commit('down');
      } else if (e.key === 'Tab') {
        e.preventDefault();
        commit('right');
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancel();
      }
    });
    input.addEventListener('blur', () => commit(null));
  }

  /* ─── Keyboard navigation ────────────────────────────────────────────── */

  document.addEventListener('keydown', (e) => {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT' || e.target.tagName === 'TEXTAREA') return;
    if (document.querySelector('.dropdown:not(.hidden)')) return;
    // Keyboard-shortcut help overlay (app.js) is modal — suppress grid
    // navigation/editing while it's open so arrow keys/typing don't silently
    // move the selection or start an edit underneath it.
    if (document.getElementById('shortcut-overlay') &&
        !document.getElementById('shortcut-overlay').classList.contains('hidden')) return;

    const mod = e.metaKey || e.ctrlKey;
    if (mod) return;

    switch (e.key) {
      case 'ArrowUp':    selectCell(selected.col, selected.row - 1); e.preventDefault(); break;
      case 'ArrowDown':  selectCell(selected.col, selected.row + 1); e.preventDefault(); break;
      case 'ArrowLeft':  selectCell(selected.col - 1, selected.row); e.preventDefault(); break;
      case 'ArrowRight': selectCell(selected.col + 1, selected.row); e.preventDefault(); break;
      case 'Enter':
      case 'F2':         beginEditSelected(); e.preventDefault(); break;
      case 'Delete':
      case 'Backspace': {
        const ref = WorkplaceEngine.coord(selected.col, selected.row);
        WorkplaceEngine.setCell(ref, '');
        WorkplaceEngine.evaluateAll();
        if (window.WorkplaceApp) WorkplaceApp.markDirty();
        render();
        e.preventDefault();
        break;
      }
      default:
        if (e.key.length === 1 && !e.altKey) {
          beginEditSelected(e.key);
          e.preventDefault();
        }
    }
  });

  /* ─── Mouse ──────────────────────────────────────────────────────────── */

  grid.addEventListener('click', (e) => {
    const td = e.target.closest('td[data-coord]');
    if (!td) return;
    const parsed = WorkplaceEngine.parseRef(td.dataset.coord);
    if (!parsed) return;
    selectCell(parsed.col, parsed.row);
  });

  grid.addEventListener('dblclick', (e) => {
    const td = e.target.closest('td[data-coord]');
    if (!td) return;
    const parsed = WorkplaceEngine.parseRef(td.dataset.coord);
    if (!parsed) return;
    selectCell(parsed.col, parsed.row);
    beginEditSelected();
  });

  return {
    loadDocument,
    getDocument,
    setSheet,
    render,
    selectCell,
    getSelected,
    toggleGridlines,
    beginEditSelected,
    syncEngineIntoDocument,
  };

})();
