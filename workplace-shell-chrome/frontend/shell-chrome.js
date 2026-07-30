/*
 * workplace-shell-chrome — frontend shell chrome for the workplace app family.
 * Copyright © 2026 PointSav Digital Systems. Apache-2.0.
 *
 * Framework-free, no build step, no runtime dependencies. Load with a plain
 * <script src="shell-chrome.js"></script> (after the DOM's body element exists)
 * and its companion shell-chrome.css. Exposes a single global: window.ShellChrome.
 *
 * Four pieces (BRIEF R4/B1):
 *   (i)   command registry     — flat { id, title, category, shortcut?, when?, run } list
 *   (ii)  command palette       — Ctrl/Cmd+Shift+P overlay; ARIA combobox/listbox;
 *                                 focus trap; Escape; subsequence filter; MRU-boosted sort
 *   (iii) keybinding dispatch   — declarative { key, command, when? } table over the registry
 *   (iv)  status / error toast  — one aria-live region, kind = info | success | error
 *
 * "when" is deliberately NOT an expression language: a comma-separated list of
 * context keys that must be truthy, each optionally negated with a leading "!".
 * e.g. "editor_focused, !palette_open". Evaluated against the context object that
 * the host app keeps current via ShellChrome.setContext({ ... }).
 */
(function (global) {
  "use strict";

  var IS_MAC =
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent || "");

  // ── Context ────────────────────────────────────────────────────────────────
  // A small, explicit bag of booleans the host app updates as its state changes.
  var context = Object.create(null);
  context.palette_open = false;

  function setContext(patch) {
    if (patch && typeof patch === "object") {
      for (var k in patch) {
        if (Object.prototype.hasOwnProperty.call(patch, k)) context[k] = patch[k];
      }
    }
    return context;
  }
  function getContext() {
    var out = Object.create(null);
    for (var k in context) {
      if (Object.prototype.hasOwnProperty.call(context, k)) out[k] = context[k];
    }
    return out;
  }

  // "editor_focused, !palette_open" → all clauses must hold.
  function evalWhen(when, ctx) {
    if (when == null || when === "") return true;
    if (typeof when === "function") return !!when(ctx);
    var clauses = String(when).split(",");
    for (var i = 0; i < clauses.length; i++) {
      var c = clauses[i].trim();
      if (!c) continue;
      var negate = c.charAt(0) === "!";
      var key = negate ? c.slice(1).trim() : c;
      var val = !!ctx[key];
      if (negate ? val : !val) return false;
    }
    return true;
  }

  // ── Command registry ─────────────────────────────────────────────────────────
  // command = { id, title, category?, shortcut?, when?, run }
  var commands = new Map();
  var mru = []; // command ids, most-recent first
  var MRU_MAX = 32;

  function registerCommand(cmd) {
    if (!cmd || typeof cmd.id !== "string" || !cmd.id) {
      throw new Error("ShellChrome.registerCommand: command needs a string id");
    }
    if (typeof cmd.run !== "function") {
      throw new Error("ShellChrome.registerCommand: command '" + cmd.id + "' needs a run() function");
    }
    commands.set(cmd.id, {
      id: cmd.id,
      title: cmd.title || cmd.id,
      category: cmd.category || "",
      shortcut: cmd.shortcut || "",
      when: cmd.when,
      run: cmd.run,
    });
    return cmd.id;
  }
  function registerCommands(list) {
    (list || []).forEach(registerCommand);
  }
  function unregisterCommand(id) {
    commands.delete(id);
  }
  function getCommand(id) {
    return commands.get(id) || null;
  }
  // Commands currently runnable given context, in stable insertion order.
  function listCommands() {
    var ctx = getContext();
    var out = [];
    commands.forEach(function (c) {
      if (evalWhen(c.when, ctx)) out.push(c);
    });
    return out;
  }

  function bumpMru(id) {
    var i = mru.indexOf(id);
    if (i !== -1) mru.splice(i, 1);
    mru.unshift(id);
    if (mru.length > MRU_MAX) mru.length = MRU_MAX;
  }

  function runCommand(id) {
    var c = commands.get(id);
    if (!c) return false;
    if (!evalWhen(c.when, getContext())) return false;
    bumpMru(id);
    try {
      c.run();
    } catch (e) {
      toast("Command failed: " + (e && e.message ? e.message : e), { kind: "error" });
      return false;
    }
    return true;
  }

  // ── Keybinding dispatch ───────────────────────────────────────────────────────
  // binding = { key: "Mod+Shift+P", command: "id", when? }
  // "Mod" resolves to Cmd on macOS, Ctrl elsewhere. Order: Mod, Ctrl, Alt, Shift, KEY.
  var bindings = [];

  function normalizeKeyString(key) {
    var parts = String(key).split("+").map(function (p) { return p.trim(); });
    var mods = { mod: false, ctrl: false, alt: false, shift: false };
    var main = "";
    parts.forEach(function (p) {
      var low = p.toLowerCase();
      if (low === "mod" || low === "cmd" || low === "meta" || low === "super") mods.mod = true;
      else if (low === "ctrl" || low === "control") mods.ctrl = true;
      else if (low === "alt" || low === "option") mods.alt = true;
      else if (low === "shift") mods.shift = true;
      else main = low;
    });
    return canonical(mods.mod, mods.ctrl, mods.alt, mods.shift, main);
  }

  function canonical(mod, ctrl, alt, shift, mainKey) {
    var out = [];
    if (mod) out.push("Mod");
    if (ctrl) out.push("Ctrl");
    if (alt) out.push("Alt");
    if (shift) out.push("Shift");
    out.push(mainKey);
    return out.join("+");
  }

  function eventToKeyString(e) {
    var main = (e.key || "").toLowerCase();
    // Normalise a few names so bindings can spell them naturally.
    if (main === " " || main === "spacebar") main = "space";
    if (main === "escape") main = "esc";
    // The platform's primary modifier: Cmd on mac, Ctrl elsewhere → "Mod".
    var mod = IS_MAC ? e.metaKey : e.ctrlKey;
    var ctrl = IS_MAC ? e.ctrlKey : false; // a literal Ctrl only counts as distinct on mac
    // Do not let the main key double-report as a modifier.
    if (main === "control" || main === "meta" || main === "alt" || main === "shift") return null;
    return canonical(mod, ctrl, e.altKey, e.shiftKey, main);
  }

  function registerKeybinding(b) {
    if (!b || typeof b.key !== "string" || typeof b.command !== "string") {
      throw new Error("ShellChrome.registerKeybinding: needs { key, command }");
    }
    bindings.push({ key: normalizeKeyString(b.key), command: b.command, when: b.when });
    return b;
  }
  function registerKeybindings(list) {
    (list || []).forEach(registerKeybinding);
  }

  function handleKeydown(e) {
    // The palette owns the keyboard while open (except its own dismiss/run keys,
    // handled by its input listener).
    var ks = eventToKeyString(e);
    if (!ks) return;
    var ctx = getContext();
    for (var i = 0; i < bindings.length; i++) {
      var b = bindings[i];
      if (b.key !== ks) continue;
      if (!evalWhen(b.when, ctx)) continue;
      var cmd = commands.get(b.command);
      if (!cmd || !evalWhen(cmd.when, ctx)) continue;
      e.preventDefault();
      e.stopPropagation();
      runCommand(b.command);
      return;
    }
  }

  // ── Palette overlay ───────────────────────────────────────────────────────────
  var paletteRoot = null, paletteInput = null, paletteList = null, paletteEmpty = null, paletteStatus = null;
  var paletteItems = [];   // current filtered [{cmd, ...}]
  var paletteActive = -1;  // index into paletteItems
  var prevFocus = null;

  function ensurePalette() {
    if (paletteRoot) return;
    var root = document.createElement("div");
    root.className = "shellchrome-palette";
    root.setAttribute("hidden", "");
    root.innerHTML =
      '<div class="shellchrome-palette__backdrop" data-shellchrome-dismiss></div>' +
      '<div class="shellchrome-palette__panel" role="dialog" aria-modal="true" aria-label="Command palette">' +
      '  <input class="shellchrome-palette__input" type="text" role="combobox" autocomplete="off" ' +
      '         spellcheck="false" aria-expanded="true" aria-controls="shellchrome-palette-list" ' +
      '         aria-activedescendant="" placeholder="Type a command…" aria-label="Command palette search">' +
      '  <ul class="shellchrome-palette__list" id="shellchrome-palette-list" role="listbox" aria-label="Commands"></ul>' +
      '  <div class="shellchrome-palette__empty" hidden>No matching commands</div>' +
      '  <div class="shellchrome-palette__status shellchrome-sr-only" role="status" aria-live="polite"></div>' +
      "</div>";
    document.body.appendChild(root);

    paletteRoot = root;
    paletteInput = root.querySelector(".shellchrome-palette__input");
    paletteList = root.querySelector(".shellchrome-palette__list");
    paletteEmpty = root.querySelector(".shellchrome-palette__empty");
    paletteStatus = root.querySelector(".shellchrome-palette__status");

    paletteInput.addEventListener("input", function () {
      refreshPalette(paletteInput.value);
    });
    paletteInput.addEventListener("keydown", onPaletteKeydown);
    root.addEventListener("mousedown", function (e) {
      if (e.target && e.target.hasAttribute("data-shellchrome-dismiss")) closePalette();
    });
    paletteList.addEventListener("mousedown", function (e) {
      // mousedown (not click) so the input doesn't blur first.
      var li = e.target.closest ? e.target.closest("[data-index]") : null;
      if (!li) return;
      e.preventDefault();
      var idx = parseInt(li.getAttribute("data-index"), 10);
      chooseActive(idx);
    });
  }

  function scoreMatch(query, text) {
    // Case-insensitive subsequence match. Returns null if not a subsequence,
    // else a score where lower is better (earlier + more contiguous wins).
    var q = query.toLowerCase(), t = text.toLowerCase();
    if (!q) return 0;
    var ti = 0, score = 0, lastHit = -2, firstHit = -1;
    for (var qi = 0; qi < q.length; qi++) {
      var ch = q[qi];
      var found = t.indexOf(ch, ti);
      if (found === -1) return null;
      if (firstHit === -1) firstHit = found;
      score += found - lastHit === 1 ? 0 : 2; // contiguity bonus
      lastHit = found;
      ti = found + 1;
    }
    return score + firstHit; // prefer earlier first hit
  }

  function refreshPalette(query) {
    var ctx = getContext();
    var scored = [];
    commands.forEach(function (c) {
      if (!evalWhen(c.when, ctx)) return;
      var label = (c.category ? c.category + ": " : "") + c.title;
      var s = scoreMatch(query || "", label);
      if (s === null) return;
      var mruIdx = mru.indexOf(c.id);
      var mruBoost = mruIdx === -1 ? 0 : -(MRU_MAX - mruIdx) * 0.01; // recent → slightly better
      scored.push({ cmd: c, label: label, score: s + mruBoost, mruIdx: mruIdx });
    });
    scored.sort(function (a, b) {
      if (a.score !== b.score) return a.score - b.score;
      return a.label.localeCompare(b.label);
    });
    paletteItems = scored;
    paletteActive = scored.length ? 0 : -1;
    renderPalette();
  }

  function renderPalette() {
    paletteList.innerHTML = "";
    if (!paletteItems.length) {
      paletteEmpty.hidden = false;
      paletteInput.setAttribute("aria-activedescendant", "");
      announce("No matching commands");
      return;
    }
    paletteEmpty.hidden = true;
    var frag = document.createDocumentFragment();
    paletteItems.forEach(function (item, i) {
      var li = document.createElement("li");
      li.className = "shellchrome-palette__item" + (i === paletteActive ? " is-active" : "");
      li.id = "shellchrome-cmd-" + i;
      li.setAttribute("role", "option");
      li.setAttribute("data-index", String(i));
      li.setAttribute("aria-selected", i === paletteActive ? "true" : "false");
      var title = document.createElement("span");
      title.className = "shellchrome-palette__title";
      title.textContent = item.label;
      li.appendChild(title);
      if (item.cmd.shortcut) {
        var kbd = document.createElement("kbd");
        kbd.className = "shellchrome-palette__shortcut";
        kbd.textContent = item.cmd.shortcut;
        li.appendChild(kbd);
      }
      frag.appendChild(li);
    });
    paletteList.appendChild(frag);
    updateActiveDescendant();
    announce(paletteItems.length + (paletteItems.length === 1 ? " command" : " commands"));
  }

  function updateActiveDescendant() {
    var items = paletteList.children;
    for (var i = 0; i < items.length; i++) {
      var on = i === paletteActive;
      items[i].classList.toggle("is-active", on);
      items[i].setAttribute("aria-selected", on ? "true" : "false");
    }
    if (paletteActive >= 0 && items[paletteActive]) {
      paletteInput.setAttribute("aria-activedescendant", items[paletteActive].id);
      items[paletteActive].scrollIntoView({ block: "nearest" });
    } else {
      paletteInput.setAttribute("aria-activedescendant", "");
    }
  }

  function moveActive(delta) {
    if (!paletteItems.length) return;
    paletteActive = (paletteActive + delta + paletteItems.length) % paletteItems.length;
    updateActiveDescendant();
  }

  function chooseActive(idx) {
    if (typeof idx === "number") paletteActive = idx;
    if (paletteActive < 0 || paletteActive >= paletteItems.length) return;
    var id = paletteItems[paletteActive].cmd.id;
    closePalette();
    runCommand(id);
  }

  function onPaletteKeydown(e) {
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); moveActive(1); break;
      case "ArrowUp": e.preventDefault(); moveActive(-1); break;
      case "Home": e.preventDefault(); paletteActive = 0; updateActiveDescendant(); break;
      case "End": e.preventDefault(); paletteActive = paletteItems.length - 1; updateActiveDescendant(); break;
      case "Enter": e.preventDefault(); chooseActive(); break;
      case "Escape": e.preventDefault(); closePalette(); break;
      case "Tab": e.preventDefault(); break; // focus trap: single control, keep focus here
      default: break;
    }
  }

  function announce(msg) {
    if (paletteStatus) paletteStatus.textContent = msg;
  }

  function openPalette() {
    ensurePalette();
    if (!paletteRoot.hasAttribute("hidden")) return;
    prevFocus = document.activeElement;
    paletteRoot.removeAttribute("hidden");
    paletteInput.value = "";
    setContext({ palette_open: true });
    refreshPalette("");
    paletteInput.focus();
  }

  function closePalette() {
    if (!paletteRoot || paletteRoot.hasAttribute("hidden")) return;
    paletteRoot.setAttribute("hidden", "");
    setContext({ palette_open: false });
    if (prevFocus && typeof prevFocus.focus === "function") prevFocus.focus();
    prevFocus = null;
  }

  function togglePalette() {
    if (paletteRoot && !paletteRoot.hasAttribute("hidden")) closePalette();
    else openPalette();
  }

  // ── Status / error toast ──────────────────────────────────────────────────────
  var toastRoot = null, toastTimer = null;

  function ensureToast() {
    if (toastRoot) return;
    toastRoot = document.createElement("div");
    toastRoot.className = "shellchrome-toast";
    toastRoot.setAttribute("role", "status");
    toastRoot.setAttribute("aria-live", "polite");
    toastRoot.setAttribute("hidden", "");
    document.body.appendChild(toastRoot);
  }

  function toast(message, opts) {
    ensureToast();
    opts = opts || {};
    var kind = opts.kind || "info"; // info | success | error
    toastRoot.className = "shellchrome-toast shellchrome-toast--" + kind;
    // Errors get assertive delivery so a screen reader interrupts for them.
    toastRoot.setAttribute("aria-live", kind === "error" ? "assertive" : "polite");
    toastRoot.textContent = message;
    toastRoot.removeAttribute("hidden");
    if (toastTimer) clearTimeout(toastTimer);
    var timeout = typeof opts.timeout === "number" ? opts.timeout : kind === "error" ? 6000 : 3000;
    if (timeout > 0) {
      toastTimer = setTimeout(function () {
        toastRoot.setAttribute("hidden", "");
      }, timeout);
    }
  }

  // ── Install ─────────────────────────────────────────────────────────────────
  var installed = false;

  function install(opts) {
    opts = opts || {};
    if (!installed) {
      document.addEventListener("keydown", handleKeydown, true); // capture: beat app if-chains
      installed = true;
    }
    // A built-in palette command + its default binding, unless the host opts out.
    if (opts.paletteCommand !== false && !commands.has("shellchrome.palette")) {
      registerCommand({
        id: "shellchrome.palette",
        title: "Show All Commands",
        category: "Chrome",
        shortcut: IS_MAC ? "⌘⇧P" : "Ctrl+Shift+P",
        run: togglePalette,
      });
    }
    if (opts.paletteBinding !== false) {
      registerKeybinding({ key: "Mod+Shift+P", command: "shellchrome.palette" });
    }
    return API;
  }

  var API = {
    // context
    setContext: setContext,
    getContext: getContext,
    // registry
    registerCommand: registerCommand,
    registerCommands: registerCommands,
    unregisterCommand: unregisterCommand,
    getCommand: getCommand,
    listCommands: listCommands,
    runCommand: runCommand,
    // keybindings
    registerKeybinding: registerKeybinding,
    registerKeybindings: registerKeybindings,
    // palette
    openPalette: openPalette,
    closePalette: closePalette,
    togglePalette: togglePalette,
    // toast
    toast: toast,
    // lifecycle
    install: install,
    // internals exposed for tests
    _evalWhen: evalWhen,
    _normalizeKeyString: normalizeKeyString,
    _scoreMatch: scoreMatch,
  };

  global.ShellChrome = API;
})(typeof window !== "undefined" ? window : this);
