/**
 * wiki.js — Phase 3 merged interaction layer.
 *
 * Loaded on every page (article, home, category, search). The wiki is a
 * read-only viewer: there is no in-browser editor — contributions flow through
 * git, and the Edit tab links to the raw Markdown source at /git/{slug}.
 *
 * Responsibilities:
 *   1.  TOC scroll-spy (IntersectionObserver; CSS .toc-entry.active)
 *   2.  TOC pin/unpin (localStorage)
 *   3.  Cmd+K command palette (fetch /api/complete; dialog open/close)
 *   4.  Three-way theme toggle (light / dark / system; localStorage + html[data-theme])
 *   5.  Citation hover cards (.citation-marker + data-citation-id → /api/citations)
 *   6.  TOC collapse toggle
 *   7.  Page hover-card previews (wikilinks)
 *   8.  Glossary tooltips
 *   9.  Mobile nav + TOC drawers
 *  10.  Footnote hover tooltips
 *  11.  Search autocomplete
 *  12.  Navbox autocollapse
 *  13.  Mobile collapsible h2 sections
 *  14.  Keyboard shortcuts ('?' overlay)
 *  15.  AJAX page navigation (fetch + DOM swap + pushState)
 *
 * No external dependencies. No module bundler. Loaded with `defer`.
 * Bloomberg article standard: plain English, no marketing copy.
 */

'use strict';

