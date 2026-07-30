/**
 * Workplace*Memo — fonts.js
 * Font panel: displays available fonts, handles @font-face registration
 * for fonts stored in the app data directory, and injects font previews.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * All built-in fonts are pre-embedded via font-data.js (generated at build
 * time from the .woff2 files in fonts/). Downloaded fonts are stored in
 * the OS app data directory and loaded via the read_font_file Tauri command.
 */

'use strict';

/* ─── Built-in font registry ─────────────────────────────────────────────── */

// These families are bundled with the app (see fonts/ directory).
// All are SIL Open Font Licence.
const BUILTIN_FONTS = [
  {
    family:  'EB Garamond',
    source:  'Octavio Pardo — Georg Duffner',
    licence: 'SIL OFL 1.1',
    category: 'Humanist Serif',
    useCase:  'Memos, briefs, formal documents',
  },
  {
    family:  'Source Serif 4',
    source:  'Adobe Systems — Frank Grießhammer',
    licence: 'SIL OFL 1.1',
    category: 'Workhorse Serif',
    useCase:  'Long reports, proposals',
  },
  {
    family:  'Lora',
    source:  'Cyreal',
    licence: 'SIL OFL 1.1',
    category: 'Literary Serif',
    useCase:  'Narrative reports, editorial',
  },
  {
    family:  'Playfair Display',
    source:  'Claus Eggers Sørensen',
    licence: 'SIL OFL 1.1',
    category: 'Display Serif',
    useCase:  'Title pages, cover sheets',
  },
  {
    family:  'Fraunces',
    source:  'Undercase Type',
    licence: 'SIL OFL 1.1',
    category: 'Expressive Serif',
    useCase:  'Premium, distinctive documents',
  },
  {
    family:  'DM Sans',
    source:  'Colophon Foundry',
    licence: 'SIL OFL 1.1',
    category: 'Geometric Sans',
    useCase:  'Corporate, modern',
  },
  {
    family:  'IBM Plex Sans',
    source:  'IBM — Bold Monday',
    licence: 'SIL OFL 1.1',
    category: 'Technical Sans',
    useCase:  'Specs, technical documentation',
  },
  {
    family:  'Source Code Pro',
    source:  'Adobe Systems — Paul D. Hunt',
    licence: 'SIL OFL 1.1',
    category: 'Monospace',
    useCase:  'Code references, data tables',
  },
];

/* ─── Inject @font-face rules from embedded base64 data ─────────────────── */

/**
 * Called once at startup. font-data.js must be loaded first.
 * Injects a single <style> block with all @font-face declarations
 * using base64 data URIs. This makes fonts available to the editor
 * canvas and the template CSS immediately, without any network request.
 */
function injectBuiltinFontFaces() {
  const fontData = window.WORKPLACE_FONT_DATA;
  if (!fontData) {
    console.warn('[fonts] WORKPLACE_FONT_DATA not found. Fonts will use system fallbacks.');
    console.warn('[fonts] Run: ./scripts/embed-fonts.sh');
    return;
  }

  let css = '';
  for (const [family, weights] of Object.entries(fontData)) {
    for (const [weight, styles] of Object.entries(weights)) {
      for (const [style, b64] of Object.entries(styles)) {
        css += `
@font-face {
  font-family: '${family}';
  font-weight: ${weight};
  font-style: ${style};
  src: url('data:font/woff2;base64,${b64}') format('woff2');
  font-display: block;
}`;
      }
    }
  }

  const styleEl = document.createElement('style');
  styleEl.id = 'workplace-font-faces';
  styleEl.textContent = css;
  document.head.appendChild(styleEl);

  console.info('[fonts] Injected built-in @font-face rules.');
}

/* ─── Fonts panel rendering ──────────────────────────────────────────────── */

function renderFontsPanel() {
  const container = document.getElementById('fonts-list');
  if (!container) return;

  container.innerHTML = '';

  BUILTIN_FONTS.forEach(font => {
    const item = document.createElement('div');
    item.className = 'font-item';
    item.innerHTML = `
      <div class="font-preview" style="font-family: '${font.family}', serif; font-size: 18px; line-height: 1.3; margin-bottom: 2px;">
        ${font.family}
      </div>
      <div class="font-meta">
        <span class="font-category">${font.category}</span>
        <span class="font-use">${font.useCase}</span>
      </div>
      <div class="font-licence">${font.licence} · ${font.source}</div>
    `;
    container.appendChild(item);
  });

  // Add minimal styles for the font panel items
  const panelStyle = document.getElementById('fonts-panel-style');
  if (!panelStyle) {
    const s = document.createElement('style');
    s.id = 'fonts-panel-style';
    s.textContent = `
      .font-item { padding: 10px 0; border-bottom: 1px solid #38383f; }
      .font-item:last-child { border-bottom: none; }
      .font-meta { display: flex; gap: 8px; margin-top: 2px; }
      .font-category { font-size: 10px; color: #c8a96e; text-transform: uppercase; letter-spacing: 0.05em; }
      .font-use { font-size: 10px; color: #888; }
      .font-licence { font-size: 9px; color: #555; margin-top: 2px; }
    `;
    document.head.appendChild(s);
  }
}

/* ─── Initialise ─────────────────────────────────────────────────────────── */

document.addEventListener('DOMContentLoaded', () => {
  injectBuiltinFontFaces();
  renderFontsPanel();
});

window.WorkplaceFonts = {
  BUILTIN_FONTS,
  injectBuiltinFontFaces,
};
