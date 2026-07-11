<div align="center">

<img src="https://raw.githubusercontent.com/pointsav/pointsav-media-assets/main/ASSET-SIGNET-MASTER.svg" width="72" alt="PointSav Digital Systems">

# PointSav Digital Systems
### *Trustworthy Records Infrastructure for Institutions That Own Their Assets*
### *Infraestructura de Registros Verificables para Instituciones que Poseen sus Activos*

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=flat-square)](https://opensource.org/licenses/Apache-2.0)
[![Compliance: WORM](https://img.shields.io/badge/Compliance-WORM_Ready-22863a.svg?style=flat-square)](#)
[![Foundation: seL4 Verified](https://img.shields.io/badge/Foundation-seL4_Verified-6f42c1.svg?style=flat-square)](#the-trustworthy-systems-foundation)
[![WCAG: 2.2 AAA](https://img.shields.io/badge/WCAG-2.2_AAA-0075ca.svg?style=flat-square)](https://github.com/pointsav/pointsav-design-system)

<br/>

**[→ Documentation Wiki](https://github.com/pointsav/content-wiki-documentation)** &nbsp;·&nbsp; **[→ Design System](https://github.com/pointsav/pointsav-design-system)** &nbsp;·&nbsp; **[→ Live Deployment](https://github.com/woodfine/woodfine-fleet-deployment)** &nbsp;·&nbsp; **[→ pointsav.com](https://pointsav.com)**

</div>

<br/>

> [!NOTE]
> The Sovereign Data Foundation's intended equity and oversight role has not yet been formally executed. This repository contains no active proprietary network payloads.

---

## The Problem

Your organisation's records live in software you do not own. The vendor controls access. If they raise their prices, change their terms, or shut down, your records are held to their schedule. This is the default arrangement for every institution that stores financial books, property records, and personnel files in a commercial cloud platform. It has always been the arrangement. Most institutions have accepted it.

PointSav was built for the ones that won't.

We build operating systems — not applications — where each archive is a self-contained, formally verified environment anchored to a specific legal asset. A building. A company. A person. The archive belongs to the institution. Not to us. Not to a cloud provider. When the asset changes hands, the complete history transfers with it. When a vendor relationship ends, the data is already safely on hardware the institution physically controls.

---

## Five Structural Differences

### I. Asset-Level Ownership

Every major cloud platform is optimised for multi-tenant application workloads. Your data sits alongside other customers' data on infrastructure the vendor controls. PointSav builds differently: each archive is anchored to a specific legal identifier — a land title, a company registration number, a passport ID. The archive belongs entirely to that asset. When a building sells, the archive transfers with the title. No hyperscaler offers this concept because it is incompatible with their multi-tenant model.

For the technical reader: each ToteboxOS instance is a self-contained unikernel — a minimal operating system containing only the kernel and the services required for that specific asset. No two archives share a kernel. If one is compromised, the blast radius is mathematically contained.

### II. Formally Verified Security

Most infrastructure security is tested — engineers attempt to break it and fix what they find. Testing proves the presence of some failures. It cannot prove the absence of all of them.

PointSav's security foundation uses the seL4 microkernel, whose security properties have been formally verified using machine-checked mathematical proofs — the same technique used in avionics and medical device software. This is not a marketing claim. It is a peer-reviewed result published in the ACM Symposium on Operating Systems Principles (SOSP 2009) and maintained in continuous formal verification since. seL4 is the only general-purpose operating system kernel with this property.

### III. Flat-File Permanence

A database is a running software engine. If the engine is deprecated, updated, or discontinued, the data becomes inaccessible until migrated. PointSav stores all records as inert flat files — Markdown, YAML, CSV, JSON. A `.yaml` file written today requires no proprietary software to read in fifty years. The data outlives the software.

This is not a limitation. It is a design commitment. Software engines process the files. The files exist independently of the engines. SHA-256 cryptographic checksums seal every record at the point of entry, making any subsequent alteration detectable by any auditor with a standard terminal.

### IV. Commodity Node Economics

The base ToteboxOS deployment runs on a commodity cloud node at approximately $7 per month. No proprietary hardware. No minimum commitment. The commercial staircase is transparent: base archive at commodity pricing; local AI processing as an optional hardware upgrade; multi-archive orchestration as the proprietary commercial layer. Institutions pay for the intelligence that connects archives together, not for the storage itself.

### V. No Egress Lock-in

The end-state export format for every ToteboxOS archive is a Bootable Disk Image — a self-contained virtual machine file that boots on any standard hypervisor: bare metal, AWS, Azure, Google Cloud, Oracle Cloud. A Docker container is not freely transferable. It requires Docker to run. A bootable disk image requires only a standard hypervisor, which is a universal commodity. The complete archive — records, ledgers, search index — can be moved to a USB drive and booted on any compatible computer on earth.

---

## The Trustworthy Systems Foundation

The platform is built on a three-tier activation model. The base tier requires no AI dependency whatsoever. WORM compliance, cryptographic record integrity, and full-text search operate completely independently of any AI vendor relationship.

| Tier | Node | What activates |
|---|---|---|
| Base | ~$7/month commodity node | ToteboxOS — flat files, WORM compliance, SHA-256 sealing, search. Zero AI dependency. |
| AI-enabled | Upgraded node meeting `service-slm` minimum specs | Local AI processing activates. The doorman gateway to external AI becomes available. Corporate data never leaves the private network. |
| Orchestration | `os-orchestration` | Multi-archive operations. Extended compute for BIM, GIS, and data warehouse. The proprietary commercial layer. |

The seL4 kernel is the formal security foundation. A compatibility shim currently adapts seL4 to run on commodity cloud hardware — this is tracked as technical debt with a named replacement path in the `moonshot-*` engineering register, consistent with how every temporary dependency is managed in this codebase.

---

## The Commercial Model

Single-archive use — one ToteboxOS instance, one ConsoleOS terminal — is completely free and open source under Apache 2.0. An independent developer, a sole practitioner, or a small organisation can run a complete, WORM-compliant records platform with no commercial relationship with PointSav.

The moment you need to aggregate across multiple archives — connecting a building's property records to the personnel records of the management team, for example — you need OrchestrationOS, which is proprietary software. This is the monetisation boundary. PointSav does not charge for private data storage. It charges for the intelligence layer that connects archives together.

PointSav follows a cost-plus commercial model. Development time is charged at cost plus a fixed margin. Value-add pricing is explicitly rejected. This keeps vendor and customer incentives structurally aligned.

---

## The Live Proof

Woodfine Management Corp., a real property management company operating across North America and Europe, is executing a complete digital transformation onto the PointSav platform — deploying verified, portable archives for property records, corporate governance, and operational ledgers, with select components live and the full stack in active deployment.

Woodfine is a subsidiary of the same parent company that owns PointSav. This is not a coincidence. It was a design decision: build the software, use the software, and document both in public so the proof is verifiable. The Woodfine fleet deployment manifest — 201 commits and growing — is at **[github.com/woodfine/woodfine-fleet-deployment](https://github.com/woodfine/woodfine-fleet-deployment)**.

---

## Engineering Status

### Infrastructure

| Component | Function | License | Status |
|:---|:---|:---|:---|
| `os-infrastructure` | Compute and hardware substrate | Proprietary | 🟢 Active |
| `os-network-admin` | Private network routing and MBA registry | Proprietary | 🟡 Development |

### Platform

| Component | Function | License | Status |
|:---|:---|:---|:---|
| `os-totebox` | Core archive operating system | Apache 2.0 | 🟡 Development |
| `os-orchestration` | Multi-archive aggregation and extended compute | Proprietary | 🟡 Development |
| `os-workplace` | Staff desktop environment | Apache 2.0 | 🟡 Development |

### Delivery

| Component | Function | License | Status |
|:---|:---|:---|:---|
| `os-console` | Operator terminal — Command Ledger | Apache 2.0 | 🟡 Development |
| `os-mediakit` | Public-facing web delivery | Proprietary | 🟢 Active |
| `os-privategit` | Self-hosted version control | Apache 2.0 | 🟢 Active |

### Totebox Services

| Component | Function | Status |
|:---|:---|:---|
| `service-fs` | Immutable ledger — WORM-compliant filesystem | 🟡 Development |
| `service-email` | Inbound mail processor — transport only | 🟡 Development |
| `service-extraction` | Deterministic parser — structured and unstructured ingestion | 🟡 Development |
| `service-slm` | AI Gateway — local model and external doorman | 🟡 Development |
| `service-people` | Personnel ledger — flat-file state machine | 🟡 Development |
| `service-content` | Knowledge index — self-healing first derivative | 🟡 Development |
| `service-search` | Inverted index — air-gappable full-text search | 🟡 Development |
| `service-egress` | Physical transfer engine — cold storage entanglement | 🟡 Development |

### Moonshot Register

Every third-party dependency is tracked as formal technical debt. For each quarantined `vendor-*` component, a corresponding `moonshot-*` engineering initiative works toward a native replacement.

| Quarantined Dependency | Moonshot Replacement |
|:---|:---|
| `vendor-sel4-kernel` | `moonshot-kernel` — no_std Rust microkernel |
| `vendor-virtio` | `moonshot-hypervisor` — native Rust VMM |
| `vendor-azure-auth` | `system-mba-shim` — Machine-Based Authorization (active) |
| `vendor-microsoft-graph` | `moonshot-protocol` — native mail transport |
| `vendor-gpu-drivers` | `moonshot-gpu` — Project X-Ray native drivers |
| `vendor-linux-systemd` | `moonshot-kernel` — rc.d / runit on FreeBSD / seL4 |
| `vendor-wireguard` | `moonshot-network` — native private mesh |
| `vendor-phi3-mini` | `moonshot-toolkit` — native SLM execution |
| `app-mediakit-telemetry/assets/` (MaxMind GeoLite2) | `moonshot-index` — native geographic resolution |

---

## Repository Map

| Repository | Purpose |
|:---|:---|
| `pointsav-monorepo` | Engineering source — all `system-*`, `service-*`, `os-*`, `moonshot-*` |
| `pointsav-design-system` | Linguistic protocols and visual standards |
| `content-wiki-documentation` | Technical library — ADRs, service specifications, glossary |
| `pointsav-fleet-deployment` | Vendor's own operational systems |
| `pointsav.github.io` | Public-facing marketing site |

---

## Contact

**pointsav.com** &nbsp;·&nbsp; **open.source@pointsav.com** &nbsp;·&nbsp; **[github.com/pointsav](https://github.com/pointsav)**

---

*→ Versión en español: [README.es.md](./README.es.md)*


---

*Copyright © 2026 Woodfine Capital Projects Inc. See [LICENSE](LICENSE) for terms.*

*Woodfine Capital Projects™, Woodfine Management Corp™, PointSav Digital Systems™, Totebox Orchestration™, and Totebox Archive™ are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. All other trademarks are the property of their respective owners.*