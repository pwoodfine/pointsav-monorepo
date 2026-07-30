// Bundle entry — re-exports every CodeMirror module the SAA editor uses
// as one global namespace `window.CMSAA`. Loaded by static/saa-init.js.
//
// Phase 2 Step 3 ships state + view + commands + language + lang-markdown.
// Phase 2 Step 4 lights up `lint` for the SAA squiggle framework.
// Phase 2 Step 5 lights up `autocomplete` for citation autocomplete.

export * as state from '@codemirror/state';
export * as view from '@codemirror/view';
export * as commands from '@codemirror/commands';
export * as language from '@codemirror/language';
export * as langMarkdown from '@codemirror/lang-markdown';
export * as lint from '@codemirror/lint';
export * as autocomplete from '@codemirror/autocomplete';
