// Mobile sidebar drawer toggle (D7).
// Injects a toggle button above the sidebar; collapses by default on narrow viewports.
(function () {
  if (window.innerWidth > 672) return;
  var sidebar = document.querySelector('nav.sidebar');
  if (!sidebar) return;
  var btn = document.createElement('button');
  btn.className = 'drawer-toggle';
  btn.textContent = '☰ Navigation';
  btn.setAttribute('aria-label', 'Toggle navigation');
  sidebar.classList.add('drawer-collapsed');
  sidebar.parentNode.insertBefore(btn, sidebar);
  btn.addEventListener('click', function () {
    var collapsed = sidebar.classList.toggle('drawer-collapsed');
    btn.textContent = collapsed ? '☰ Navigation' : '✕ Close';
  });
})();
