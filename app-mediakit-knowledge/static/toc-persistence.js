/**
 * toc-persistence.js — TOC collapse/pin state persistence.
 *
 * Extracted from wiki.js (L25: route-gated bundles). Loaded on article
 * pages alongside wiki.js. Handles two distinct persistence concerns:
 *   1. TOC collapse/expand state (localStorage: 'vector-toc-expanded')
 *   2. TOC pin state (localStorage: 'wiki-toc-pinned')
 *
 * Both keys are intentionally shared with wiki.js to preserve state across
 * any upgrade. No external dependencies.
 */

'use strict';

(function () {

  var STORAGE_KEY_TOC = 'vector-toc-expanded';
  var STORAGE_KEY_PIN = 'wiki-toc-pinned';

  function initToc() {
    var toc    = document.querySelector('aside.toc');
    var toggle = document.getElementById('toc-toggle');
    var list   = document.getElementById('toc-list');
    if (!toc || !toggle || !list) return;

    var saved    = localStorage.getItem(STORAGE_KEY_TOC);
    var expanded = saved === null ? true : saved === 'true';
    applyTocState(toc, toggle, expanded);

    toggle.addEventListener('click', function () {
      if (toc.classList.contains('toc-pinned')) return;
      expanded = !expanded;
      localStorage.setItem(STORAGE_KEY_TOC, String(expanded));
      applyTocState(toc, toggle, expanded);
    });
  }

  function applyTocState(toc, toggle, expanded) {
    toggle.setAttribute('aria-expanded', String(expanded));
    toggle.textContent = expanded ? '[hide]' : '[show]';
    if (expanded) {
      toc.classList.remove('toc-collapsed');
    } else {
      toc.classList.add('toc-collapsed');
    }
  }

  function initTocPin() {
    var pinBtn = document.getElementById('toc-pin-btn');
    if (!pinBtn) return;
    var toc    = document.querySelector('aside.toc');

    var pinned = localStorage.getItem(STORAGE_KEY_PIN) === '1';
    applyPinState(toc, pinBtn, pinned);

    pinBtn.addEventListener('click', function () {
      pinned = !pinned;
      localStorage.setItem(STORAGE_KEY_PIN, pinned ? '1' : '0');
      applyPinState(toc, pinBtn, pinned);
      if (pinned) {
        toc.classList.remove('toc-collapsed');
        localStorage.setItem(STORAGE_KEY_TOC, 'true');
      }
    });
  }

  function applyPinState(toc, pinBtn, pinned) {
    toc.classList.toggle('toc-pinned', pinned);
    pinBtn.classList.toggle('toc-pin-active', pinned);
    pinBtn.setAttribute('aria-pressed', String(pinned));
    pinBtn.textContent = pinned ? '[unpin]' : '[pin]';
  }

  document.addEventListener('DOMContentLoaded', function () {
    initToc();
    initTocPin();
  });

}());
