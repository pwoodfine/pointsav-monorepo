/**
 * Workplace*Proforma — export.js
 * Print / PDF path and XLSX export stub.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * PDF is generated through the OS print dialogue via @media print CSS —
 * the application chrome is hidden, the grid container is flattened onto
 * the print surface, and the OS produces a high-fidelity PDF. This matches
 * app-workplace-memo's approach: no third-party PDF library required.
 *
 * XLSX export is stubbed in Phase 1. Phase 2 adds rust_xlsxwriter (or
 * xlsxwriter via Python sidecar) invoked through an IPC command.
 */

'use strict';

window.WorkplaceExport = (function () {

  function print() {
    window.print();
  }

  function exportXlsx() {
    alert(
      'XLSX export is a Phase 2 feature.\n\n' +
      'The canonical file format is .json — see docs/schema.md. ' +
      'XLSX is generated on demand from that canonical source for ' +
      'recipients whose systems require Excel.\n\n' +
      'In the Phase 1 MVP, use Print / Save as PDF for distribution.'
    );
  }

  return { print, exportXlsx };

})();
