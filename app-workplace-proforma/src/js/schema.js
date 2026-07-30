/**
 * Workplace*Proforma — schema.js
 * Canonical JSON schema: defaults, validation, and initial document creation.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * The canonical document format for this application is a single JSON file
 * conforming to the schema version declared in the first field. See
 * docs/schema.md for the full specification.
 */

'use strict';

window.WorkplaceSchema = (function () {

  const SCHEMA_VERSION = '1.0';

  // Default blank-grid dimensions. Matches Excel's "new workbook" feel — a
  // blank canvas with plenty of room to start typing. Exactly this many
  // rows and columns are materialised on file creation; more can be added
  // in Phase 2 with Insert → Row/Column.
  const DEFAULT_COLUMNS = 26;   // A through Z
  const DEFAULT_ROWS    = 50;

  /**
   * Generate a UUIDv7-ish identifier. Not cryptographically strict; sufficient
   * for file identity across rename/move.
   */
  function generateDocId() {
    const timestamp = Date.now().toString(16).padStart(12, '0');
    const random = Array.from(crypto.getRandomValues(new Uint8Array(10)))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
    return `${timestamp.slice(0, 8)}-${timestamp.slice(8, 12)}-7${random.slice(0, 3)}-${random.slice(3, 7)}-${random.slice(7, 19)}`;
  }

  /**
   * Return the timestamp in ISO 8601 with timezone offset.
   */
  function nowIso() {
    const d = new Date();
    const pad = (n) => String(n).padStart(2, '0');
    const tz = -d.getTimezoneOffset();
    const sign = tz >= 0 ? '+' : '-';
    const tzh = pad(Math.floor(Math.abs(tz) / 60));
    const tzm = pad(Math.abs(tz) % 60);
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}${sign}${tzh}:${tzm}`;
  }

  /**
   * Create a new blank proforma document — a single empty sheet with a
   * 26×50 grid. Matches Excel's "new workbook" default exactly so users
   * coming from Excel see a familiar canvas on launch.
   */
  function newDocument() {
    const now = nowIso();
    return {
      proforma_version: SCHEMA_VERSION,
      document_id: generateDocId(),

      anchor: null,

      metadata: {
        title: 'Untitled Proforma',
        created: now,
        last_modified: now,
        author: '',
        description: '',
        currency: 'USD',
        locale: 'en-US',
        schema_url: 'https://schemas.pointsav.com/proforma/1.0',
      },

      template: null,

      assumptions: [],

      sheets: [
        {
          id: 'sheet1',
          name: 'Sheet1',
          type: 'blank',
          columns: DEFAULT_COLUMNS,
          rows: DEFAULT_ROWS,
          cells: {},           // keyed by A1-notation: { "A1": { raw, value, format } }
        },
      ],

      named_ranges: {},

      presentation: {
        theme: 'institutional-cream',
        frozen_panes: { columns: 0, rows: 0 },
        conditional_formatting: [],
      },

      audit: {
        sha256: null,
        signed_by: null,
        signed_at: null,
        parent_sha256: null,
        commit_context: null,
      },
    };
  }

  /**
   * Shallow validation. Checks the structural shape of the document.
   */
  function validate(doc) {
    if (!doc || typeof doc !== 'object') {
      return { ok: false, error: 'Document is not an object.' };
    }
    if (!doc.proforma_version) {
      return { ok: false, error: 'Missing proforma_version field.' };
    }
    if (!doc.document_id) {
      return { ok: false, error: 'Missing document_id field.' };
    }
    if (!doc.metadata || typeof doc.metadata !== 'object') {
      return { ok: false, error: 'Missing or malformed metadata.' };
    }
    if (!Array.isArray(doc.sheets)) {
      return { ok: false, error: 'Missing or malformed sheets array.' };
    }
    return { ok: true };
  }

  /**
   * SHA-256 hex digest of the canonical string form of the document.
   */
  async function sha256Hex(str) {
    const enc = new TextEncoder().encode(str);
    const hash = await crypto.subtle.digest('SHA-256', enc);
    return Array.from(new Uint8Array(hash))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
  }

  /**
   * Canonical serialisation: 2-space indentation, stable key order.
   */
  function canonicalise(doc) {
    return JSON.stringify(doc, null, 2);
  }

  return {
    SCHEMA_VERSION,
    DEFAULT_COLUMNS,
    DEFAULT_ROWS,
    generateDocId,
    nowIso,
    newDocument,
    validate,
    sha256Hex,
    canonicalise,
  };

})();
