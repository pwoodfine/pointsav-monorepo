/**
 * editor.js — CodeMirror 6 editor init for /edit/* pages.
 *
 * L25: loaded only on /edit/* routes. toc-persistence.js is NOT loaded here.
 *
 * Depends on: window.CMSAA (exposed by /static/vendor/cm-saa.bundle.js).
 * Mounts onto: <textarea id="wikitext"> (pre-filled with current Markdown).
 *
 * Phase 0 stub: visual editor surface. POST submit is Phase 2 Step 3 work.
 */

'use strict';

(function () {

  function initEditor() {
    var textarea = document.getElementById('wikitext');
    if (!textarea) return;

    var CM = window.CMSAA;
    if (!CM || !CM.view || !CM.state) return;

    var startState = CM.state.EditorState.create({
      doc: textarea.value,
      extensions: [
        CM.langMarkdown.markdown(),
        CM.view.lineNumbers(),
        CM.view.highlightActiveLine(),
      ],
    });

    var editorView = new CM.view.EditorView({
      state: startState,
      parent: textarea.parentNode,
    });

    // Hide native textarea; keep in sync so form submits correctly.
    textarea.style.display = 'none';

    editorView.dom.addEventListener('input', function () {
      textarea.value = editorView.state.doc.toString();
    });
  }

  document.addEventListener('DOMContentLoaded', initEditor);

}());