(function () {

  /* ------------------------------------------------------------------ *
   * Module-level state shared across idempotent inits                   *
   * ------------------------------------------------------------------ */

  var _sectionObserver = null;  // IntersectionObserver for active TOC tracking
  var _hoverCard  = null;       // hover-card element (created once, reused)
  var _hoverTimer = null;       // hide timeout
  var _hoverTarget = null;      // currently hovered link
  var _hoverCache = {};         // slug → preview data
  var _glossaryTip = null;      // glossary tooltip element
  var _fnTip = null;            // footnote tooltip element

  function escHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  /* ------------------------------------------------------------------ *
   * 1. TOC collapse toggle                                               *
   * ------------------------------------------------------------------ */

  var STORAGE_KEY_TOC = 'vector-toc-expanded';

  function initToc() {
    var toc    = document.querySelector('aside.toc');
    var toggle = document.getElementById('toc-toggle');
    var list   = document.getElementById('toc-list');
    if (!toc || !toggle || !list) return;

    var saved    = localStorage.getItem(STORAGE_KEY_TOC);
    var expanded = saved === null ? true : saved === 'true';
    applyTocState(toc, toggle, expanded);

    toggle.addEventListener('click', function () {
      if (toc.classList.contains('toc-pinned')) return; // pinned: ignore hide request
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

  /* ------------------------------------------------------------------ *
   * 2. TOC pin button                                                    *
   *                                                                     *
   * Pinned = TOC always shows regardless of the hide/show preference.   *
   * State persisted to localStorage under 'wiki-toc-pinned'.            *
   * ------------------------------------------------------------------ */

  var STORAGE_KEY_PIN = 'wiki-toc-pinned';

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

  /* ------------------------------------------------------------------ *
   * 3. Reader density toggle                                             *
   *                                                                     *
   * Three states:                                                        *
   *   off        — no IVC marks (pure reading experience)               *
   *   exceptions — neutral marks visible; coloured marks prominent      *
   *                (default; operationalises the TLS-padlock lesson)    *
   *   all        — shows verified marks too; for auditors, power-users  *
   *                                                                     *
   * Phase 7 reads this key and applies the appropriate body class.      *
   * ------------------------------------------------------------------ */

  var STORAGE_KEY_DENSITY = 'wiki-citation-density';
  var DENSITY_DEFAULT     = 'exceptions';

  function initDensityToggle() {
    var btnOff        = document.getElementById('density-off');
    var btnExceptions = document.getElementById('density-exceptions');
    var btnAll        = document.getElementById('density-all');
    if (!btnOff || !btnExceptions || !btnAll) return;

    var current = localStorage.getItem(STORAGE_KEY_DENSITY) || DENSITY_DEFAULT;
    applyDensity(current, btnOff, btnExceptions, btnAll);

    btnOff.addEventListener('click', function () {
      setDensity('off', btnOff, btnExceptions, btnAll);
    });
    btnExceptions.addEventListener('click', function () {
      setDensity('exceptions', btnOff, btnExceptions, btnAll);
    });
    btnAll.addEventListener('click', function () {
      setDensity('all', btnOff, btnExceptions, btnAll);
    });
  }

  function setDensity(value, btnOff, btnExceptions, btnAll) {
    localStorage.setItem(STORAGE_KEY_DENSITY, value);
    applyDensity(value, btnOff, btnExceptions, btnAll);
    // Phase 7: document.body.dataset.citationDensity = value;
  }

  function applyDensity(value, btnOff, btnExceptions, btnAll) {
    var ACTIVE = 'density-btn-active';
    btnOff.classList.toggle(ACTIVE, value === 'off');
    btnExceptions.classList.toggle(ACTIVE, value === 'exceptions');
    btnAll.classList.toggle(ACTIVE, value === 'all');
  }

  /* ------------------------------------------------------------------ *
   * 4. Page Previews (Hover Cards)                                       *
   *                                                                     *
   * Idempotent: the card element is created once and reused after       *
   * AJAX navigations. New content links are re-bound on each call.      *
   * ------------------------------------------------------------------ */

  function initHoverCards() {
    if (!_hoverCard) {
      _hoverCard = document.createElement('div');
      _hoverCard.className = 'wiki-hover-card';
      _hoverCard.style.display = 'none';
      document.body.appendChild(_hoverCard);
      _hoverCard.addEventListener('mouseenter', function () {
        clearTimeout(_hoverTimer);
      });
      _hoverCard.addEventListener('mouseleave', function () {
        _hoverTimer = setTimeout(function () {
          _hoverCard.style.display = 'none';
          _hoverTarget = null;
        }, 200);
      });
    }

    var links = document.querySelectorAll('a[data-wikilink="true"]:not(.wiki-redlink)');
    links.forEach(function (link) {
      link.addEventListener('mouseenter', function () {
        clearTimeout(_hoverTimer);
        var href = link.getAttribute('href');
        if (!href || !href.startsWith('/wiki/')) return;
        var slug = href.substring(6);
        _hoverTarget = link;

        var rect = link.getBoundingClientRect();
        _hoverCard.style.left = Math.max(10, rect.left + window.scrollX) + 'px';
        _hoverCard.style.top  = rect.bottom + window.scrollY + 5 + 'px';

        if (_hoverCache[slug]) {
          renderHoverCard(_hoverCache[slug]);
        } else {
          _hoverCard.innerHTML = '<div class="hover-loading">Loading…</div>';
          _hoverCard.style.display = 'block';
          fetch('/api/preview/' + slug)
            .then(function (r) { return r.json(); })
            .then(function (data) {
              _hoverCache[slug] = data;
              if (_hoverTarget === link) renderHoverCard(data);
            })
            .catch(function () { _hoverCard.style.display = 'none'; });
        }
      });

      link.addEventListener('mouseleave', function () {
        _hoverTimer = setTimeout(function () {
          _hoverCard.style.display = 'none';
          _hoverTarget = null;
        }, 200);
      });
    });
  }

  function renderHoverCard(data) {
    var snip = data.snippet || 'No summary available.';
    var imgHtml = data.image_url
      ? '<img src="' + data.image_url + '" alt="">'
      : '';
    var slug = data.slug || '';
    var moreHtml = slug
      ? '<a class="hover-card-more" href="/wiki/' + escHtml(slug) + '">Read more →</a>'
      : '';
    _hoverCard.innerHTML = imgHtml + '<strong>' + escHtml(data.title) + '</strong><p>' + escHtml(snip) + '</p>' + moreHtml;
    _hoverCard.style.display = 'block';
  }

  /* ------------------------------------------------------------------ *
   * 5. Glossary Tooltips                                                 *
   *                                                                     *
   * Idempotent: the tooltip element is created once and reused.         *
   * ------------------------------------------------------------------ */

  function initGlossaryTooltips() {
    if (!_glossaryTip) {
      _glossaryTip = document.createElement('div');
      _glossaryTip.className = 'wiki-glossary-tooltip';
      _glossaryTip.style.display = 'none';
      document.body.appendChild(_glossaryTip);
    }

    var terms = document.querySelectorAll('.wiki-glossary-term');
    terms.forEach(function (term) {
      term.addEventListener('mouseenter', function () {
        var defn = term.getAttribute('title');
        if (!defn) return;
        term.dataset.title = defn;
        term.removeAttribute('title');
        _glossaryTip.textContent = defn;
        _glossaryTip.style.display = 'block';
        var rect = term.getBoundingClientRect();
        _glossaryTip.style.left = rect.left + window.scrollX + 'px';
        // Flip below term when near viewport top to avoid clipping.
        if (rect.top < 110) {
          _glossaryTip.style.top = (rect.bottom + window.scrollY + 5) + 'px';
        } else {
          _glossaryTip.style.top = (rect.top + window.scrollY - _glossaryTip.offsetHeight - 5) + 'px';
        }
      });
      term.addEventListener('mouseleave', function () {
        _glossaryTip.style.display = 'none';
        if (term.dataset.title) term.setAttribute('title', term.dataset.title);
      });
    });
  }

  /* ------------------------------------------------------------------ *
   * 6. Mobile nav drawer toggle                                          *
   * ------------------------------------------------------------------ */

  // Trap keyboard focus inside an open drawer (WCAG 2.1 §2.1.2).
  function trapFocus(drawer) {
    drawer.addEventListener('keydown', function (e) {
      if (e.key !== 'Tab') return;
      var focusable = Array.from(drawer.querySelectorAll(
        'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])'
      ));
      if (!focusable.length) return;
      var first = focusable[0];
      var last  = focusable[focusable.length - 1];
      if (e.shiftKey) {
        if (document.activeElement === first) { e.preventDefault(); last.focus(); }
      } else {
        if (document.activeElement === last)  { e.preventDefault(); first.focus(); }
      }
    });
  }

  function initMobileNav() {
    var btn      = document.getElementById('nav-toggle');
    var drawer   = document.getElementById('mobile-nav-drawer');
    var overlay  = document.getElementById('mobile-nav-overlay');
    var closeBtn = document.getElementById('mobile-nav-close');
    if (!btn || !drawer || !overlay) return;

    function openNav() {
      document.body.removeAttribute('data-toc-open');
      var tocDrawer = document.getElementById('mobile-toc-drawer');
      var tocBtn    = document.getElementById('toc-toggle-btn');
      if (tocDrawer) tocDrawer.setAttribute('aria-hidden', 'true');
      if (tocBtn)    tocBtn.setAttribute('aria-expanded', 'false');
      document.body.setAttribute('data-nav-open', 'true');
      drawer.removeAttribute('aria-hidden');
      overlay.removeAttribute('aria-hidden');
      btn.setAttribute('aria-expanded', 'true');
      if (closeBtn) closeBtn.focus();
    }

    function closeNav() {
      document.body.removeAttribute('data-nav-open');
      drawer.setAttribute('aria-hidden', 'true');
      overlay.setAttribute('aria-hidden', 'true');
      btn.setAttribute('aria-expanded', 'false');
      btn.focus();
    }

    btn.addEventListener('click', openNav);
    overlay.addEventListener('click', closeNav);
    if (closeBtn) closeBtn.addEventListener('click', closeNav);

    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && document.body.hasAttribute('data-nav-open')) closeNav();
    });

    // Close when a nav link is followed (matches TOC drawer behavior).
    drawer.querySelectorAll('a').forEach(function (link) {
      link.addEventListener('click', closeNav);
    });

    trapFocus(drawer);
  }

  /* ------------------------------------------------------------------ *
   * 7. Mobile ToC drawer toggle                                          *
   * ------------------------------------------------------------------ */

  function initTocDrawer() {
    var btn      = document.getElementById('toc-toggle-btn');
    var drawer   = document.getElementById('mobile-toc-drawer');
    var overlay  = document.getElementById('mobile-nav-overlay');
    var closeBtn = document.getElementById('mobile-toc-close');
    if (!btn || !drawer || !overlay) return;

    function openToc() {
      document.body.removeAttribute('data-nav-open');
      var navDrawer = document.getElementById('mobile-nav-drawer');
      var navBtn    = document.getElementById('nav-toggle');
      if (navDrawer) navDrawer.setAttribute('aria-hidden', 'true');
      if (navBtn)    navBtn.setAttribute('aria-expanded', 'false');
      document.body.setAttribute('data-toc-open', 'true');
      drawer.removeAttribute('aria-hidden');
      overlay.removeAttribute('aria-hidden');
      btn.setAttribute('aria-expanded', 'true');
      if (closeBtn) closeBtn.focus();
    }

    function closeToc() {
      document.body.removeAttribute('data-toc-open');
      drawer.setAttribute('aria-hidden', 'true');
      overlay.setAttribute('aria-hidden', 'true');
      btn.setAttribute('aria-expanded', 'false');
      btn.focus();
    }

    btn.addEventListener('click', openToc);
    overlay.addEventListener('click', closeToc);
    if (closeBtn) closeBtn.addEventListener('click', closeToc);

    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && document.body.hasAttribute('data-toc-open')) closeToc();
    });

    drawer.querySelectorAll('a').forEach(function (link) {
      link.addEventListener('click', closeToc);
    });

    trapFocus(drawer);
  }

  /* ------------------------------------------------------------------ *
   * 8. Footnote hover tooltips                                           *
   *                                                                     *
   * Idempotent: the tooltip element is created once and reused.         *
   * ------------------------------------------------------------------ */

  function initFootnoteTooltips() {
    if (!_fnTip) {
      _fnTip = document.createElement('div');
      _fnTip.className = 'fn-tooltip';
      _fnTip.setAttribute('aria-hidden', 'true');
      _fnTip.style.display = 'none';
      document.body.appendChild(_fnTip);
    }

    var refs = document.querySelectorAll('sup.footnote-ref a');
    if (!refs.length) return;

    function getFootnoteText(href) {
      var id = href.replace(/^#/, '');
      var li = document.getElementById(id);
      if (!li) return null;
      var clone   = li.cloneNode(true);
      var backref = clone.querySelector('.footnote-backref');
      if (backref) backref.remove();
      return clone.textContent.trim();
    }

    function showTip(anchor, text) {
      _fnTip.textContent = text;
      _fnTip.style.display = 'block';
      var rect = anchor.getBoundingClientRect();
      var top  = window.scrollY + rect.bottom + 6;
      var left = Math.min(window.scrollX + rect.left, window.innerWidth - 400);
      _fnTip.style.top  = top + 'px';
      _fnTip.style.left = Math.max(8, left) + 'px';
    }

    refs.forEach(function (anchor) {
      anchor.addEventListener('mouseenter', function () {
        var text = getFootnoteText(anchor.getAttribute('href') || '');
        if (text) showTip(anchor, text);
      });
      anchor.addEventListener('mouseleave', function () { _fnTip.style.display = 'none'; });
      anchor.addEventListener('focus',      function () {
        var text = getFootnoteText(anchor.getAttribute('href') || '');
        if (text) showTip(anchor, text);
      });
      anchor.addEventListener('blur',       function () { _fnTip.style.display = 'none'; });
    });
  }

  /* ------------------------------------------------------------------ *
   * 9. Search autocomplete                                               *
   * Debounced dropdown fed by GET /api/complete?q={prefix}.             *
   * ------------------------------------------------------------------ */

  function initSearchAutocomplete() {
    var input    = document.getElementById('header-search-q');
    var dropdown = document.getElementById('search-autocomplete-dropdown');
    if (!input || !dropdown) return;

    var debounceTimer = null;
    var activeIdx     = -1;
    var items         = [];

    function hideDropdown() {
      dropdown.style.display = 'none';
      dropdown.innerHTML = '';
      items     = [];
      activeIdx = -1;
    }

    function renderDropdown(hits) {
      dropdown.innerHTML = '';
      items     = hits;
      activeIdx = -1;
      if (!hits.length) { hideDropdown(); return; }
      hits.forEach(function (hit) {
        var li = document.createElement('a');
        li.className = 'ac-item';
        li.href = '/wiki/' + hit.slug;
        var slugParts = hit.slug.split('/');
        var contentType = slugParts.length > 1 ? slugParts[0] : '';
        var typeMap = { 'how-to': 'Guide', 'guides': 'Guide', 'reference': 'Reference',
                        'applications': 'Topic', 'patterns': 'Topic', 'substrate': 'Topic',
                        'design-system': 'Topic', 'services': 'Topic', 'governance': 'Topic' };
        var typeLabel = typeMap[contentType] || '';
        li.innerHTML =
          '<span class="ac-title">' + escHtml(hit.title) + '</span>' +
          (typeLabel ? '<span class="ac-type">' + escHtml(typeLabel) + '</span>' : '') +
          (hit.lede ? '<span class="ac-lede">' + escHtml(hit.lede.slice(0, 90)) + '…</span>' : '');
        li.addEventListener('mousedown', function (e) {
          e.preventDefault();
          window.location.href = '/wiki/' + hit.slug;
        });
        dropdown.appendChild(li);
      });
      dropdown.style.display = 'block';
    }

    function escHtml(s) {
      return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
    }

    input.addEventListener('input', function () {
      clearTimeout(debounceTimer);
      var q = input.value.trim();
      if (q.length < 2) { hideDropdown(); return; }
      debounceTimer = setTimeout(function () {
        fetch('/api/complete?q=' + encodeURIComponent(q))
          .then(function (r) { return r.json(); })
          .then(renderDropdown)
          .catch(function () { hideDropdown(); });
      }, 200);
    });

    input.addEventListener('keydown', function (e) {
      if (!items.length) return;
      var rows = dropdown.querySelectorAll('.ac-item');
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        activeIdx = Math.min(activeIdx + 1, rows.length - 1);
        rows.forEach(function (r, i) { r.classList.toggle('ac-active', i === activeIdx); });
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        activeIdx = Math.max(activeIdx - 1, 0);
        rows.forEach(function (r, i) { r.classList.toggle('ac-active', i === activeIdx); });
      } else if (e.key === 'Enter' && activeIdx >= 0) {
        e.preventDefault();
        window.location.href = '/wiki/' + items[activeIdx].slug;
      } else if (e.key === 'Escape') {
        hideDropdown();
      }
    });

    document.addEventListener('click', function (e) {
      if (!dropdown.contains(e.target) && e.target !== input) hideDropdown();
    });
  }

  /* ------------------------------------------------------------------ *
   * 10. Navbox autocollapse                                              *
   * ------------------------------------------------------------------ */

  function initNavboxes() {
    var navboxes = document.querySelectorAll('.navbox');
    navboxes.forEach(function (nb) {
      var title = nb.querySelector('.navbox-title');
      if (!title) return;
      title.addEventListener('click', function () {
        nb.classList.toggle('navbox-collapsed');
      });
    });
  }

  /* ------------------------------------------------------------------ *
   * 11. Mobile collapsible h2 sections                                   *
   *                                                                     *
   * At <960px each h2 in the article body acts as a toggle for the      *
   * content following it. State is persisted per slug in localStorage.  *
   * After AJAX navigation the content DOM is replaced, so this can be   *
   * safely called again on the new content.                             *
   * ------------------------------------------------------------------ */

  function initCollapsibleSections() {
    if (window.innerWidth >= 960) return;

    var article = document.querySelector('#mw-content-text, .mw-body');
    if (!article) return;

    var slug = (document.querySelector('link[rel="canonical"]') || {}).href
      || window.location.pathname;
    var storageKey  = 'wiki-sections:' + slug;
    var openSections = {};
    try {
      var stored = localStorage.getItem(storageKey);
      if (stored) openSections = JSON.parse(stored);
    } catch (e) { openSections = {}; }

    var h2s = article.querySelectorAll('h2');
    if (!h2s.length) return;

    h2s.forEach(function (h2) {
      if (h2.classList.contains('section-toggle')) return; // already processed
      var id     = h2.id || h2.textContent.trim().replace(/\s+/g, '-').toLowerCase();
      var isOpen = openSections[id] !== false;

      var siblings = [];
      var next = h2.nextElementSibling;
      while (next && next.tagName !== 'H2') {
        siblings.push(next);
        next = next.nextElementSibling;
      }
      if (!siblings.length) return;

      var wrapper = document.createElement('div');
      wrapper.className = 'section-body';
      h2.parentNode.insertBefore(wrapper, siblings[0]);
      siblings.forEach(function (s) { wrapper.appendChild(s); });

      h2.classList.add('section-toggle');
      h2.setAttribute('aria-expanded', String(isOpen));
      if (!isOpen) {
        wrapper.style.display = 'none';
        h2.classList.add('section-collapsed');
      }

      h2.addEventListener('click', function () {
        isOpen = !isOpen;
        h2.setAttribute('aria-expanded', String(isOpen));
        h2.classList.toggle('section-collapsed', !isOpen);
        wrapper.style.display = isOpen ? '' : 'none';
        openSections[id] = isOpen;
        try { localStorage.setItem(storageKey, JSON.stringify(openSections)); } catch (e) {}
      });
    });
  }

  /* ------------------------------------------------------------------ *
   * 12a. Appearance menu (theme + width toggle)                         *
   * ------------------------------------------------------------------ */

  function initAppearanceMenu() {
    var btn  = document.getElementById('wiki-appearance-btn');
    var menu = document.getElementById('wiki-appearance-menu');
    if (!btn || !menu) return;

    function applyTheme(t) {
      document.documentElement.setAttribute('data-theme', t);
      try { localStorage.setItem('wiki-theme', t); } catch(e) {}
      document.querySelectorAll('#wiki-theme-options .wiki-appearance-opt').forEach(function(el) {
        el.classList.toggle('appearance-active', el.getAttribute('data-theme-val') === t);
      });
    }

    function applyWidth(w) {
      document.documentElement.setAttribute('data-width', w);
      try { localStorage.setItem('wiki-width', w); } catch(e) {}
      document.querySelectorAll('#wiki-width-options .wiki-appearance-opt').forEach(function(el) {
        el.classList.toggle('appearance-active', el.getAttribute('data-width-val') === w);
      });
    }

    // Reflect stored state on open.
    function reflectState() {
      var t = document.documentElement.getAttribute('data-theme') || 'auto';
      var w = document.documentElement.getAttribute('data-width') || 'standard';
      applyTheme(t);
      applyWidth(w);
    }

    btn.addEventListener('click', function(e) {
      e.stopPropagation();
      var open = btn.getAttribute('aria-expanded') === 'true';
      btn.setAttribute('aria-expanded', String(!open));
      if (!open) {
        menu.removeAttribute('hidden');
        reflectState();
      } else {
        menu.setAttribute('hidden', '');
      }
    });

    menu.addEventListener('click', function(e) {
      var el = e.target;
      var tv = el.getAttribute('data-theme-val');
      var wv = el.getAttribute('data-width-val');
      if (tv) applyTheme(tv);
      if (wv) applyWidth(wv);
    });

    document.addEventListener('click', function(e) {
      if (!menu.hasAttribute('hidden') && !menu.contains(e.target) && e.target !== btn) {
        menu.setAttribute('hidden', '');
        btn.setAttribute('aria-expanded', 'false');
      }
    });

    document.addEventListener('keydown', function(e) {
      if (e.key === 'Escape' && !menu.hasAttribute('hidden')) {
        menu.setAttribute('hidden', '');
        btn.setAttribute('aria-expanded', 'false');
        btn.focus();
      }
    });

    // Apply stored preferences immediately (anti-FOUT inline script did this
    // before paint; here we also mark the active buttons).
    reflectState();
  }

  /* ------------------------------------------------------------------ *
   * 12b. More actions dropdown (#p-cactions)                            *
   * ------------------------------------------------------------------ */

  function initMoreMenu() {
    var details = document.getElementById('p-cactions-details');
    if (!details) return;

    document.addEventListener('click', function(e) {
      if (details.open && !details.contains(e.target)) {
        details.open = false;
      }
    });

    document.addEventListener('keydown', function(e) {
      if (e.key === 'Escape' && details.open) {
        details.open = false;
      }
    });
  }

  /* ------------------------------------------------------------------ *
   * 12. Sticky header                                                    *
   *                                                                     *
   * IntersectionObserver on #mw-header. Fires once at page load;       *
   * #mw-header stays in DOM across AJAX navigations.                    *
   * ------------------------------------------------------------------ */

  function initStickyHeader() {
    var stickyEl   = document.getElementById('wiki-sticky-header');
    var mainHeader = document.getElementById('mw-header');
    if (!stickyEl || !mainHeader) return;

    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          stickyEl.classList.remove('sticky-visible');
          stickyEl.setAttribute('aria-hidden', 'true');
        } else {
          stickyEl.classList.add('sticky-visible');
          stickyEl.removeAttribute('aria-hidden');
        }
      });
    }, { threshold: 0 });

    observer.observe(mainHeader);
  }

  /* ------------------------------------------------------------------ *
   * 13. Active TOC section tracking                                      *
   *                                                                     *
   * IntersectionObserver on h2/h3 headings. The previous observer is   *
   * disconnected and a fresh one created after each AJAX navigation     *
   * (old headings are replaced in the DOM swap).                        *
   * ------------------------------------------------------------------ */

  function initActiveTocTracking() {
    var tocList = document.getElementById('toc-list');
    if (!tocList) return;

    var headings = document.querySelectorAll('.prose h2[id], .prose h3[id]');
    if (!headings.length) return;

    if (_sectionObserver) {
      _sectionObserver.disconnect();
      _sectionObserver = null;
    }

    var activeId = null;

    function setActive(id) {
      if (id === activeId) return;
      activeId = id;
      tocList.querySelectorAll('li').forEach(function (li) {
        li.classList.remove('toc-section-active');
      });
      if (!id) return;
      var link = tocList.querySelector('a[href="#' + id + '"]');
      if (link && link.parentElement) link.parentElement.classList.add('toc-section-active');
    }

    _sectionObserver = new IntersectionObserver(function (entries) {
      var topEntry = null;
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          if (!topEntry || entry.boundingClientRect.top < topEntry.boundingClientRect.top) {
            topEntry = entry;
          }
        }
      });
      if (topEntry) setActive(topEntry.target.id);
    }, { rootMargin: '-10% 0px -70% 0px', threshold: 0 });

    headings.forEach(function (h) { _sectionObserver.observe(h); });
  }

  /* ------------------------------------------------------------------ *
   * 14. Keyboard shortcuts                                               *
   *                                                                     *
   * AccessKey attributes on anchor elements (added in server.rs) handle *
   * Alt/Ctrl+Option navigation natively across browsers. This section   *
   * adds the '?' help overlay showing the available bindings.           *
   * ------------------------------------------------------------------ */

  var SHORTCUT_OVERLAY_HTML = [
    '<div id="wiki-shortcut-overlay" role="dialog" aria-label="Keyboard shortcuts" aria-modal="true">',
    '  <div id="wiki-shortcut-panel">',
    '    <button id="wiki-shortcut-close" aria-label="Close">×</button>',
    '    <h2>Keyboard shortcuts</h2>',
    '    <table>',
    '      <tr><th>Key</th><th>Action</th></tr>',
    '      <tr><td>Alt+Shift+R</td><td>Read (view article)</td></tr>',
    '      <tr><td>Alt+Shift+E</td><td>Edit this page</td></tr>',
    '      <tr><td>Alt+Shift+S</td><td>View source</td></tr>',
    '      <tr><td>Alt+Shift+H</td><td>View history</td></tr>',
    '      <tr><td>Alt+Shift+T</td><td>Talk page</td></tr>',
    '      <tr><td>?</td><td>This help overlay</td></tr>',
    '      <tr><td>Esc</td><td>Close overlay / close mobile drawer</td></tr>',
    '    </table>',
    '    <p class="wiki-shortcut-note">On macOS, use Ctrl+Option instead of Alt+Shift.</p>',
    '  </div>',
    '</div>'
  ].join('\n');

  function initKeyboardShortcuts() {
    document.addEventListener('keydown', function (e) {
      if (e.target.tagName === 'INPUT'    ||
          e.target.tagName === 'TEXTAREA' ||
          e.target.isContentEditable) return;
      if (e.key === '?' && !e.altKey && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        toggleShortcutOverlay();
      }
      if (e.key === 'Escape') {
        var overlay = document.getElementById('wiki-shortcut-overlay');
        if (overlay) overlay.remove();
      }
    });
    // Cmd-K / Ctrl-K: focus the home page search input when present.
    document.addEventListener('keydown', function (e) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        var input = document.querySelector('.wiki-home-search input[type="search"]');
        if (input) { e.preventDefault(); input.focus(); }
      }
    });
  }

  function toggleShortcutOverlay() {
    var existing = document.getElementById('wiki-shortcut-overlay');
    if (existing) { existing.remove(); return; }
    document.body.insertAdjacentHTML('beforeend', SHORTCUT_OVERLAY_HTML);
    var overlay  = document.getElementById('wiki-shortcut-overlay');
    var closeBtn = document.getElementById('wiki-shortcut-close');
    closeBtn.addEventListener('click', function () { overlay.remove(); });
    overlay.addEventListener('click', function (e) {
      if (e.target === overlay) overlay.remove();
    });
    closeBtn.focus();
  }

  /* ------------------------------------------------------------------ *
   * 15. AJAX page navigation                                             *
   *                                                                     *
   * Intercepts clicks on /wiki/* links, fetches the new page, swaps    *
   * the article content and TOC in-place, and updates the URL via       *
   * pushState. The page header, left nav, and footer stay in place.     *
   *                                                                     *
   * Fallback: any fetch error falls through to a full page navigation.  *
   * Mobile TOC drawer is not updated after AJAX navigation (Phase 4).   *
   * ------------------------------------------------------------------ */

  function initAjaxNavigation() {
    document.addEventListener('click', handleNavClick);
    window.addEventListener('popstate', handlePopState);
    history.replaceState({ path: location.pathname }, '', location.pathname);
  }

  function handleNavClick(e) {
    var link = e.target.closest('a[href]');
    if (!link) return;
    var href = link.getAttribute('href');
    if (!href || !href.startsWith('/wiki/')) return;
    if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
    // Only do AJAX swap when already on an article page that has #mw-content-text.
    // From the homepage or category pages the element is absent; let the browser navigate normally.
    if (!document.querySelector('#mw-content-text')) return;
    e.preventDefault();
    navigateTo(href);
  }

  function handlePopState(e) {
    if (e.state && e.state.path) navigateTo(e.state.path, true);
  }

  function navigateTo(path, isPopState) {
    showLoadingBar();
    fetch(path, { headers: { 'Accept': 'text/html' } })
      .then(function (res) {
        if (!res.ok) throw new Error('HTTP ' + res.status);
        return res.text();
      })
      .then(function (html) {
        var doc = new DOMParser().parseFromString(html, 'text/html');

        var newContent = doc.querySelector('#mw-content-text');
        if (!newContent) throw new Error('No content element in response');

        // Swap article content
        var contentEl = document.querySelector('#mw-content-text');
        if (contentEl) contentEl.innerHTML = newContent.innerHTML;

        // Swap desktop TOC (re-init events after swap)
        var newToc = doc.querySelector('aside.toc');
        var tocEl  = document.querySelector('aside.toc');
        if (newToc && tocEl) tocEl.innerHTML = newToc.innerHTML;

        // Swap page title
        var newTitle = doc.querySelector('h1.article__title');
        var titleEl  = document.querySelector('h1.article__title');
        if (newTitle && titleEl) titleEl.innerHTML = newTitle.innerHTML;

        // Swap action tabs (Read/Edit/History hrefs change per slug)
        var newTabs = doc.querySelector('nav #p-views');
        var tabsEl  = document.querySelector('nav #p-views');
        if (newTabs && tabsEl) tabsEl.innerHTML = newTabs.innerHTML;

        // Swap breadcrumb if present
        var newCrumb = doc.querySelector('nav.crumb');
        var crumbEl  = document.querySelector('nav.crumb');
        if (newCrumb && crumbEl) crumbEl.innerHTML = newCrumb.innerHTML;

        // Update document title
        var newDocTitle = doc.querySelector('title');
        if (newDocTitle) document.title = newDocTitle.textContent;

        if (!isPopState) history.pushState({ path: path }, '', path);
        window.scrollTo(0, 0);

        reinitContentInteractions();
        finishLoadingBar();
      })
      .catch(function () {
        finishLoadingBar();
        window.location.href = path;
      });
  }

  /* Loading bar — thin progress indicator at the top of the viewport */

  function showLoadingBar() {
    var bar = document.getElementById('wiki-loading-bar');
    if (!bar) {
      bar = document.createElement('div');
      bar.id = 'wiki-loading-bar';
      document.body.prepend(bar);
    }
    bar.className = 'loading';
  }

  function finishLoadingBar() {
    var bar = document.getElementById('wiki-loading-bar');
    if (!bar) return;
    bar.className = 'done';
    setTimeout(function () { bar.className = ''; }, 400);
  }

  /* ------------------------------------------------------------------ *
   * reinitContentInteractions                                            *
   *                                                                     *
   * Called after every AJAX navigation. Re-binds all interactions that  *
   * target article content elements (which were replaced by the swap).  *
   * Also re-inits TOC buttons since #vector-toc innerHTML changed.      *
   * ------------------------------------------------------------------ */

  function reinitContentInteractions() {
    initHoverCards();
    initGlossaryTooltips();
    initFootnoteTooltips();
    initNavboxes();
    initCollapsibleSections();
    initActiveTocTracking();
    initToc();
    initTocPin();
    initAnchorShare();
    initCitationMarkerCards();
  }

  /* ------------------------------------------------------------------ *
   * Anchor-share ¶ buttons                                              *
   * ------------------------------------------------------------------ */

  function initAnchorShare() {
    var prose = document.querySelector('.prose');
    if (!prose) return;

    // Ensure toast element exists
    var toast = document.getElementById('anchor-toast');
    if (!toast) {
      toast = document.createElement('div');
      toast.id = 'anchor-toast';
      toast.className = 'anchor-toast';
      toast.textContent = 'Link copied';
      document.body.appendChild(toast);
    }

    // Inject ¶ button after each h2/h3 with an id
    prose.querySelectorAll('h2[id], h3[id]').forEach(function (heading) {
      if (heading.querySelector('.anchor-share')) return; // idempotent
      var btn = document.createElement('button');
      btn.className = 'anchor-share';
      btn.setAttribute('aria-label', 'Copy section link');
      btn.setAttribute('data-target', heading.id);
      btn.textContent = '¶';
      heading.appendChild(btn);
    });

    // Delegate click to document (survives re-inits)
    if (prose._anchorShareBound) return;
    prose._anchorShareBound = true;
    prose.addEventListener('click', function (e) {
      var btn = e.target.closest('.anchor-share');
      if (!btn) return;
      var url = window.location.href.split('#')[0] + '#' + btn.getAttribute('data-target');
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(url).then(function () { showAnchorToast(); });
      } else {
        // Fallback for older browsers
        var tmp = document.createElement('textarea');
        tmp.value = url;
        document.body.appendChild(tmp);
        tmp.select();
        document.execCommand('copy');
        document.body.removeChild(tmp);
        showAnchorToast();
      }
    });
  }

  function showAnchorToast() {
    var toast = document.getElementById('anchor-toast');
    if (!toast) return;
    toast.classList.add('visible');
    setTimeout(function () { toast.classList.remove('visible'); }, 1500);
  }

  /* ------------------------------------------------------------------ *
   * Reading mode                                                         *
   * ------------------------------------------------------------------ */

  function initReadingMode() {
    var btn = document.getElementById('reading-mode-btn');
    if (!btn) return;
    var PREF = 'wiki-reading-mode';
    var on = localStorage.getItem(PREF) === '1';
    if (on) {
      document.body.classList.add('reading-mode');
      btn.setAttribute('aria-pressed', 'true');
    }
    btn.addEventListener('click', function () {
      var active = document.body.classList.toggle('reading-mode');
      btn.setAttribute('aria-pressed', active ? 'true' : 'false');
      localStorage.setItem(PREF, active ? '1' : '0');
    });
  }

  /* ------------------------------------------------------------------ *
   * Citation marker hover cards (.citation-marker[data-citation-id])   *
   * Fetches citation metadata from /api/citations?id=<id> and shows    *
   * a positioned hover card. (Phase 3 — §7 wiki.js responsibilities)   *
   * ------------------------------------------------------------------ */
  function initCitationMarkerCards() {
    var citationCard = null;
    var citationTimer = null;

    function getCitationCard() {
      if (!citationCard) {
        citationCard = document.createElement('div');
        citationCard.className = 'cite-hover-card';
        citationCard.style.display = 'none';
        citationCard.style.position = 'absolute';
        document.body.appendChild(citationCard);
        citationCard.addEventListener('mouseenter', function () { clearTimeout(citationTimer); });
        citationCard.addEventListener('mouseleave', function () {
          citationTimer = setTimeout(function () { if (citationCard) citationCard.style.display = 'none'; }, 200);
        });
      }
      return citationCard;
    }

    document.querySelectorAll('.citation-marker[data-citation-id]').forEach(function (el) {
      el.addEventListener('mouseenter', function () {
        clearTimeout(citationTimer);
        var id = el.dataset.citationId;
        if (!id) return;
        var card = getCitationCard();
        var rect = el.getBoundingClientRect();
        card.style.left = (window.scrollX + rect.left) + 'px';
        card.style.top  = (window.scrollY + rect.bottom + 4) + 'px';
        card.innerHTML = '<em>Loading…</em>';
        card.style.display = 'block';
        fetch('/api/citations?id=' + encodeURIComponent(id))
          .then(function (r) { return r.ok ? r.json() : null; })
          .then(function (data) {
            if (!data) { card.style.display = 'none'; return; }
            var parts = [];
            if (data.title) parts.push('<strong>' + data.title + '</strong>');
            if (data.authors) parts.push(data.authors);
            if (data.year) parts.push(data.year);
            if (data.url) parts.push('<a href="' + data.url + '" target="_blank" rel="noopener">' + data.url + '</a>');
            card.innerHTML = parts.join(' · ');
          })
          .catch(function () { card.style.display = 'none'; });
      });
      el.addEventListener('mouseleave', function () {
        citationTimer = setTimeout(function () { if (citationCard) citationCard.style.display = 'none'; }, 200);
      });
    });
  }

  function initCitationHoverCards() {
    var card = null;

    function getCard() {
      if (!card) {
        card = document.createElement('div');
        card.className = 'cite-hover-card';
        document.body.appendChild(card);
      }
      return card;
    }

    function showCard(sup, evt) {
      var anchor = sup.querySelector('a[href^="#fn"]');
      if (!anchor) return;
      var fnId = anchor.getAttribute('href').slice(1); // e.g. "fn-1"
      var fnEl = document.getElementById(fnId);
      if (!fnEl) return;
      var c = getCard();
      c.innerHTML = fnEl.innerHTML;
      // Strip back-link arrow inside the card.
      var back = c.querySelector('.footnote-backref');
      if (back) back.remove();
      var r = sup.getBoundingClientRect();
      c.style.left = (window.scrollX + r.left) + 'px';
      c.style.top  = (window.scrollY + r.bottom + 4) + 'px';
      c.style.display = 'block';
    }

    function hideCard() {
      if (card) card.style.display = 'none';
    }

    document.querySelectorAll('sup.footnote-ref').forEach(function (sup) {
      sup.addEventListener('mouseenter', function (e) { showCard(sup, e); });
      sup.addEventListener('mouseleave', hideCard);
    });
  }

  /* ------------------------------------------------------------------ *
   * M1 — Tap-to-open popovers on touch devices                          *
   *                                                                     *
   * The glossary / footnote / citation popovers above bind only         *
   * mouseenter/mouseleave, so they are unreachable on touch (no hover). *
   * On (hover: none) devices a tap on a NON-navigating affordance        *
   * (glossary term, footnote/citation ref) re-dispatches the same mouse  *
   * events to reuse the existing show/hide logic; tap-again, tap-outside,*
   * or Esc dismiss. Wikilinks are excluded — a tap on a real link should *
   * navigate, not preview. Delegated once on document so it survives     *
   * AJAX navigation.                                                     *
   * ------------------------------------------------------------------ */
  function initTapPopovers() {
    if (!window.matchMedia || !window.matchMedia('(hover: none)').matches) return;
    var SEL = '.wiki-glossary-term, sup.footnote-ref';
    var openEl = null;
    function fire(el, type) { el.dispatchEvent(new MouseEvent(type, { bubbles: true })); }
    function close() { if (openEl) { fire(openEl, 'mouseleave'); openEl = null; } }
    document.addEventListener('click', function (e) {
      var t = e.target.closest(SEL);
      if (t) {
        e.preventDefault();        // footnote refs: show inline, don't jump to the note
        if (openEl === t) { close(); return; } // tap again closes
        if (openEl) fire(openEl, 'mouseleave');
        fire(t, 'mouseenter');
        openEl = t;
      } else {
        close();                   // tap outside closes
      }
    });
    document.addEventListener('keydown', function (e) { if (e.key === 'Escape') close(); });
  }

  /* ------------------------------------------------------------------ *
   * Phase 4 — Command palette (Cmd/Ctrl-K)                              *
   *                                                                     *
   * Self-contained: builds its own overlay (no markup change). Fuzzy    *
   * search via /api/complete; arrow/enter nav; Esc / tap-scrim close.   *
   * Full-screen on mobile (≤640px), centered palette on desktop.        *
   * ------------------------------------------------------------------ */
  function initCommandPalette() {
    var overlay = document.createElement('div');
    overlay.className = 'cmdk-overlay';
    overlay.setAttribute('aria-hidden', 'true');
    overlay.innerHTML =
      '<div class="cmdk-panel" role="dialog" aria-modal="true" aria-label="Search articles">' +
        '<input class="cmdk-input" type="search" placeholder="Search articles…" autocomplete="off" spellcheck="false" aria-label="Search articles">' +
        '<ul class="cmdk-results" role="listbox"></ul>' +
      '</div>';
    document.body.appendChild(overlay);
    var input = overlay.querySelector('.cmdk-input');
    var list = overlay.querySelector('.cmdk-results');
    var items = [];
    var active = -1;
    var debounce;

    function isOpen() { return overlay.getAttribute('aria-hidden') === 'false'; }
    function open() {
      overlay.setAttribute('aria-hidden', 'false');
      document.body.style.overflow = 'hidden';
      setTimeout(function () { input.focus(); }, 40);
    }
    window.openCmdK = open;
    function close() {
      overlay.setAttribute('aria-hidden', 'true');
      document.body.style.overflow = '';
      input.value = ''; list.innerHTML = ''; items = []; active = -1;
    }
    function highlight() {
      Array.prototype.forEach.call(list.children, function (li, i) {
        li.classList.toggle('cmdk-active', i === active);
        li.setAttribute('aria-selected', i === active ? 'true' : 'false');
        if (i === active) li.scrollIntoView({ block: 'nearest' });
      });
    }
    function go(i) { if (items[i]) window.location.href = '/wiki/' + items[i].slug; }
    function render(rows) {
      items = rows || [];
      list.innerHTML = '';
      items.forEach(function (r, i) {
        var li = document.createElement('li');
        li.className = 'cmdk-item';
        li.setAttribute('role', 'option');
        li.textContent = r.title;
        li.addEventListener('click', function () { go(i); });
        list.appendChild(li);
      });
      active = items.length ? 0 : -1;
      highlight();
    }

    input.addEventListener('input', function () {
      var q = input.value.trim();
      clearTimeout(debounce);
      if (!q) { render([]); return; }
      debounce = setTimeout(function () {
        fetch('/api/complete?q=' + encodeURIComponent(q))
          .then(function (r) { return r.json(); })
          .then(function (d) { if (isOpen()) render((d || []).slice(0, 20)); })
          .catch(function () {});
      }, 120);
    });
    input.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowDown') { e.preventDefault(); active = Math.min(active + 1, items.length - 1); highlight(); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); active = Math.max(active - 1, 0); highlight(); }
      else if (e.key === 'Enter') { e.preventDefault(); go(active); }
      else if (e.key === 'Escape') { e.preventDefault(); close(); }
    });
    overlay.addEventListener('click', function (e) { if (e.target === overlay) close(); });
    document.addEventListener('keydown', function (e) {
      if ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        isOpen() ? close() : open();
      }
    });
  }

  /* ------------------------------------------------------------------ *
   * Phase 7E — Mobile bottom bar                                        *
   * ------------------------------------------------------------------ */
  function initMobileBottomBar() {
    var tocBtn   = document.getElementById('mobile-bar-toc');
    var shareBtn = document.getElementById('mobile-bar-share');
    if (!tocBtn && !shareBtn) return;

    // Contents → open TOC drawer (same as toc-toggle-btn)
    if (tocBtn) {
      tocBtn.addEventListener('click', function () {
        var tocTrigger = document.getElementById('toc-toggle-btn');
        if (tocTrigger) { tocTrigger.click(); }
      });
    }

    // Share → navigator.share or clipboard fallback
    if (shareBtn) {
      shareBtn.addEventListener('click', function () {
        var title = document.title;
        var url   = window.location.href;
        if (navigator.share) {
          navigator.share({ title: title, url: url }).catch(function () {});
        } else {
          navigator.clipboard.writeText(url).then(function () {
            var orig = shareBtn.textContent;
            shareBtn.textContent = 'Copied!';
            setTimeout(function () { shareBtn.textContent = orig; }, 1500);
          }).catch(function () {});
        }
      });
    }
  }

  /* ------------------------------------------------------------------ *
   * Phase 10 — Reading state progress bar                               *
   * ------------------------------------------------------------------ */

  function initReadingProgress() {
    function getState() {
      try { return JSON.parse(localStorage.getItem('wiki-read-state') || '{}'); } catch(e) { return {}; }
    }
    function setState(data) {
      try { localStorage.setItem('wiki-read-state', JSON.stringify(data)); } catch(e) {}
    }

    var bar = document.querySelector('.reading-progress-bar');
    var slug = document.body.dataset.slug;
    if (bar && slug) {
      function scrollPct() {
        var el = document.documentElement;
        var scrollable = el.scrollHeight - el.clientHeight;
        return scrollable > 0 ? Math.round((window.scrollY / scrollable) * 100) : 0;
      }
      function updateBar() { bar.style.width = scrollPct() + '%'; }

      var state = getState();
      if (state[slug] && state[slug].scrollPct > 0) {
        var scrollable = document.documentElement.scrollHeight - document.documentElement.clientHeight;
        window.scrollTo(0, Math.round((state[slug].scrollPct / 100) * scrollable));
      }
      updateBar();

      window.addEventListener('scroll', function () {
        updateBar();
        var pct = scrollPct();
        var st = getState();
        st[slug] = { scrollPct: pct, lastReadAt: Date.now(), completed: pct >= 90 };
        setState(st);
      }, { passive: true });
    }

    var strip = document.getElementById('continue-reading-strip');
    if (strip) {
      var entries = Object.entries(getState())
        .filter(function(e) { return !e[1].completed && e[1].scrollPct > 3; })
        .sort(function(a, b) { return b[1].lastReadAt - a[1].lastReadAt; })
        .slice(0, 5);
      if (entries.length > 0) {
        strip.hidden = false;
        strip.innerHTML = '<p class="continue-reading__label">Continue reading</p>' +
          entries.map(function(e) {
            var s = e[0]; var pct = e[1].scrollPct;
            return '<a class="continue-reading__item" href="/wiki/' + s + '">' +
              '<span class="continue-reading__title">' + s.replace(/-/g,' ') + '</span>' +
              '<span class="continue-reading__pct">' + pct + '%</span>' +
              '</a>';
          }).join('');
      }
    }
  }

  /* ------------------------------------------------------------------ *
   * Phase 9 — Claim-rail IntersectionObserver                           *
   * ------------------------------------------------------------------ */

  function initClaimRail() {
    var rail = document.querySelector('.claim-rail');
    if (!rail) return;
    var ticks = Array.from(rail.querySelectorAll('.claim-tick[data-para]'));
    if (!ticks.length) return;

    var paras = {};
    ticks.forEach(function(t) { paras[t.dataset.para] = t; });

    var obs = new IntersectionObserver(function(entries) {
      entries.forEach(function(entry) {
        var id = entry.target.id || entry.target.dataset.para;
        var tick = paras[id];
        if (tick) tick.classList.toggle('active', entry.isIntersecting);
      });
    }, { threshold: 0.3 });

    Object.keys(paras).forEach(function(id) {
      var el = document.getElementById(id) || document.querySelector('[data-para="' + id + '"]');
      if (el) obs.observe(el);
    });

    ticks.forEach(function(t) {
      t.addEventListener('click', function(e) {
        e.preventDefault();
        var id = t.dataset.para;
        var el = document.getElementById(id);
        if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
      });
    });
  }

   /* ------------------------------------------------------------------ *
   * Boot                                                                 *
   * ------------------------------------------------------------------ */

  document.addEventListener('DOMContentLoaded', function () {
    initAppearanceMenu();
    initMoreMenu();
    initDensityToggle();
    initHoverCards();
    initGlossaryTooltips();
    initMobileNav();
    initTocDrawer();
    initStickyHeader();
    initActiveTocTracking();
    initCollapsibleSections();
    initFootnoteTooltips();
    initNavboxes();
    initSearchAutocomplete();
    initKeyboardShortcuts();
    initAjaxNavigation();
    initAnchorShare();
    initReadingMode();
    initCitationHoverCards();
    initCitationMarkerCards();
    initMobileBottomBar();
    initReadingProgress();
    initClaimRail();
    initTapPopovers();
    initCommandPalette();
    initScrollElevation();
    initCategoryFacets();
  });

}());

