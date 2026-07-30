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
    if (!res.ok) return;
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
