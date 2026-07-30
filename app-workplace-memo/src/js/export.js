/**
 * Workplace*Memo — export.js
 * Assembles the final self-contained HTML document with all fonts embedded
 * as base64 data URIs. Handles print trigger and HTML file download.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * The exported HTML file is intentionally dependency-free — it contains
 * all fonts, all styles, and all content inline. No CDN links.
 * The file will render identically offline in any browser, now or in 2040.
 */

'use strict';

/* ─── @font-face declarations with embedded fonts ───────────────────────── */

/**
 * Builds @font-face CSS for all fonts used by the current template,
 * using the base64 data from font-data.js (generated at build time).
 *
 * font-data.js defines: window.WORKPLACE_FONT_DATA
 * Shape: { [familyName]: { [weight]: { [style]: 'base64string' } } }
 */
function buildFontFaceCSS() {
  const fontData = window.WORKPLACE_FONT_DATA;
  if (!fontData) {
    console.warn('[export] No font data found. Run scripts/embed-fonts.sh first.');
    return '';
  }

  let css = '';
  for (const [family, weights] of Object.entries(fontData)) {
    for (const [weight, styles] of Object.entries(weights)) {
      for (const [style, b64] of Object.entries(styles)) {
        css += `
@font-face {
  font-family: '${family}';
  font-weight: ${weight};
  font-style: ${style};
  src: url('data:font/woff2;base64,${b64}') format('woff2');
  font-display: block;
}`;
      }
    }
  }
  return css;
}

/* ─── @page rules for print geometry ────────────────────────────────────── */

function buildPageCSS() {
  const state  = window.WorkplaceEditor?.State;
  const size   = state?.pageSize || 'A4';
  const margin = state?.marginsMm || 25;

  // A4: 210mm × 297mm  |  Letter: 216mm × 279mm
  const dims = size === 'A4' ? '210mm 297mm' : '216mm 279mm';

  return `
@page {
  size: ${dims};
  margin: ${margin}mm;
}
@page :first {
  margin-top: ${margin + 8}mm;
}
/* Prevent orphan headings */
h1, h2, h3, h4 { break-after: avoid; page-break-after: avoid; }
/* Keep tables and figures together */
table, figure, pre { break-inside: avoid; page-break-inside: avoid; }
/* Force explicit page breaks */
.page-break { break-before: always; page-break-before: always; }
`;
}

/* ─── Assemble the full standalone HTML document ─────────────────────────── */

function assembleHTML() {
  const canvas      = document.getElementById('document-canvas');
  const tplKey      = window.WorkplaceEditor?.State?.templateKey || 'corporate';
  const tpl         = window.WorkplaceTemplates?.get(tplKey) || { label: 'Document', css: '' };
  const docTitle    = window.WorkplaceEditor?.State?.title || 'Document';

  const fontFaceCSS = buildFontFaceCSS();
  const pageCss     = buildPageCSS();
  const tplCss      = tpl.css || '';

  // Capture the document body content from the canvas
  // Use the raw #document-canvas innerHTML, not the Paged.js output
  const rawCanvas = document.getElementById('document-canvas');
  const bodyContent = rawCanvas.innerHTML;

  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<meta name="generator" content="Workplace Memo — PointSav Digital Systems" />
<meta name="template" content="${tpl.label}" />
<title>${escapeHtml(docTitle)}</title>
<style>
/* === Workplace*Memo — Self-contained document export ===
   Generated: ${new Date().toISOString()}
   Template:  ${tpl.label}
   Licence:   EUPL-1.2 © 2026 PointSav Digital Systems

   This file is self-contained. All fonts are embedded as base64.
   No network connection is required to render this document.
   Open in any browser. Print from any browser.
*/

/* ── Embedded fonts ── */
${fontFaceCSS}

/* ── Page geometry (print/PDF) ── */
${pageCss}

/* ── Base reset ── */
*, *::before, *::after { box-sizing: border-box; }
html { background: #f0f0f0; }

/* ── Document page styling ── */
body {
  max-width: 794px;       /* A4 width at 96dpi */
  margin: 40px auto;
  padding: 96px;          /* ~25mm margins */
  background: #ffffff;
  box-shadow: 0 2px 12px rgba(0,0,0,0.15);
}

/* ── Template styles ── */
${tplCss}

/* ── Print: remove browser decoration ── */
@media print {
  html { background: white; }
  body {
    max-width: none;
    margin: 0;
    padding: 0;
    box-shadow: none;
    background: white;
  }
  /* Ensure colour backgrounds print */
  * {
    -webkit-print-color-adjust: exact !important;
    print-color-adjust: exact !important;
  }
}
</style>
</head>
<body>
${bodyContent}
</body>
</html>`;
}

/* ─── Escape HTML for safe insertion into attributes ─────────────────────── */

function escapeHtml(str) {
  return str
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/* ─── Export HTML — triggers Tauri save dialogue ─────────────────────────── */

async function exportHTML() {
  const html  = assembleHTML();
  const title = window.WorkplaceEditor?.State?.title || 'document';
  const name  = title.replace(/[^a-zA-Z0-9\-_\s]/g, '').trim() + '.html';

  if (typeof window.__TAURI__ !== 'undefined') {
    const saved = await window.__TAURI__.core.invoke('save_file', {
      content: html,
      suggestedName: name,
    });
    if (saved) {
      console.info('[export] Saved to:', saved);
    }
  } else {
    // Browser fallback: trigger a download
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' });
    const url  = URL.createObjectURL(blob);
    const a    = document.createElement('a');
    a.href     = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
  }
}

/* ─── Print — opens the OS print dialogue ────────────────────────────────── */

/**
 * Print strategy:
 *
 * We open a new hidden window (or use window.print in Tauri) containing the
 * fully assembled HTML — with embedded fonts and @page CSS — and trigger
 * the OS print dialogue. The document the user prints is the exported version,
 * not the live editor canvas, which ensures:
 *
 * 1. All fonts are embedded (no CDN calls that might fail)
 * 2. The @page margins match the document settings exactly
 * 3. Application chrome (toolbar, ruler, etc.) never appears in print output
 *
 * On macOS 10.13, @page margin-box at-rules are not supported by WKWebKit.
 * Margins are handled via body padding in the @media print block above.
 * On Linux with WebKitGTK 2.40+, full @page support is available.
 */
function print() {
  const html = assembleHTML();

  // Open in a new window for the print dialogue
  const printWindow = window.open('', '_blank', 'width=900,height=700');
  if (!printWindow) {
    alert('Could not open print window. Check if pop-ups are blocked.');
    return;
  }

  printWindow.document.write(html);
  printWindow.document.close();

  // Wait for fonts to load from data URIs before printing
  // (data URIs are synchronous, but document.close() needs a tick)
  printWindow.addEventListener('load', () => {
    printWindow.focus();
    printWindow.print();
    // Close the print window after the dialogue is dismissed
    // Some browsers don't fire 'afterprint' reliably; use a timeout
    setTimeout(() => printWindow.close(), 1000);
  });
}

/* ─── Public API ─────────────────────────────────────────────────────────── */

window.WorkplaceExport = {
  assembleHTML,
  exportHTML,
  print,
};