// Scroll elevation for topnav
(function() {
  var nav = document.querySelector('header.topnav');
  if (!nav) return;
  window.addEventListener('scroll', function() {
    nav.classList.toggle('scrolled', window.scrollY > 8);
  }, { passive: true });
})();

// Category page facet filter pills
(function() {
  document.querySelectorAll('.facet-bar').forEach(function(bar) {
    bar.querySelectorAll('.facet-pill').forEach(function(pill) {
      pill.addEventListener('click', function() {
        var f = pill.dataset.filter;
        bar.querySelectorAll('.facet-pill').forEach(function(p) {
          p.classList.toggle('is-active', p === pill);
        });
        document.querySelectorAll('.wiki-cat-page-item').forEach(function(li) {
          li.hidden = f !== 'all' && li.dataset.kind !== f;
        });
      });
    });
  });
})();

function initScrollElevation() {
  var nav = document.querySelector('header.topnav');
  if (!nav) return;
  window.addEventListener('scroll', function() {
    nav.classList.toggle('scrolled', window.scrollY > 8);
  }, { passive: true });
}

function initCategoryFacets() {
  document.querySelectorAll('.facet-bar').forEach(function(bar) {
    bar.querySelectorAll('.facet-pill').forEach(function(pill) {
      pill.addEventListener('click', function() {
        var f = pill.dataset.filter;
        bar.querySelectorAll('.facet-pill').forEach(function(p) {
          p.classList.toggle('is-active', p === pill);
        });
        document.querySelectorAll('.wiki-cat-page-item').forEach(function(li) {
          li.hidden = f !== 'all' && li.dataset.kind !== f;
        });
      });
    });
  });
}

