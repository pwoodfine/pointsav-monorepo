# moonshot-cad-kernel

Sovereign CAD kernel foundation for the workplace CAD/BIM tool
(`BRIEF-workplace-bim-cad-tool`). Phase 0: 2D geometry primitives + the git-like
**operation-log document model** that is the tool's crown jewel — every change is a
typed op, the drawing state is a rebuildable projection of the log, and undo/redo +
a diffable history fall out for free.

## What's here (Phase 0)

- **Geometry:** `Point`, `Bounds`, and `Entity` (Line, Circle, Arc, Polyline) with
  `length()` and `bounds()`.
- **Document model:** `Document` (layers + placed entities) is rebuilt deterministically
  by replaying an append-only `Op` log.
- **`Drawing`:** the op log + current state + an undo/redo cursor; `apply`, `undo`, `redo`,
  convenience builders (`add_layer`, `add_entity`), and **JSON-Lines** save/load
  (`to_jsonl` / `from_jsonl`) — the canonical, diffable, git-friendly format.

## Not here yet (later phases, per the BRIEF)

- `wgpu` 2D/3D rendering (one pipeline, no JS render path).
- An ISOtope-style 2D constraint solver.
- Forking `truck` for B-rep 3D (sketch→extrude).
- BIM token/type/occurrence binding via `moonshot-bim-engine` (two layers, one kernel).

## Design notes

- Sovereign, offline, WASM-ready. Dependencies: `serde` + `serde_json` only (Apache/MIT).
- The op log **is** the document — this is what makes git-like design history and
  (later) `moonshot-crdt` collaboration native rather than bolted on.
- Deterministic Rust; no AI inference touches the model (SYS-ADR-07).

`cargo test` covers geometry, apply/undo/redo, redo-tail semantics, and JSON-Lines
round-trip.
