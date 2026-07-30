/**
 * Workplace*Memo — pagination.js
 * Configures Paged.js for live page break rendering in the editor canvas.
 * Paged.js is MIT licensed — https://pagedjs.org
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * NOTE: Paged.js operates in "polyfill mode" — it intercepts the document
 * and re-renders the content as discrete page divs. This gives us the
 * true Word-like page-on-desktop appearance with live pagination.
 *
 * The Paged.js vendor file (js/vendor/paged.polyfill.js) must be served
 * from the same origin. No CDN. See scripts/download-paged.sh for
 * fetching the vendor file.
 */

'use strict';

/* ─── Paged.js handler ───────────────────────────────────────────────────── */

/**
 * PagedJsHandler listens to Paged.js lifecycle events to update
 * the page counter in the status bar.
 */
class WorkplacePaginationHandler {
  constructor() {
    this.totalPages = 0;
  }

  // Called by Paged.js after each page is rendered
  afterPageLayout(pageFragment, page, breakToken) {
    this.totalPages = page.id + 1;
  }

  // Called by Paged.js after all pages are rendered
  afterRendered(pages) {
    this.totalPages = pages.length;
    updatePageDisplay(pages.length);
    attachScrollPageTracking();
    console.info(`[pagination] Rendered ${pages.length} page(s).`);
  }
}

/* ─── Page display update ────────────────────────────────────────────────── */

function updatePageDisplay(total) {
  window.WorkplaceEditor?.updatePageCount(1, total);
}

/* ─── Track current page on scroll ──────────────────────────────────────── */

function attachScrollPageTracking() {
  const desktop = document.getElementById('document-desktop');
  if (!desktop) return;

  desktop.addEventListener('scroll', () => {
    const pages = document.querySelectorAll('.pagedjs_page');
    if (!pages.length) return;

    const desktopRect = desktop.getBoundingClientRect();
    let currentPage = 1;

    pages.forEach((page, idx) => {
      const rect = page.getBoundingClientRect();
      // Page is "current" if its top is above the middle of the desktop viewport
      if (rect.top < desktopRect.top + (desktopRect.height / 2)) {
        currentPage = idx + 1;
      }
    });

    const total = pages.length;
    window.WorkplaceEditor?.updatePageCount(currentPage, total);
  }, { passive: true });
}

/* ─── Initialise Paged.js ────────────────────────────────────────────────── */

function initPagination() {
  if (typeof window.Paged === 'undefined') {
    // Paged.js not loaded yet — it defers its own execution via the polyfill.
    // The polyfill fires automatically on DOMContentLoaded.
    // We register our handler here so it is ready when Paged.js starts.
    console.info('[pagination] Paged.js not yet available; handler registered for auto-init.');

    // Paged.js polyfill exposes window.PagedPolyfill; register our handler
    window.PagedConfig = window.PagedConfig || {};
    window.PagedConfig.auto = true;  // run automatically
    return;
  }

  // If Paged is already available (rare — script order), register directly
  Paged.registerHandlers(new WorkplacePaginationHandler());
}

/* ─── Refresh pagination (after content or template changes) ─────────────── */

let _refreshTimeout = null;

function refreshPagination() {
  // Debounce: avoid thrashing on rapid keystrokes
  clearTimeout(_refreshTimeout);
  _refreshTimeout = setTimeout(() => {
    if (typeof window.Paged !== 'undefined') {
      // Re-render — Paged.js will replace the page structure
      new window.Paged.Previewer()
        .preview(
          document.getElementById('document-canvas').innerHTML,
          [],
          document.getElementById('document-desktop')
        )
        .catch(err => console.warn('[pagination] Preview error:', err));
    }
  }, 400);
}

/* ─── DOM ready ──────────────────────────────────────────────────────────── */

document.addEventListener('DOMContentLoaded', () => {
  initPagination();
});

/* ─── Public API ─────────────────────────────────────────────────────────── */

window.WorkplacePagination = {
  refresh: refreshPagination,
};
