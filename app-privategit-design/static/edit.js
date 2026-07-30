// D3 — WYSIWYG edit overlay (markdown textarea + PUT save-back).
// SYS-ADR-10: operator reviews diff in textarea before confirming save.
(function () {
  var parts = location.pathname.split('/');
  var slug = parts[2], tab = parts[3];
  if (!slug || !tab) return;

  var content = document.querySelector('.content');
  if (!content) return;

  var editing = false, originalHTML = '', rawMarkdown = '', saveBtn, textarea;

  // Inject floating edit toggle button
  var btn = document.createElement('button');
  btn.id = 'edit-toggle';
  btn.textContent = 'Edit';
  applyStyle(btn, 'position:fixed;bottom:2rem;right:2rem;padding:0.5rem 1.25rem;'
    + 'background:#0050e6;color:#fff;border:none;border-radius:4px;cursor:pointer;'
    + 'font-size:0.875rem;font-weight:600;z-index:200;box-shadow:0 2px 8px rgba(0,0,0,.2)');
  document.body.appendChild(btn);

  btn.addEventListener('click', function () {
    if (!editing) enterEdit();
    else cancelEdit();
  });

  async function enterEdit() {
    var token = sessionStorage.getItem('design_edit_token');
    if (!token) {
      token = prompt('Edit token (printed to server log at startup):');
      if (!token) return;
      sessionStorage.setItem('design_edit_token', token);
    }

    var res = await fetch('/vault/elements/' + slug + '/' + tab + '/raw');
    if (!res.ok) { alert('Could not load raw content: ' + res.status); return; }
    rawMarkdown = await res.text();
    originalHTML = content.innerHTML;
    editing = true;
    btn.textContent = 'Cancel';

    // Replace rendered content with editable textarea
    textarea = document.createElement('textarea');
    textarea.value = rawMarkdown;
    applyStyle(textarea, 'width:100%;min-height:480px;padding:1rem;resize:vertical;'
      + 'font-family:"IBM Plex Mono",monospace;font-size:0.875rem;line-height:1.6;'
      + 'border:2px solid #0050e6;border-radius:4px;outline:none');
    content.innerHTML = '';
    content.appendChild(textarea);
    textarea.focus();

    saveBtn = document.createElement('button');
    saveBtn.textContent = 'Save';
    applyStyle(saveBtn, 'position:fixed;bottom:2rem;right:7.5rem;padding:0.5rem 1.25rem;'
      + 'background:#0e6027;color:#fff;border:none;border-radius:4px;cursor:pointer;'
      + 'font-size:0.875rem;font-weight:600;z-index:200;box-shadow:0 2px 8px rgba(0,0,0,.2)');
    saveBtn.addEventListener('click', doSave);
    document.body.appendChild(saveBtn);
  }

  function cancelEdit() {
    editing = false;
    content.innerHTML = originalHTML;
    btn.textContent = 'Edit';
    if (saveBtn) { saveBtn.remove(); saveBtn = null; }
    textarea = null;
  }

  async function doSave() {
    // SYS-ADR-10 human review gate: operator confirms before write
    if (!confirm('Confirm save? Your edits will be written to the vault and the page will reload.')) return;

    var token = sessionStorage.getItem('design_edit_token');
    var body  = textarea.value;

    var res = await fetch('/vault/elements/' + slug + '/' + tab, {
      method: 'PUT',
      headers: { 'Authorization': 'Bearer ' + token, 'Content-Type': 'text/markdown; charset=utf-8' },
      body: body,
    });

    if (res.ok) {
      location.reload();
    } else if (res.status === 401) {
      sessionStorage.removeItem('design_edit_token');
      alert('Token rejected — re-enter on next edit.');
    } else {
      alert('Save failed: ' + res.status);
    }
  }

  function applyStyle(el, css) { el.style.cssText = css; }
})();