// Topnav scrolled shadow — adds .scrolled class when page is scrolled > 4px
(function() {
  var nav = document.querySelector('header.topnav');
  if (!nav) return;
  function update() { nav.classList.toggle('scrolled', window.scrollY > 4); }
  window.addEventListener('scroll', update, { passive: true });
  update();
}());

// Phase 4b — Reading time estimate (article pages only)
(function() {
  var rt = document.querySelector('.reading-time[data-words]');
  var prose = document.querySelector('.prose');
  if (!rt || !prose) return;
  var words = prose.innerText.trim().split(/\s+/).length;
  var mins = Math.max(1, Math.round(words / 200));
  rt.setAttribute('data-words', words);
  rt.textContent = mins + ' min read';
}());

// Phase 4f — Reading progress bar (article pages only; reuses #wiki-loading-bar)
(function() {
  var bar = document.getElementById('wiki-loading-bar');
  var article = document.querySelector('article.article__body');
  if (!bar || !article) return;
  // Only activate when no AJAX nav is in flight
  var active = false;
  function update() {
    if (active) return;
    var rect = article.getBoundingClientRect();
    var total = article.offsetHeight - window.innerHeight;
    if (total <= 0) return;
    var scrolled = Math.max(0, -rect.top);
    var pct = Math.min(1, scrolled / total);
    bar.style.transform = 'scaleX(' + pct + ')';
    bar.style.opacity = pct > 0.01 ? '0.8' : '0';
    bar.style.transition = 'none';
    bar.style.background = 'var(--ct-color, var(--accent))';
  }
  window.addEventListener('scroll', update, { passive: true });
  // Disable progress bar during AJAX navigation (loading bar takes over)
  document.addEventListener('wiki:nav-start', function() { active = true; bar.style.opacity = '0'; });
  document.addEventListener('wiki:nav-done',  function() { active = false; bar.style.transform = 'scaleX(0)'; });
  update();
}());

