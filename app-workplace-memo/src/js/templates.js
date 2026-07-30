/**
 * Workplace*Memo — templates.js
 * Template definitions: each template is a CSS string + page geometry.
 * Switching templates is a CSS swap — no content is modified.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 */

'use strict';

const TEMPLATES = {

  corporate: {
    label: 'Corporate Memo',
    fonts: ['EB Garamond', 'DM Sans'],
    pageSize: 'A4',
    marginsMm: 25,
    css: `
      body, #document-canvas {
        font-family: 'DM Sans', 'Helvetica Neue', Arial, sans-serif;
        font-size: 11pt;
        color: #1a1a1a;
        line-height: 1.7;
      }
      h1 {
        font-family: 'EB Garamond', Georgia, serif;
        font-size: 22pt;
        font-weight: 600;
        color: #0a0a0a;
        border-bottom: 2px solid #c8a96e;
        padding-bottom: 6px;
        margin-bottom: 20px;
        margin-top: 0;
      }
      h2 {
        font-family: 'EB Garamond', Georgia, serif;
        font-size: 15pt;
        font-weight: 600;
        color: #1a1a1a;
        margin-top: 26px;
        margin-bottom: 8px;
      }
      h3 {
        font-size: 11pt;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: #555;
        margin-top: 20px;
        margin-bottom: 6px;
      }
      h4, h5, h6 {
        font-size: 10pt;
        font-weight: 600;
        color: #333;
        margin-bottom: 4px;
      }
      p { margin: 0 0 10px; }
      ul, ol { padding-left: 20px; margin: 8px 0; }
      li { margin-bottom: 4px; }
      code {
        background: #f0ece4;
        padding: 1px 5px;
        border-radius: 3px;
        font-size: 9.5pt;
        font-family: 'Source Code Pro', monospace;
      }
      pre {
        background: #f5f1ea;
        border-left: 3px solid #c8a96e;
        padding: 12px 16px;
        border-radius: 0 4px 4px 0;
        overflow-x: auto;
        margin: 14px 0;
        font-family: 'Source Code Pro', monospace;
        font-size: 9pt;
      }
      pre code { background: none; padding: 0; }
      blockquote {
        border-left: 3px solid #c8a96e;
        margin: 14px 0;
        padding: 8px 16px;
        color: #555;
        font-style: italic;
      }
      hr { border: none; border-top: 1px solid #d8d0c0; margin: 20px 0; }
      a { color: #8b5c1a; }
      strong { font-weight: 600; }
      table { border-collapse: collapse; width: 100%; margin: 14px 0; }
      th, td { border: 1px solid #d8d0c0; padding: 7px 12px; text-align: left; font-size: 10pt; }
      th { background: #f5f1ea; font-weight: 600; }
    `,
  },

  technical: {
    label: 'Technical Spec',
    fonts: ['IBM Plex Sans', 'Source Code Pro'],
    pageSize: 'A4',
    marginsMm: 25,
    css: `
      body, #document-canvas {
        font-family: 'IBM Plex Sans', 'Helvetica Neue', Arial, sans-serif;
        font-size: 10.5pt;
        color: #111827;
        line-height: 1.65;
      }
      h1 {
        font-size: 20pt;
        font-weight: 700;
        color: #0d1117;
        border-bottom: 1px solid #e5e7eb;
        padding-bottom: 8px;
        margin-bottom: 18px;
        margin-top: 0;
      }
      h2 {
        font-size: 13pt;
        font-weight: 600;
        color: #111827;
        margin-top: 26px;
        padding-top: 4px;
        border-top: 1px solid #f3f4f6;
        margin-bottom: 8px;
      }
      h3 {
        font-size: 10.5pt;
        font-weight: 600;
        color: #374151;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        margin-top: 18px;
        margin-bottom: 6px;
      }
      h4, h5, h6 { font-size: 10pt; font-weight: 600; margin-bottom: 4px; }
      p { margin: 0 0 10px; }
      ul, ol { padding-left: 22px; margin: 8px 0; }
      li { margin-bottom: 3px; }
      code {
        font-family: 'Source Code Pro', monospace;
        font-size: 9pt;
        background: #f3f4f6;
        padding: 1px 5px;
        border-radius: 3px;
        border: 1px solid #e5e7eb;
      }
      pre {
        background: #0d1117;
        color: #c9d1d9;
        border-radius: 6px;
        padding: 14px 18px;
        overflow-x: auto;
        margin: 14px 0;
        border: 1px solid #30363d;
        font-family: 'Source Code Pro', monospace;
      }
      pre code { background: none; border: none; color: inherit; font-size: 8.5pt; }
      blockquote {
        background: #f0f9ff;
        border-left: 3px solid #0ea5e9;
        padding: 10px 14px;
        margin: 14px 0;
        border-radius: 0 4px 4px 0;
      }
      hr { border: none; border-top: 1px solid #e5e7eb; margin: 22px 0; }
      a { color: #0ea5e9; text-decoration: none; border-bottom: 1px solid #bae6fd; }
      strong { font-weight: 600; color: #0d1117; }
      table { border-collapse: collapse; width: 100%; margin: 14px 0; font-size: 9.5pt; }
      th, td { border: 1px solid #e5e7eb; padding: 6px 10px; text-align: left; }
      th { background: #f9fafb; font-weight: 600; }
      tr:nth-child(even) td { background: #f9fafb; }
    `,
  },

  minimal: {
    label: 'Minimal Report',
    fonts: ['Lora', 'DM Sans'],
    pageSize: 'A4',
    marginsMm: 30,
    css: `
      body, #document-canvas {
        font-family: 'DM Sans', 'Helvetica Neue', Arial, sans-serif;
        font-weight: 300;
        font-size: 11pt;
        color: #2c2c2c;
        line-height: 1.8;
      }
      h1 {
        font-family: 'Lora', Georgia, serif;
        font-size: 26pt;
        font-weight: 600;
        color: #111;
        letter-spacing: -0.02em;
        margin-bottom: 6px;
        margin-top: 0;
        border: none;
      }
      h2 {
        font-family: 'Lora', Georgia, serif;
        font-size: 16pt;
        font-weight: 400;
        font-style: italic;
        color: #333;
        margin-top: 28px;
        margin-bottom: 8px;
      }
      h3 {
        font-size: 10pt;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.12em;
        color: #888;
        margin-top: 22px;
        margin-bottom: 6px;
      }
      h4, h5, h6 { font-size: 10pt; font-weight: 700; margin-bottom: 4px; }
      p { margin: 0 0 12px; }
      ul, ol { padding-left: 18px; margin: 10px 0; }
      li { margin-bottom: 5px; }
      code {
        font-family: 'Source Code Pro', monospace;
        font-size: 9pt;
        background: #f5f5f5;
        padding: 2px 5px;
        border-radius: 2px;
      }
      pre {
        background: #f5f5f5;
        padding: 14px 18px;
        border-radius: 2px;
        overflow-x: auto;
        margin: 14px 0;
        font-family: 'Source Code Pro', monospace;
        font-size: 9pt;
      }
      pre code { background: none; }
      blockquote {
        color: #666;
        font-style: italic;
        border-left: 2px solid #ccc;
        padding: 6px 14px;
        margin: 14px 0;
      }
      hr { border: none; border-top: 1px solid #ddd; margin: 24px 0; }
      a { color: #2c2c2c; text-decoration: underline; }
      strong { font-weight: 700; }
      table { border-collapse: collapse; width: 100%; margin: 14px 0; }
      th, td { border-bottom: 1px solid #ddd; padding: 8px 0; text-align: left; font-size: 10.5pt; }
      th { font-weight: 700; font-size: 9pt; text-transform: uppercase; letter-spacing: 0.05em; color: #888; }
    `,
  },

  confidential: {
    label: 'Confidential Brief',
    fonts: ['Playfair Display', 'DM Sans'],
    pageSize: 'A4',
    marginsMm: 28,
    css: `
      body, #document-canvas {
        font-family: 'DM Sans', 'Helvetica Neue', Arial, sans-serif;
        font-size: 10.5pt;
        color: #1c1c1c;
        line-height: 1.75;
      }
      h1 {
        font-family: 'Playfair Display', Georgia, serif;
        font-size: 24pt;
        color: #0c0c0c;
        margin-bottom: 4px;
        margin-top: 0;
      }
      h2 {
        font-family: 'Playfair Display', Georgia, serif;
        font-size: 14pt;
        color: #111;
        border-bottom: 1px solid #222;
        padding-bottom: 4px;
        margin-top: 26px;
        margin-bottom: 10px;
      }
      h3 {
        font-size: 10pt;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.1em;
        color: #444;
        margin-top: 18px;
        margin-bottom: 6px;
      }
      h4, h5, h6 { font-size: 10pt; font-weight: 600; margin-bottom: 4px; }
      p { margin: 0 0 10px; }
      ul, ol { padding-left: 20px; margin: 8px 0; }
      li { margin-bottom: 4px; }
      code {
        font-family: 'Source Code Pro', monospace;
        font-size: 9pt;
        background: #efefef;
        padding: 1px 4px;
      }
      pre {
        background: #efefef;
        padding: 12px 16px;
        overflow-x: auto;
        margin: 12px 0;
        border-left: 4px solid #222;
        font-family: 'Source Code Pro', monospace;
        font-size: 9pt;
      }
      pre code { background: none; }
      blockquote {
        border-left: 3px solid #333;
        padding: 8px 14px;
        margin: 14px 0;
        font-style: italic;
        color: #444;
        background: #f8f8f8;
      }
      hr { border: none; border-top: 2px solid #1c1c1c; margin: 20px 0; }
      a { color: #1c1c1c; font-weight: 600; }
      strong { font-weight: 600; }
      table { border-collapse: collapse; width: 100%; margin: 14px 0; }
      th, td { border: 1px solid #ccc; padding: 7px 10px; text-align: left; font-size: 10pt; }
      th { background: #f0f0f0; font-weight: 600; text-transform: uppercase; font-size: 9pt; }
    `,
  },

};

/* ─── Style injection ────────────────────────────────────────────────────── */

let _templateStyleEl = null;

function applyTemplate(key) {
  const tpl = TEMPLATES[key];
  if (!tpl) {
    console.warn(`[templates] Unknown template key: ${key}`);
    return;
  }

  // Inject / replace the template style element
  if (!_templateStyleEl) {
    _templateStyleEl = document.createElement('style');
    _templateStyleEl.id = 'workplace-template-style';
    document.head.appendChild(_templateStyleEl);
  }
  _templateStyleEl.textContent = tpl.css;

  // Register that this template is active
  document.body.dataset.template = key;

  console.info(`[templates] Applied: ${tpl.label}`);
}

/* ─── Apply default template on load ────────────────────────────────────── */

applyTemplate('corporate');

/* ─── Public API ─────────────────────────────────────────────────────────── */

window.WorkplaceTemplates = {
  TEMPLATES,
  apply: applyTemplate,
  get: (key) => TEMPLATES[key] || null,
  getActiveCSS: () => _templateStyleEl ? _templateStyleEl.textContent : '',
};
