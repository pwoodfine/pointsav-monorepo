#!/usr/bin/env node
// Regression armor — viewport overflow + landmark assertions
// Run against local wiki instances before every Stage 6 promote.
//
// Usage:
//   node scripts/responsive-check.js
//   WIKI_HOST=http://127.0.0.1:9090 node scripts/responsive-check.js
//
// Requires: node_modules in .agent/audit/2026-06-24/runner/ (playwright-core)
// Chromium: ~/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome (auto-detected)
//
// Exit 0 = all assertions pass. Exit 1 = at least one failure (details printed).

'use strict';

const path = require('path');
const { chromium } = require(
  path.resolve(__dirname, '../../../.agent/audit/2026-06-24/runner/node_modules/playwright-core')
);

const VIEWPORTS = [
  { label: '320',  w: 320,  h: 568  },
  { label: '768',  w: 768,  h: 1024 },
  { label: '1440', w: 1440, h: 900  },
];

// One instance per check run. Operator may override via env.
const INSTANCES = [
  { id: 'documentation', host: process.env.WIKI_HOST || 'http://127.0.0.1:9090', instance: 'documentation' },
  { id: 'projects',      host: 'http://127.0.0.1:9093',                           instance: 'projects'      },
  { id: 'corporate',     host: 'http://127.0.0.1:9095',                           instance: 'corporate'     },
];

// Pages to check per instance — cover home, article, category, misc chrome paths
const PAGES = [
  { path: '/',                      label: 'home'     },
  { path: '/wiki/about',            label: 'article'  },
  { path: '/special/categories',    label: 'category' },
  { path: '/special/all-pages',     label: 'all-pages'},
];

const CHROMIUM_EXEC = (function findChrome() {
  const candidates = [
    path.resolve(process.env.HOME, '.cache/ms-playwright/chromium-1223/chrome-linux64/chrome'),
    path.resolve(process.env.HOME, '.cache/ms-playwright/chromium_headless_shell-1223/chrome-linux/headless_shell'),
    '/usr/bin/google-chrome',
    '/usr/bin/chromium-browser',
  ];
  const fs = require('fs');
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  throw new Error('Chromium not found. Run: npx playwright install chromium');
}());

async function checkPage(page, url, vp, instanceId) {
  const failures = [];

  await page.setViewportSize({ width: vp.w, height: vp.h });
  try {
    await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });
  } catch (e) {
    return [{ url, vp: vp.label, rule: 'navigate', detail: e.message }];
  }

  const result = await page.evaluate((expectedInstance) => {
    const f = [];

    // R1: No horizontal overflow at this viewport width
    const scrollW = document.documentElement.scrollWidth;
    const clientW = document.documentElement.clientWidth;
    if (scrollW > clientW) {
      f.push({ rule: 'no-hscroll', detail: `scrollWidth ${scrollW} > clientWidth ${clientW}` });
    }

    // R2: <main> landmark present
    if (!document.querySelector('main')) {
      f.push({ rule: 'main-landmark', detail: 'document.querySelector("main") is null' });
    }

    // R3: <h1> heading present
    if (!document.querySelector('h1')) {
      f.push({ rule: 'h1-present', detail: 'document.querySelector("h1") is null' });
    }

    // R4: data-instance attribute matches expected tenant
    const el = document.querySelector('[data-instance]');
    if (!el) {
      f.push({ rule: 'data-instance', detail: 'no element with data-instance attribute found' });
    } else if (el.dataset.instance !== expectedInstance) {
      f.push({ rule: 'data-instance', detail: `expected "${expectedInstance}", got "${el.dataset.instance}"` });
    }

    // R5: role="banner" header present (sovereign chrome)
    if (!document.querySelector('[role="banner"]')) {
      f.push({ rule: 'role-banner', detail: 'no [role="banner"] element found' });
    }

    // R6: role="contentinfo" footer present (sovereign chrome)
    if (!document.querySelector('[role="contentinfo"]')) {
      f.push({ rule: 'role-contentinfo', detail: 'no [role="contentinfo"] element found' });
    }

    // R7: article pages — prose column and TOC list must be present
    if (document.querySelector('.article__body')) {
      if (!document.querySelector('.prose')) {
        f.push({ rule: 'article-prose', detail: '.prose column not found in article page' });
      }
      // TOC only generated when article has ≥2 headings; skip check if none present
      const hasH2 = document.querySelectorAll('.prose h2').length >= 2;
      if (hasH2 && !document.querySelector('#toc-list, .mobile-toc-list')) {
        f.push({ rule: 'article-toc', detail: '#toc-list or .mobile-toc-list not found in article page with ≥2 headings' });
      }
    }

    return f;
  }, instanceId);

  return result.map(f => ({ url, vp: vp.label, ...f }));
}

async function main() {
  const browser = await chromium.launch({ executablePath: CHROMIUM_EXEC, headless: true });
  const allFailures = [];

  for (const inst of INSTANCES) {
    const page = await browser.newPage();
    for (const pg of PAGES) {
      const url = inst.host + pg.path;
      for (const vp of VIEWPORTS) {
        process.stdout.write(`  ${inst.id} ${pg.label} @${vp.label} … `);
        const failures = await checkPage(page, url, vp, inst.instance);
        if (failures.length === 0) {
          process.stdout.write('ok\n');
        } else {
          process.stdout.write('FAIL\n');
          for (const f of failures) {
            console.log(`    [${f.rule}] ${f.detail}`);
          }
          allFailures.push(...failures);
        }
      }
    }
    await page.close();
  }

  await browser.close();

  console.log('');
  if (allFailures.length === 0) {
    console.log('✓ All viewport assertions passed.');
    process.exit(0);
  } else {
    console.log(`✗ ${allFailures.length} assertion(s) failed.`);
    process.exit(1);
  }
}

main().catch(err => { console.error(err); process.exit(1); });
