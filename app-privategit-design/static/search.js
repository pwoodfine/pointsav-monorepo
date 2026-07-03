// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// P1.2 — header search autocomplete wired to the existing /tokens/search endpoint.
(function () {
  var input = document.getElementById('header-search-q');
  var dropdown = document.getElementById('search-autocomplete-dropdown');
  if (!input || !dropdown) return;

  var debounceTimer = null;

  function render(results) {
    if (!results.length) {
      dropdown.innerHTML = '';
      dropdown.classList.remove('open');
      return;
    }
    dropdown.innerHTML = results.map(function (r) {
      return '<a href="' + escapeAttr(r.url) + '">' +
        '<div class="sd-title">' + escapeHtml(r.title) + '</div>' +
        '<div class="sd-snippet">' + escapeHtml(r.snippet) + '</div>' +
        '</a>';
    }).join('');
    dropdown.classList.add('open');
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>]/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c];
    });
  }
  function escapeAttr(s) { return escapeHtml(s).replace(/"/g, '&quot;'); }

  input.addEventListener('input', function () {
    var q = input.value.trim();
    clearTimeout(debounceTimer);
    if (!q) { render([]); return; }
    debounceTimer = setTimeout(function () {
      fetch('/tokens/search?q=' + encodeURIComponent(q))
        .then(function (res) { return res.ok ? res.json() : []; })
        .then(render)
        .catch(function () { render([]); });
    }, 200);
  });

  document.addEventListener('click', function (e) {
    if (!dropdown.contains(e.target) && e.target !== input) {
      dropdown.classList.remove('open');
    }
  });
})();
