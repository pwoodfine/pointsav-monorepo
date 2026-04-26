# vendor-sel4-fs

[ 🇪🇸 Leer este documento en Español ](./README.es.md)

Reserved-folder scaffold for a future seL4-based bare-metal
file-system service. Houses the `#![no_std] #![no_main]`
scaffold relocated from `service-fs` on 2026-04-26 (cluster
outbox `ring1-scaffold-runtime-model-drift`; ratified by Master
Decision 2 the same date).

## Why this directory exists

`service-fs` is a Ring 1 boundary-ingest service in the three-ring
architecture (per
`~/Foundry/conventions/three-ring-architecture.md`) and runs as a
hosted MCP-server process under systemd
(`~/Foundry/conventions/zero-container-runtime.md`). The earlier
scaffold at `service-fs/src/main.rs` was instead `#![no_std]
#![no_main]` with a hand-rolled `_start` entrypoint and a panic
loop — a bare-metal seL4 unikernel framing that contradicted both
ratified conventions.

The seL4 lineage already has its own home in the registry:

- `vendor-sel4-kernel` (1074 files; vendored seL4 kernel source)
- `moonshot-sel4-vmm` (4 files; seL4 virtual machine monitor)
- `system-substrate-broadcom`, `-freebsd`, `-wifi` (hardware bridges)

`vendor-sel4-fs` joins that lineage as the natural home for an
eventual seL4-based file-system service. Today it is a
**Reserved-folder placeholder** — the relocated 26-line scaffold
plus this README pair. Activation per `~/Foundry/CLAUDE.md` §9 is
deferred until seL4-track work resumes and someone defines the
scope.

## State

Reserved-folder. Created 2026-04-26 by Task Claude on the
`project-data` cluster as a relocation target for the seL4
scaffold previously at `service-fs/`. Not yet activated, not yet
in workspace members, not yet wired into any build.

## Contents

```
vendor-sel4-fs/
├── Cargo.toml             — bare-metal package manifest
├── Cargo.lock             — lockfile (no deps; minimal)
├── .cargo/config.toml     — `target = "x86_64-unknown-none"` override
├── src/main.rs            — 26-line no_std/no_main scaffold
├── README.md              — this file
└── README.es.md           — Spanish overview
```

## Hard rules — do not violate (when this project activates)

- **Pure bare-metal.** Zero OS dependencies permitted (per the
  Cargo.toml comment that came with the relocation). When the
  scope expands, additions are bare-metal-compatible only.
- **Out of scope: Ring 1 hosted-process work.** Ring 1 hosted
  MCP-server boundary-ingest is `service-fs`'s job, not
  `vendor-sel4-fs`'s. The two have different runtime models.

## Licence

Refer to the repo `LICENSE` file. Component-level licence
assignment is governed by `pointsav/factory-release-engineering`'s
`LICENSE-MATRIX.md`.

## See also

- `service-fs/` — Ring 1 WORM ledger (hosted Tokio MCP-server;
  rebuilt 2026-04-26 in the same session that created this
  directory)
- `vendor-sel4-kernel/`, `moonshot-sel4-vmm/` — sibling seL4-track
  scaffolds
- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1
  contract that excluded the bare-metal framing
- `~/Foundry/conventions/zero-container-runtime.md` — deployment
  shape that excluded the bare-metal framing
- `~/Foundry/clones/project-data/.claude/outbox-archive.md`
  (when archived) — the `ring1-scaffold-runtime-model-drift`
  outbox message and Master's ratification
