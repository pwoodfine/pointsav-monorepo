// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// bim-catalog.js — home-page catalog enhancement (Objects / Compositions).
//
// The page is fully server-rendered: both tab grids, both facet sets, and the
// result counter are already in the HTML. This module layers on tab switching,
// faceted filtering, and the detail modal. Detail data is fetched once from
// /api/tokens.json (the `_catalog` key added to that endpoint) so the modal
// can show full spec tables and, for compositions, the resolved bill of
// objects — without a full page reload.
//
// Plain vanilla JS, matching bim.js. Guarded against double-init so it is safe
// to re-run when bim.js swaps it back in during SPA navigation.

(function () {
  const root = document.getElementById('bim-catalog');
  if (!root || root.dataset.catInit === '1') return;
  root.dataset.catInit = '1';

  const modal = document.getElementById('bim-cat-modal');
  const modalHead = document.getElementById('bim-cat-modal-head');
  const modalBody = document.getElementById('bim-cat-modal-body');
  const modalPanel = document.getElementById('bim-cat-modal-panel');
  const searchInput = document.getElementById('bim-cat-search');
  const resetBtn = document.getElementById('bim-cat-reset');
  const resCount = document.getElementById('bim-cat-rescount');
  const resNoun = document.getElementById('bim-cat-resnoun');

  const state = { tab: 'objects', search: '', facets: {} };
  const CATALOG = { objects: {}, compositions: {} }; // id -> entry, filled by fetch

  const esc = (s) =>
    String(s == null ? '' : s)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');

  // ── data payload ──────────────────────────────────────────────────────────
  fetch('/api/tokens.json', { headers: { Accept: 'application/json' } })
    .then((r) => (r.ok ? r.json() : null))
    .then((json) => {
      const cat = json && json._catalog;
      if (!cat) return;
      (cat.objects || []).forEach((o) => { CATALOG.objects[o.id] = o; });
      (cat.compositions || []).forEach((c) => { CATALOG.compositions[c.id] = c; });
    })
    .catch(() => {});

  // ── helpers ───────────────────────────────────────────────────────────────
  function activePanel() {
    return document.getElementById('bim-panel-' + state.tab);
  }
  function activeFacetset() {
    return root.querySelector('.bim-cat-facetset[data-tab="' + state.tab + '"]');
  }
  function noun(n) {
    if (state.tab === 'objects') return n === 1 ? 'object' : 'objects';
    return n === 1 ? 'composition' : 'compositions';
  }

  // ── filtering ─────────────────────────────────────────────────────────────
  function cardMatches(card) {
    if (state.search) {
      const hay = (card.dataset.search || '');
      if (hay.indexOf(state.search) === -1) return false;
    }
    for (const key in state.facets) {
      const sel = state.facets[key];
      if (!sel || !sel.size) continue;
      if (!sel.has(card.dataset[key])) return false;
    }
    return true;
  }

  function applyFilter() {
    const panel = activePanel();
    if (!panel) return;
    let shown = 0;
    panel.querySelectorAll('.bim-cat-card').forEach((card) => {
      const ok = cardMatches(card);
      card.classList.toggle('is-hidden', !ok);
      if (ok) shown += 1;
    });
    if (resCount) resCount.textContent = String(shown);
    if (resNoun) resNoun.textContent = noun(shown);
    let empty = panel.querySelector('.bim-cat-empty');
    if (shown === 0) {
      if (!empty) {
        empty = document.createElement('div');
        empty.className = 'bim-cat-empty';
        empty.textContent = 'No ' + noun(0) + ' match the current filters.';
        panel.appendChild(empty);
      }
      empty.hidden = false;
    } else if (empty) {
      empty.hidden = true;
    }
    const anySel = Object.values(state.facets).some((s) => s && s.size);
    if (resetBtn) resetBtn.hidden = !(anySel || state.search);
  }

  // ── tabs ──────────────────────────────────────────────────────────────────
  function setTab(tab) {
    state.tab = tab;
    state.facets = {};
    state.search = '';
    if (searchInput) searchInput.value = '';
    root.querySelectorAll('.bim-cat-tab').forEach((t) => {
      t.setAttribute('aria-selected', String(t.dataset.tab === tab));
    });
    root.querySelectorAll('.bim-cat-grid').forEach((g) => {
      g.hidden = g.id !== 'bim-panel-' + tab;
    });
    root.querySelectorAll('.bim-cat-facetset').forEach((f) => {
      f.hidden = f.dataset.tab !== tab;
      f.querySelectorAll('input[type="checkbox"]').forEach((cb) => { cb.checked = false; });
    });
    applyFilter();
  }

  root.querySelectorAll('.bim-cat-tab').forEach((t) => {
    t.addEventListener('click', () => setTab(t.dataset.tab));
    t.addEventListener('keydown', (e) => {
      if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return;
      e.preventDefault();
      const tabs = [...root.querySelectorAll('.bim-cat-tab')];
      const i = tabs.indexOf(t);
      const dir = e.key === 'ArrowRight' ? 1 : tabs.length - 1;
      const next = tabs[(i + dir) % tabs.length];
      next.focus();
      next.click();
    });
  });

  // ── facet + search events ─────────────────────────────────────────────────
  root.addEventListener('change', (e) => {
    const cb = e.target.closest('input[data-facet]');
    if (!cb) return;
    const k = cb.dataset.facet;
    state.facets[k] = state.facets[k] || new Set();
    if (cb.checked) state.facets[k].add(cb.value);
    else state.facets[k].delete(cb.value);
    applyFilter();
  });

  if (searchInput) {
    let timer;
    searchInput.addEventListener('input', (e) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        state.search = e.target.value.trim().toLowerCase();
        applyFilter();
      }, 90);
    });
  }

  if (resetBtn) {
    resetBtn.addEventListener('click', () => {
      state.facets = {};
      state.search = '';
      if (searchInput) searchInput.value = '';
      const fs = activeFacetset();
      if (fs) fs.querySelectorAll('input[type="checkbox"]').forEach((cb) => { cb.checked = false; });
      applyFilter();
    });
  }

  // ── card click → modal ────────────────────────────────────────────────────
  root.addEventListener('click', (e) => {
    const card = e.target.closest('.bim-cat-card');
    if (!card) return;
    openDetail(card.dataset.kind, card.dataset.id);
  });

  function specTable(spec) {
    if (!spec || !spec.length) return '';
    const rows = spec
      .map((r) => '<tr><th>' + esc(r[0]) + '</th><td>' + esc(r[1]) + '</td></tr>')
      .join('');
    return '<table class="bim-cat-spectable"><tbody>' + rows + '</tbody></table>';
  }

  function sectionH(t) {
    return '<div class="bim-cat-secth">' + esc(t) + '</div>';
  }

  function objectDetail(o) {
    const dl = o.ifc_file
      ? '<a class="bim-cat-btn" href="/furniture/download/' + esc(o.ifc_file) + '">Download IFC (.ifc)</a>'
      : '<p class="bim-cat-note">No IFC block published for this object yet.</p>';
    const src = o.url
      ? '<p class="bim-cat-note">Manufacturer source: <a href="' + esc(o.url) + '" target="_blank" rel="noopener">' + esc(o.url) + '</a></p>'
      : '';
    return (
      '<p class="bim-cat-desc">' + esc(o.description) + '</p>' +
      '<div>' + sectionH('Specification & property set') + specTable(o.spec) + '</div>' +
      '<div>' + sectionH('Classification & registry') +
        '<div class="bim-cat-classblock">' +
          '<div class="bim-cat-clrow"><span class="bim-cat-clrow__k">IFC 4.3 entity class</span><span class="bim-cat-clrow__v">' + esc(o.ifc_class) + '</span></div>' +
          '<div class="bim-cat-clrow"><span class="bim-cat-clrow__k">Uniclass 2015 — Pr (Products)</span><span class="bim-cat-clrow__v">' + esc(o.uniclass_pr) + '<small>' + esc(o.uniclass_pr_title) + '</small></span></div>' +
          '<div class="bim-cat-clrow"><span class="bim-cat-clrow__k">Registry reference</span><span class="bim-cat-clrow__v">' + esc(o.ref) + '</span></div>' +
        '</div>' +
      '</div>' +
      '<div>' + sectionH('Download') + dl + src + '</div>'
    );
  }

  function zoneBars(c) {
    if (!c.has_zone_data) {
      return '<p class="bim-cat-note">Floor-scale plan — sized as a proportion of the floor plate; no three-zone cross-section is modeled at this program level.</p>';
    }
    const z = [
      ['Zone 1', 'Habitat', c.zone1],
      ['Zone 2', 'Magazine', c.zone2],
      ['Zone 3', 'Corridor', c.zone3],
    ].filter((r) => r[2] != null);
    const max = Math.max.apply(null, z.map((r) => parseFloat(r[2]) || 0));
    return (
      '<div class="bim-cat-zones">' +
      z.map((r) => {
        const pct = max > 0 ? Math.round(((parseFloat(r[2]) || 0) / max) * 100) : 0;
        return (
          '<div class="bim-cat-zone"><div class="bim-cat-zone__k">' + esc(r[0]) + '</div>' +
          '<div class="bim-cat-zone__t">' + esc(r[1]) + '</div>' +
          '<div class="bim-cat-zone__n">' + esc(r[2]) + '<small> m</small></div>' +
          '<div class="bim-cat-zone__bar"><i style="width:' + pct + '%"></i></div></div>'
        );
      }).join('') +
      '</div>'
    );
  }

  function billBlock(c) {
    const rows = (c.bill || []).map((b) => {
      if (b.linked) {
        return (
          '<button class="bim-cat-billrow" type="button" data-obj="' + esc(b.obj_id) + '">' +
          '<span class="bim-cat-billrow__main"><span class="bim-cat-billrow__name">' + esc(b.name) + '</span>' +
          '<span class="bim-cat-billrow__code">' + esc(b.code) + ' · view object →</span></span></button>'
        );
      }
      return (
        '<div class="bim-cat-billrow bim-cat-billrow--ext">' +
        '<span class="bim-cat-billrow__main"><span class="bim-cat-billrow__name">' + esc(b.name) + '</span></span></div>'
      );
    }).join('');
    const note = c.refs_pending
      ? '<p class="bim-cat-note">Prose furniture program shown — structured object linking pending for this entry.</p>'
      : '';
    return '<div class="bim-cat-bill">' + rows + '</div>' + note;
  }

  function complianceBlock(c) {
    if (!c.compliance || typeof c.compliance !== 'object') return '';
    const keys = Object.keys(c.compliance);
    if (!keys.length) return '';
    const rows = keys.map((k) =>
      '<div class="bim-cat-cons"><b>' + esc(k.replace(/_/g, ' ')) + '</b> ' + esc(c.compliance[k]) + '</div>'
    ).join('');
    return '<div>' + sectionH('Constraints held') + '<div class="bim-cat-cons-list">' + rows + '</div></div>';
  }

  function compositionDetail(c) {
    // "Data Box" per the architects' own sketch-note label for this exact
    // size/area summary panel (DISCOVERY_MCorp_Sketches_Key Plans_Business_Notes.pdf,
    // e.g. "DATA BOX B (SMALL)").
    const area = c.area_sf
      ? '<div class="bim-cat-areabox"><span class="bim-cat-areabox__kicker">Data Box</span><span class="bim-cat-areabox__n">' + esc(c.area_sf) + '<small> SF</small></span>' +
        '<span class="bim-cat-areabox__l">Net leasable area' + (c.area_m2 ? ' · ' + esc(c.area_m2) + ' m²' : '') + '</span></div>'
      : '';
    const svg = c.svg ? '<div class="bim-cat-preview" data-category="' + esc(c.category) + '">' + c.svg + '</div>' : '';
    return (
      svg +
      '<p class="bim-cat-desc">' + esc(c.description) + '</p>' +
      area +
      '<div>' + sectionH('Zone allocation') + zoneBars(c) + '</div>' +
      '<div>' + sectionH('Composed from — bill of objects') + billBlock(c) + '</div>' +
      complianceBlock(c) +
      '<div>' + sectionH('Specification') + specTable(c.spec) + '</div>' +
      '<div>' + sectionH('Classification & registry') +
        '<div class="bim-cat-classblock">' +
          '<div class="bim-cat-clrow"><span class="bim-cat-clrow__k">Uniclass 2015 — SL (Spaces/locations)</span><span class="bim-cat-clrow__v">' + esc(c.uniclass_space) + '</span></div>' +
          '<div class="bim-cat-clrow"><span class="bim-cat-clrow__k">Key Plan reference</span><span class="bim-cat-clrow__v">' + esc(c.id) + '<small>' + esc(c.category_label) + '</small></span></div>' +
        '</div>' +
      '</div>'
    );
  }

  let lastFocus = null;
  function openDetail(kind, id) {
    const isObj = kind === 'object';
    const item = isObj ? CATALOG.objects[id] : CATALOG.compositions[id];
    if (!item) return; // catalog not loaded yet / unknown id
    lastFocus = document.activeElement;
    const chip = isObj
      ? '<span class="bim-cat-chip bim-cat-chip--pr"><span class="bim-cat-chip__lv">Pr</span>' + esc(item.uniclass_pr) + '</span>' +
        '<span class="bim-cat-chip bim-cat-chip--plain">' + esc(item.ifc_class) + '</span>'
      : '<span class="bim-cat-chip bim-cat-chip--ef"><span class="bim-cat-chip__lv">SL</span>' + esc(item.uniclass_space) + '</span>' +
        '<span class="bim-cat-chip bim-cat-chip--plain">KEY PLAN</span>';
    const sub = isObj
      ? esc(item.manufacturer) + ' · certified BIM Object'
      : 'Composition · assembled from ' + (item.bill ? item.bill.length : 0) + ' object entries';
    modalHead.innerHTML =
      '<div class="bim-cat-modal__ey">' + chip + '<span class="bim-cat-modal__id">' + esc(item.id) + '</span></div>' +
      '<h2 class="bim-cat-modal__title" id="bim-cat-modal-title">' + esc(item.name) + '</h2>' +
      '<p class="bim-cat-modal__sub">' + sub + '</p>';
    modalBody.innerHTML = isObj ? objectDetail(item) : compositionDetail(item);
    modalBody.querySelectorAll('[data-obj]').forEach((b) => {
      b.addEventListener('click', () => openDetail('object', b.dataset.obj));
    });
    modal.classList.add('is-open');
    modal.setAttribute('aria-hidden', 'false');
    document.body.style.overflow = 'hidden';
    modalBody.scrollTop = 0;
    if (modalPanel) modalPanel.focus();
  }

  function closeDetail() {
    modal.classList.remove('is-open');
    modal.setAttribute('aria-hidden', 'true');
    document.body.style.overflow = '';
    if (lastFocus && lastFocus.focus) lastFocus.focus();
  }

  modal.querySelectorAll('[data-cat-close]').forEach((el) => el.addEventListener('click', closeDetail));
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && modal.classList.contains('is-open')) closeDetail();
  });

  // ── init ──────────────────────────────────────────────────────────────────
  setTab('objects');
})();
