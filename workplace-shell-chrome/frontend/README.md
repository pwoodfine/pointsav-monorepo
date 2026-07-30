# workplace-shell-chrome — frontend chrome (B1)

Framework-free shell chrome for the browser-based workplace apps
(`app-privategit-workbench`, `app-workplace-http-prototype`, and the
`app-workplace-*` Tauri frontends). No build step, no runtime dependencies.

This is the **frontend** half of `workplace-shell-chrome`. The sibling Rust
crate (`../src/`) is a separate, backend-only concern (the `get_X`/`set_X`
config-store triad); the two share a name and a design lineage but no code.
See `BRIEF-workplace-institutional-quality-roadmap.md` §R4 / Track B for why
this exists (it consolidates the command-palette / keybinding / status-toast
patterns the apps had each independently reinvented).

## Files

| File | What |
|---|---|
| `shell-chrome.js` | The module. Exposes `window.ShellChrome`. |
| `shell-chrome.css` | Companion styles (namespaced `.shellchrome-*`; themeable via `--shellchrome-*` custom properties). |
| `selftest.js` | Headless `node selftest.js` — covers the pure logic (registry, `when`, key normalisation, fuzzy scoring, MRU). |

## Usage

```html
<link rel="stylesheet" href="shell-chrome.css">
<script src="shell-chrome.js"></script>
<script>
  ShellChrome.install();                 // attaches the global keydown listener +
                                         // the built-in "Show All Commands" (Ctrl/Cmd+Shift+P)

  ShellChrome.registerCommands([
    { id: "file.open",  title: "Open File",  category: "File",
      shortcut: "Ctrl+O", run: openFile },
    { id: "file.save",  title: "Save",       category: "File",
      shortcut: "Ctrl+S", when: "editor_focused", run: saveFile },
  ]);

  ShellChrome.registerKeybindings([
    { key: "Ctrl+O", command: "file.open" },
    { key: "Ctrl+S", command: "file.save", when: "editor_focused" },
  ]);

  // Keep the context current as app state changes; `when` resolves against it.
  ShellChrome.setContext({ editor_focused: true, has_selection: false });

  ShellChrome.toast("Saved.", { kind: "success" });
</script>
```

### `when`

Not an expression language — a comma-separated list of context keys that must be
truthy, each optionally negated with a leading `!`. Example:
`"editor_focused, !palette_open"`. A function `when(ctx) -> bool` is also accepted.

### Keybindings

`key` uses `Mod` for the platform-primary modifier (⌘ on macOS, Ctrl elsewhere),
plus `Ctrl` / `Alt` / `Shift`. The main key is matched case-insensitively against
`event.key`. The listener is installed in **capture** phase so it wins over an
app's existing `keydown` if-chains — the B2 retrofit is to move each existing
hardcoded shortcut into the registry as its canonical definition.

## Verification status

- ✅ `node --check shell-chrome.js` — clean.
- ✅ `node selftest.js` — all pass. Covers: `when` evaluation, key normalisation,
  fuzzy/subsequence scoring, registry register/list/get, `runCommand` gating,
  MRU, and the throwing-`run()` catch path.
- ⬜ **Palette + toast DOM behaviour is NOT covered here** (they touch `document`).
  Focus trap, `aria-activedescendant`, Escape, backdrop dismiss, and toast
  `aria-live` need an operator browser pass — do this as part of the B2 retrofit
  into the first consuming app.

## Accessibility

The palette is an ARIA combobox (`role=combobox` input + `role=listbox` +
`role=option` items, `aria-activedescendant`, `aria-selected`), traps focus while
open, closes on Escape and restores prior focus, and announces result counts via a
visually-hidden `role=status`. The toast is `aria-live` (`assertive` for errors,
`polite` otherwise). This carries forward the ARIA lessons from the
`app-privategit-workbench` C3 pass (BRIEF §S5).
