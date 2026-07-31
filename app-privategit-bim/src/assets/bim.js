// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// bim.js — partial-page navigation, SSE hot-reload, SchemaState
// Hand-written; no HTMX dependency.

// ── Navigation ─────────────────────────────────────────────────────────────

function getMain() {
  return document.getElementById('bim-main-content');
}

async function navigate(path) {
  try {
    const res = await fetch('/fragment' + path, {
      headers: { 'X-Fragment': '1' },
    });
    if (!res.ok) {
      // Not every route has a /fragment/* counterpart (e.g. home, /about,
      // /disclaimers, /search — only tokens/tokens-detail/research do).
      // A missing fragment used to silently do nothing on click; fall back
      // to a real navigation instead, same as the network-failure path
      // below.
      window.location.href = path;
      return;
    }
    const html = await res.text();
    const main = getMain();
    if (main) {
      main.innerHTML = html;
      // Re-run any module scripts in the new content
      main.querySelectorAll('script[type="module"]').forEach((old) => {
        const next = document.createElement('script');
        next.type = 'module';
        if (old.src) {
          next.src = old.src;
        } else {
          next.textContent = old.textContent;
        }
        old.replaceWith(next);
      });
    }
    history.pushState({}, '', path);
    syncActiveNav(path);
  } catch (_) {
    // Let normal navigation proceed on network failure
    window.location.href = path;
  }
}

function syncActiveNav(path) {
  document.querySelectorAll('[data-path]').forEach((el) => {
    const match = el.dataset.path === path;
    el.setAttribute('aria-current', match ? 'page' : 'false');
    el.closest('[aria-current]')?.setAttribute('aria-current', match ? 'page' : 'false');
  });
}

// Intercept clicks on any element with data-path or .bim-nav-link
document.addEventListener('click', (e) => {
  const link = e.target.closest('[data-path], .bim-nav-link[href]');
  if (!link) return;
  const path = link.dataset.path || link.getAttribute('href');
  if (!path || path.startsWith('http') || path.startsWith('//')) return;
  // Only intercept same-origin paths that start with /
  if (!path.startsWith('/')) return;
  // Skip download and external links
  if (path.includes('/download/') || path.includes('.zip') || path.includes('.ifc')) return;
  e.preventDefault();
  navigate(path);
});

// Browser back/forward
window.addEventListener('popstate', () => {
  navigate(location.pathname);
});

// ── Envelope jurisdiction-overlay toggle ────────────────────────────────────
// Homepage envelope diagram: switches which pre-rendered overlay-state frame
// is visible (municipal / +provincial / +accessibility). Each frame is a
// complete SVG (render::envelope generates one per state) rather than one
// shared SVG with toggled sub-groups, since the tiers' shapes genuinely
// differ between states, not just their visibility.

document.addEventListener('click', (e) => {
  const btn = e.target.closest('.bim-envelope__overlay-btn');
  if (!btn) return;
  const key = btn.dataset.overlayTarget;
  const envelope = btn.closest('.bim-envelope');
  if (!envelope) return;
  envelope.setAttribute('data-active-overlay', key);
  envelope.querySelectorAll('.bim-envelope__frame').forEach((frame) => {
    frame.hidden = frame.dataset.overlay !== key;
  });
  envelope.querySelectorAll('.bim-envelope__overlay-btn').forEach((b) => {
    b.setAttribute('aria-pressed', b === btn ? 'true' : 'false');
  });
});

// ── Theme toggle ──────────────────────────────────────────────────────────

function syncThemeControls(theme) {
  document.querySelectorAll('.bim-theme-toggle').forEach((btn) => {
    btn.setAttribute('aria-pressed', theme === 'dark' ? 'true' : 'false');
    btn.setAttribute('aria-label', theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme');
  });
}

document.addEventListener('click', (e) => {
  const toggle = e.target.closest('.bim-theme-toggle');
  if (!toggle || toggle.disabled) return;
  const current = document.documentElement.getAttribute('data-theme') === 'dark' ? 'dark' : 'light';
  const next = current === 'dark' ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', next);
  try { localStorage.setItem('bim-theme', next); } catch (_) {}
  syncThemeControls(next);
});

syncThemeControls(document.documentElement.getAttribute('data-theme') === 'dark' ? 'dark' : 'light');

// ── Force Important Information open when printing ─────────────────────────
// A CSS-only `display: block !important` on the closed <details> body is not
// reliable across Chromium's print-to-PDF path (verified: getComputedStyle
// reports the body as visible/non-zero-height under print media, but the
// actual PDF output omits it) — so open the element for real via the DOM
// attribute, which every engine honors, and restore whatever state the
// reader had it in afterward.
let disclosureWasOpen = null;
window.addEventListener('beforeprint', () => {
  const details = document.querySelector('.bim-disclosure__details');
  if (!details) return;
  disclosureWasOpen = details.open;
  details.open = true;
});
window.addEventListener('afterprint', () => {
  const details = document.querySelector('.bim-disclosure__details');
  if (!details || disclosureWasOpen === null) return;
  details.open = disclosureWasOpen;
  disclosureWasOpen = null;
});

// ── SSE hot-reload ──────────────────────────────────────────────────────────

let sseRetries = 0;

function connectSSE() {
  const evs = new EventSource('/api/events');
  evs.onopen = () => { sseRetries = 0; };
  evs.onmessage = (e) => {
    try {
      const msg = JSON.parse(e.data);
      if (msg.event === 'token-updated') {
        // Re-fetch current fragment to show updated content
        navigate(location.pathname);
      }
    } catch (_) {}
  };
  evs.onerror = () => {
    evs.close();
    sseRetries++;
    const delay = Math.min(1000 * Math.pow(2, sseRetries), 30000);
    setTimeout(connectSSE, delay);
  };
}

connectSSE();

// ── SchemaState ─────────────────────────────────────────────────────────────
// Bidirectional state between the visual pane and the CodeMirror code pane.

const SchemaState = {
  data: {},
  _listeners: [],

  get(path) {
    return path.split('.').reduce((o, k) => (o != null ? o[k] : undefined), this.data);
  },

  set(path, value) {
    const parts = path.split('.');
    const last = parts.pop();
    const target = parts.reduce((o, k) => (o[k] = o[k] || {}), this.data);
    target[last] = value;
    this._notify();
  },

  replace(obj) {
    this.data = obj;
    this._notify();
  },

  subscribe(fn) {
    this._listeners.push(fn);
    return () => {
      this._listeners = this._listeners.filter((f) => f !== fn);
    };
  },

  _notify() {
    this._listeners.forEach((fn) => fn(this.data));
  },
};

// Expose globally for inline editor scripts
window.SchemaState = SchemaState;
window.BimNavigate = navigate;
