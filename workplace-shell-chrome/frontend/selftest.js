/*
 * Headless self-test for shell-chrome.js — exercises the pure logic (registry,
 * when-evaluation, keybinding normalisation, fuzzy scoring, MRU) with no DOM.
 * The palette/toast paths touch `document` and are not covered here; they need
 * an operator browser pass (see frontend/README.md).
 *
 * Run: node selftest.js   (exit 0 = all pass)
 */
"use strict";

// Load shell-chrome.js. It guards `navigator` and only touches `document` inside
// DOM functions, so loading is DOM-free. In CommonJS the IIFE's top-level `this`
// is module.exports, so ShellChrome is attached to the require() return value.
// (In a browser, top-level `this` is `window`, so there it attaches to window.)
var SC = require("./shell-chrome.js").ShellChrome;

var failures = 0;
function ok(cond, msg) {
  if (!cond) { failures++; console.error("FAIL: " + msg); }
  else console.log("pass: " + msg);
}

// ── when-evaluation ──
ok(SC._evalWhen("", {}) === true, "empty when is always true");
ok(SC._evalWhen("editor_focused", { editor_focused: true }) === true, "single truthy clause");
ok(SC._evalWhen("editor_focused", { editor_focused: false }) === false, "single falsy clause");
ok(SC._evalWhen("!palette_open", { palette_open: false }) === true, "negated clause");
ok(SC._evalWhen("a, !b", { a: true, b: false }) === true, "AND of clauses passes");
ok(SC._evalWhen("a, !b", { a: true, b: true }) === false, "AND of clauses fails on negation");
ok(SC._evalWhen(function (c) { return c.x === 1; }, { x: 1 }) === true, "function when supported");

// ── key normalisation (main key is lowercased so it matches event.key) ──
ok(SC._normalizeKeyString("Mod+Shift+P") === "Mod+Shift+p", "canonical order preserved (key lowercased)");
ok(SC._normalizeKeyString("shift+mod+p") === "Mod+Shift+p", "modifier order normalised");
ok(SC._normalizeKeyString("Cmd+K") === "Mod+k", "Cmd → Mod, key lowercased");
ok(SC._normalizeKeyString("Ctrl+Alt+Delete") === "Ctrl+Alt+delete", "multi-mod");

// ── fuzzy scoring ──
ok(SC._scoreMatch("", "anything") === 0, "empty query scores 0");
ok(SC._scoreMatch("xyz", "abc") === null, "non-subsequence returns null");
ok(SC._scoreMatch("op", "Open File") !== null, "subsequence matches");
ok(
  SC._scoreMatch("open", "Open File") < SC._scoreMatch("open", "Reopen Panel"),
  "earlier + contiguous match scores better (lower)"
);

// ── registry + runCommand + MRU ──
var ran = [];
SC.registerCommand({ id: "t.a", title: "Alpha", run: function () { ran.push("a"); } });
SC.registerCommand({ id: "t.b", title: "Beta", when: "enabled", run: function () { ran.push("b"); } });
ok(SC.getCommand("t.a") !== null, "registered command retrievable");
ok(SC.listCommands().some(function (c) { return c.id === "t.a"; }), "t.a listed (no when)");
ok(!SC.listCommands().some(function (c) { return c.id === "t.b"; }), "t.b hidden while when unmet");
SC.setContext({ enabled: true });
ok(SC.listCommands().some(function (c) { return c.id === "t.b"; }), "t.b listed once when met");

ok(SC.runCommand("t.a") === true, "runCommand returns true on success");
ok(ran.length === 1 && ran[0] === "a", "run() actually invoked");
ok(SC.runCommand("t.b") === true, "gated command runs when context allows");
SC.setContext({ enabled: false });
ok(SC.runCommand("t.b") === false, "gated command refused when context blocks");
ok(SC.runCommand("t.nope") === false, "unknown command returns false");

// run() that throws is caught (would toast in a DOM; here it must not crash test)
SC.registerCommand({ id: "t.boom", title: "Boom", run: function () { throw new Error("x"); } });
// toast() touches document — stub a minimal element so the catch/toast path runs.
function stubEl() {
  return {
    className: "", textContent: "", style: {},
    setAttribute: function () {}, removeAttribute: function () {},
    appendChild: function () {}, classList: { toggle: function () {} },
  };
}
global.document = { createElement: stubEl, body: { appendChild: function () {} } };
ok(SC.runCommand("t.boom") === false, "throwing run() is caught and returns false");

if (failures) { console.error("\n" + failures + " FAILURE(S)"); process.exit(1); }
console.log("\nAll shell-chrome self-tests passed.");
