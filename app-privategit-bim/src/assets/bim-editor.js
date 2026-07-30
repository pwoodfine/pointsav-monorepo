// bim-editor.js — save logic for /edit/:slug pages
// Loaded as a module script by render::editor::render_editor_panel

(function () {
  const slug = document.querySelector('.bim-editor')?.dataset.slug;
  if (!slug) return;

  const saveBtn = document.getElementById('bim-save-btn');
  const statusEl = document.getElementById('bim-save-status');
  const switcher = document.getElementById('bim-mode-switcher');
  const visualPane = document.getElementById('bim-visual-pane');
  const codePane = document.getElementById('bim-code-pane');

  // Mode switcher
  if (switcher) {
    switcher.addEventListener('cds-content-switcher-selected', (e) => {
      const mode = e.detail?.item?.getAttribute('value');
      if (mode === 'code') {
        visualPane?.setAttribute('hidden', '');
        codePane?.removeAttribute('hidden');
      } else {
        codePane?.setAttribute('hidden', '');
        visualPane?.removeAttribute('hidden');
      }
    });
  }

  // Save button: two-phase (dry-run → confirm)
  let pendingPayload = null;

  if (saveBtn) {
    saveBtn.addEventListener('click', async () => {
      const payload = getPayload();
      if (!payload) return;

      // Phase 1: dry-run validation
      showStatus('info', 'Validating…');
      const dryRes = await fetch(`/edit/${slug}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const dryData = await dryRes.json();

      if (!dryData.valid && dryData.errors?.length) {
        showStatus('error', 'Validation failed: ' + dryData.errors.slice(0, 2).join('; '));
        pendingPayload = null;
        return;
      }

      if (dryData.dry_run) {
        pendingPayload = payload;
        showStatus('warning', 'Validation passed. Click Save again to write the file. (F12 gate)');
        saveBtn.textContent = 'Confirm Save';
        return;
      }

      // Phase 2: confirmed write
      if (pendingPayload) {
        const writeRes = await fetch(`/edit/${slug}?confirm=1`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(pendingPayload),
        });
        const writeData = await writeRes.json();
        pendingPayload = null;
        saveBtn.textContent = 'Save';
        if (writeData.saved) {
          showStatus('success', `Saved to ${writeData.file}`);
        } else {
          showStatus('error', writeData.error || 'Save failed');
        }
      }
    });
  }

  function getPayload() {
    // Try to get JSON from the CodeMirror editor if it's visible
    const editor = window.BimEditor;
    if (editor && editor.view && !codePane?.hasAttribute('hidden')) {
      try {
        return JSON.parse(editor.view.state.doc.toString());
      } catch (_) {
        showStatus('error', 'JSON parse error — check the code editor for syntax errors');
        return null;
      }
    }
    // Fall back to SchemaState
    return window.SchemaState?.data || null;
  }

  function showStatus(kind, text) {
    if (!statusEl) return;
    statusEl.style.display = '';
    statusEl.setAttribute('kind', kind);
    statusEl.setAttribute('subtitle', text);
  }
})();
