// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

(function () {
  'use strict';

  var overlay = null;
  var btn = null;

  function buildOverlay() {
    var el = document.createElement('div');
    el.id = 'ai-overlay';
    el.style.cssText = 'position:fixed;bottom:1.5rem;right:1.5rem;width:min(360px, calc(100vw - 2rem));max-height:60vh;'
      + 'background:var(--cds-layer);border:1px solid var(--cds-border-subtle);'
      + 'border-radius:var(--cds-radius-md);padding:1rem;overflow-y:auto;z-index:900;display:none;'
      + 'font-size:0.875rem;color:var(--cds-text-primary);box-shadow:var(--cds-elevation-3);';
    var header = document.createElement('div');
    header.style.cssText = 'display:flex;justify-content:space-between;align-items:center;'
      + 'margin-bottom:0.75rem;font-weight:600;';
    header.textContent = 'AI assistant';
    var close = document.createElement('button');
    close.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>';
    close.setAttribute('aria-label', 'Close');
    close.style.cssText = 'background:none;border:none;color:inherit;cursor:pointer;display:flex;'
      + 'width:var(--cds-icon-size-md);height:var(--cds-icon-size-md);padding:0;';
    close.onclick = function () { el.style.display = 'none'; };
    header.appendChild(close);
    el.appendChild(header);
    var output = document.createElement('div');
    output.id = 'ai-output';
    output.style.cssText = 'white-space:pre-wrap;line-height:1.5;';
    el.appendChild(output);
    document.body.appendChild(el);
    return el;
  }

  function getSelectionContext() {
    var sel = window.getSelection();
    if (!sel || sel.isCollapsed) return null;
    return sel.toString().trim();
  }

  function getSchemaContext() {
    var badge = document.querySelector('.schema-badge');
    return badge ? badge.textContent.trim() : '';
  }

  function getPageContext() {
    var content = document.querySelector('.content');
    return content ? content.textContent.slice(0, 800) : '';
  }

  function streamAi(selection) {
    if (!overlay) overlay = buildOverlay();
    var output = document.getElementById('ai-output');
    output.textContent = '';
    overlay.style.display = 'block';

    fetch('/ai/session', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Model': 'doorman' },
      body: JSON.stringify({
        selection: selection,
        schema: getSchemaContext(),
        context: getPageContext()
      })
    }).then(function (res) {
      if (!res.ok) { output.textContent = 'Error: ' + res.status; return; }
      var reader = res.body.getReader();
      var decoder = new TextDecoder();
      var buf = '';
      function pump() {
        reader.read().then(function (result) {
          if (result.done) return;
          buf += decoder.decode(result.value, { stream: true });
          var lines = buf.split('\n');
          buf = lines.pop();
          lines.forEach(function (line) {
            if (!line.startsWith('data:')) return;
            var json = line.slice(5).trim();
            if (!json) return;
            try {
              var msg = JSON.parse(json);
              if (msg.type === 'delta') output.textContent += msg.text;
            } catch (_) {}
          });
          pump();
        });
      }
      pump();
    }).catch(function (err) {
      output.textContent = 'Network error: ' + err.message;
    });
  }

  function injectButton() {
    btn = document.createElement('button');
    btn.id = 'ai-trigger';
    btn.textContent = 'Ask AI';
    // Bottom-left, deliberately separate from the edit/save cluster (bottom-right)
    // so the two floating-button groups never overlap on narrow viewports (P1.9).
    btn.style.cssText = 'position:fixed;bottom:1.5rem;left:1.5rem;'
      + 'padding:0.5rem 1.25rem;background:var(--cds-interactive,#4589ff);color:#fff;'
      + 'border:none;border-radius:4px;cursor:pointer;font-size:0.875rem;display:none;z-index:800;';
    btn.onclick = function () {
      var sel = getSelectionContext();
      if (!sel) return;
      streamAi(sel);
      btn.style.display = 'none';
    };
    document.body.appendChild(btn);
  }

  document.addEventListener('mouseup', function () {
    if (!btn) return;
    var sel = getSelectionContext();
    btn.style.display = sel ? 'block' : 'none';
  });

  document.addEventListener('DOMContentLoaded', injectButton);
}());