// Search toggle — opens/closes the topnav search panel
(function() {
  var btn = document.querySelector('.search-toggle');
  if (!btn) return;
  btn.addEventListener('click', function() {
    var open = document.documentElement.classList.toggle('search-open');
    btn.setAttribute('aria-expanded', open);
    if (open) {
      var input = document.getElementById('header-search-q');
      if (input) { input.focus(); }
    }
  });
  document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
      document.documentElement.classList.remove('search-open');
      if (btn) btn.setAttribute('aria-expanded', 'false');
    }
  });
  // Close panel when clicking outside
  document.addEventListener('click', function(e) {
    if (!document.documentElement.classList.contains('search-open')) return;
    var panel = document.getElementById('topnav-search-panel');
    var topnav = document.querySelector('header.topnav');
    if (panel && !panel.contains(e.target) && topnav && !topnav.contains(e.target)) {
      document.documentElement.classList.remove('search-open');
      if (btn) btn.setAttribute('aria-expanded', 'false');
    }
  });
}());

// Theme cycle: #s-theme-btn cycles light → dark → auto
(function() {
  var btn = document.getElementById('s-theme-btn');
  if (!btn) return;
  var CYCLE = ['light', 'dark', 'auto'];
  function setTheme(t) {
    if (t === 'auto') {
      var prefer = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      document.documentElement.setAttribute('data-theme', prefer);
    } else {
      document.documentElement.setAttribute('data-theme', t);
    }
    try { localStorage.setItem('wiki-theme', t); } catch(e) {}
    btn.setAttribute('aria-label', t === 'light' ? 'Switch to dark mode' : t === 'dark' ? 'Switch to auto mode' : 'Switch to light mode');
  }
  btn.addEventListener('click', function() {
    var current = localStorage.getItem('wiki-theme') || 'light';
    var next = CYCLE[(CYCLE.indexOf(current) + 1) % CYCLE.length];
    setTheme(next);
  });
}());
