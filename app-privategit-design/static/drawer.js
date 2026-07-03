// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Mobile sidebar drawer toggle (D7).
// Injects a toggle button above the sidebar on narrow viewports; re-evaluates on
// resize/orientationchange instead of only once at load (P1.6). Wires aria-expanded
// + aria-hidden so the collapsed nav leaves the tab order for keyboard/AT users (P1.7).
(function () {
  var BREAKPOINT = 768;
  var sidebar = document.querySelector('nav.sidebar');
  if (!sidebar) return;
  var btn = null;

  function setCollapsed(collapsed) {
    sidebar.classList.toggle('drawer-collapsed', collapsed);
    sidebar.setAttribute('aria-hidden', collapsed ? 'true' : 'false');
    if (btn) {
      btn.setAttribute('aria-expanded', collapsed ? 'false' : 'true');
      btn.textContent = collapsed ? '☰ Navigation' : '✕ Close';
    }
  }

  function ensureToggle() {
    if (btn) return;
    btn = document.createElement('button');
    btn.className = 'drawer-toggle';
    btn.setAttribute('aria-label', 'Toggle navigation');
    btn.setAttribute('aria-controls', 'nav-sidebar');
    sidebar.id = sidebar.id || 'nav-sidebar';
    sidebar.parentNode.insertBefore(btn, sidebar);
    btn.addEventListener('click', function () {
      setCollapsed(!sidebar.classList.contains('drawer-collapsed'));
    });
    setCollapsed(true);
  }

  function removeToggle() {
    if (!btn) return;
    btn.remove();
    btn = null;
    sidebar.classList.remove('drawer-collapsed');
    sidebar.removeAttribute('aria-hidden');
  }

  function evaluate() {
    if (window.innerWidth <= BREAKPOINT) ensureToggle();
    else removeToggle();
  }

  evaluate();
  window.addEventListener('resize', evaluate);
  window.addEventListener('orientationchange', evaluate);
})();
