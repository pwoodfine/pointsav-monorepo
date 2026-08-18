# Woodfine / PointSav Platform — User Guide

> Generated: 2026-03-30
> To regenerate this document, run `/generate-user-guide` in Claude Code.

---

## Table of Contents

- [Part I: The Foundation — Why This Exists](#part-i-the-foundation--why-this-exists)
- [Part II: The Vision — Where This Is Going](#part-ii-the-vision--where-this-is-going)
- [Part III: The Architecture — How It Is Organized](#part-iii-the-architecture--how-it-is-organized)
- [Part IV: The Security Model](#part-iv-the-security-model)
- [Part V: Data Architecture — Files Over Databases](#part-v-data-architecture--files-over-databases)
- [Part VI: The Data Pipeline — How Information Flows](#part-vi-the-data-pipeline--how-information-flows)
- [Part VII: The Operator Interface](#part-vii-the-operator-interface)
- [Part VIII: Fleet Deployment](#part-viii-fleet-deployment)
- [Part IX: Operational Guides](#part-ix-operational-guides)
- [Part X: Architecture Decisions](#part-x-architecture-decisions)
- [Part XI: The Road Ahead — Moonshots](#part-xi-the-road-ahead--moonshots)
- [Appendix: Glossary](#appendix-glossary)

---

# Part I: The Foundation — Why This Exists

## The Opportunity

Modern businesses store their most important records — financial ledgers, personnel files, property documents — inside software they do not own. Cloud platforms hold the data. The vendor controls the access. If the vendor changes terms, raises prices, or shuts down, the business scrambles.

Woodfine Capital Projects Inc. builds real estate. Over the next 8-10 years, the company plans to develop 75+ LEED-certified buildings across Canada, the United States, Mexico, and Spain. That means thousands of tenants, hundreds of service providers, decades of financial records, and building systems that need to keep running long after the original software vendor has moved on.

Rather than trust those records to someone else's platform, Woodfine decided to build its own.

## The Organizations

Four entities make this work:

| Entity | Role | Think of It As |
|--------|------|----------------|
| **Woodfine Capital Projects Inc.** | Parent company | The holding company. Real estate firm, not a tech company. |
| **Woodfine Management Corp.** | Subsidiary. The customer. | The operational arm that manages properties and uses the software daily. |
| **PointSav Digital Systems** | Subsidiary. The vendor. | The technology arm that builds the software. Follows a cost-plus model (charges for development time, not for value delivered). |
| **Sovereign Data Foundation** (Denmark) | Intended equity holder in PointSav | Planned to oversee open-source component integrity and data sovereignty standards. This arrangement has not yet been formally executed. |

**The key relationship:** PointSav builds the software. Woodfine uses it. Both are owned by the same parent. On GitHub, the `pointsav` organization holds the engineering source code and the `woodfine` organization holds the customer's operational deployments.

## The Three Pillars

Every piece of software PointSav builds serves one or more of three business purposes. These are not technical categories — they are the business reasons the platform exists.

**Business Administration.** Managing properties, tenants, service providers, communications, budgets, and reporting. This is the operational work that happens every day inside a real estate company.

**Record Keeping.** Personnel records, corporate records (financial books, minute books, statutory calendars), and real property records. These are records that must exist for legal and regulatory reasons, must be auditable, and must be preserved for the full lifetime of the asset they describe.

**Cyber-physical Connectivity.** IoT sensors, Building Information Modelling (BIM) drawings, materials databases, digital twins, and remote building management. This is where the physical buildings connect to software.

If a component cannot be traced back to one of these three pillars, it needs a strong reason to exist.

---

# Part II: The Vision — Where This Is Going

## The End State

The long-term goal is a platform where every major component runs as its own self-contained operating system — a purpose-built machine that does one thing and does it well. Think of a physical office network where the printer has its own firmware, the network-attached storage has its own firmware, and the firewall has its own firmware. Each device is independent. If the printer is compromised, the attacker is physically trapped inside the printer — they cannot reach the filing cabinet.

That is the model PointSav is building toward, but virtualized and applied to corporate data. Each component will run as a tiny, isolated virtual machine called a unikernel (explained below). No two components share a kernel. If one is compromised, the damage is mathematically contained to that single machine.

The reference implementations for this architecture are AWS Firecracker (a micro-VM technology) and the seL4 kernel (a mathematically proven microkernel). These are reference points, not locked-in choices.

## The Current Reality

The end state is years away. Right now, the platform runs as ordinary Linux applications, mostly on cloud servers. This is the MVP — Minimum Viable Product — phase.

What matters is that the **component names, boundaries, and data ownership rules are the same in both phases**. The code you write today should treat each component as if it were already running on its own machine. That means:

- Components communicate over the network, not through shared files or databases.
- Each archive's data is self-contained and can be packaged up independently.
- Authentication is designed so it can later be replaced by hardware-level cryptography.

Low-cost choices made today smooth the transition to the end state. Hardcoded connection strings and shared databases make the transition expensive.

## The 3-Layer Stack

The full system breaks into three layers.

**Infrastructure.** Raw computing power. Physical servers, cloud instances, or dedicated hardware that provides CPU, memory, storage, and networking. In the end state, this layer runs a PointSav-built virtualization substrate called InfrastructureOS. In the MVP, it is cloud compute (GCP/AWS).

**Platform.** The operating systems themselves — the ToteboxOS archives (data vaults), the OrchestrationOS hubs, and the NetworkAdminOS that manages the network. These components run as kernel-isolated virtual machines on top of the infrastructure layer.

**Delivery.** The ConsoleOS — the software each operator uses at their desk. In the end state, each variant runs as its own virtual machine on the user's workstation. In the MVP, it is a browser-based Heads-Up Display served over the Private Network.

```
+--------------------------------------------------+
|  DELIVERY LAYER                                  |
|  ConsoleOS variants on operator workstations     |
|  (FKeysConsole, InputMachine, BIMConsole, etc.)  |
+--------------------------------------------------+
                        |
                        | network
                        |
+--------------------------------------------------+
|  PLATFORM LAYER                                  |
|  ToteboxOS archives  (data)                      |
|  OrchestrationOS hubs (logic)                    |
|  NetworkAdminOS      (management)                |
+--------------------------------------------------+
                        |
                        | runs on
                        |
+--------------------------------------------------+
|  INFRASTRUCTURE LAYER                            |
|  InfrastructureOS / GCP / AWS / on-prem servers  |
+--------------------------------------------------+
```

## The Licensing Boundary

Single-archive use — one person, one Totebox Archive, one ConsoleOS — is completely free and open source. This is the Community Member path.

The moment you need to aggregate across multiple archives (for example, pulling together personnel records and property records for a project team), you need OrchestrationOS, which is proprietary software. This aggregation layer is how PointSav monetizes: it does not charge for private data storage, but it charges for the logic that connects multiple archives together.

This boundary is not just commercial — it is a design constraint. FOSS (free and open source) components must never contain proprietary logic. An independent developer should be able to run a Totebox Archive and a ConsoleOS variant for their own use without touching any proprietary code.

---

# Part III: The Architecture — How It Is Organized

## The Five Core Components

The platform is organized around five named components. These names are canonical — they appear in code, documentation, and conversation. Do not substitute synonyms.

### ToteboxOS (Totebox Archives)

A ToteboxOS instance is a self-contained store for one specific asset. Think of it as a sealed filing cabinet that belongs entirely to one entity — a building, a company, or a person. Everything relating to that entity lives inside it: documents, records, and logs.

In the current phase, a ToteboxOS instance is a structured directory on a cloud server. In the end state, it is a minimal operating system — a unikernel — that runs as a virtual machine, boots from a disk image, and can be physically moved to another computer.

All data is stored as inert flat files (Markdown, YAML, CSV). Software engines are explicitly decoupled from the data they process. A `.yaml` file or a `.csv` ledger will be universally readable in 100 years, requiring zero proprietary software to decrypt or access.

There are three preset types of Totebox Archives:

| Archive Type | Anchored To | Contains |
|---|---|---|
| PersonnelArchive | SIN or Passport ID | Professional network, identity records, contact history |
| CorporateArchive | Business Incorporation Number or Tax ID | Financial records, minute books, statutory ledgers, calendars |
| PropertyArchive | Land Title PIN or legal address | Property data, permits, lifecycle logs, BIM drawings, IoT data |

The anchor identifier is the universal key. A PersonnelArchive anchored to a SIN number ties together every record about that individual regardless of which system generated it.

### OrchestrationOS (The Logic Hub)

OrchestrationOS is a stateless layer — it holds no data of its own. Its job is to connect a ConsoleOS to one or more ToteboxOS archives and apply business logic to the combined result. It also provides the extended compute capacity that the base archive node cannot deliver — BIM rendering, GIS spatial analysis, SLM inference, and data warehouse operations all require OrchestrationOS capacity.

If a user only needs to view one archive, they can connect to it directly from their ConsoleOS. OrchestrationOS becomes necessary when they need to see across multiple archives simultaneously — for example, a project manager who needs to view both the building's records and the personnel records for the project team at the same time.

This aggregation function is the monetization boundary. The main variant used by Woodfine is the CommandCentre, which aggregates PersonnelArchives and CorporateArchives.

Other OrchestrationOS variants include:

| Variant | Purpose |
|---|---|
| CommandCentre | Primary administrative hub. Aggregates Personnel and Corporate Archives |
| BIMServer | Serves structured Building Information Modelling data to field engineers |
| GISEngine | Geographic analysis — origin-destination studies, mapping |
| IoTConnect | Bridges IoT device logs: water sensors, access doors, temperature |
| DataWarehouse | Aggregates data from multiple archives for analytics |
| BuildingIDServer | Links digital archives to physical building identifiers |
| JobShackServer | Construction project coordination |

### ConsoleOS (The User Interface)

The ConsoleOS is what operators use at their desk. Each variant is purpose-built for a specific workflow and connects to a specific OrchestrationOS variant. In the current phase, these are browser-based Heads-Up Displays served via NGINX. In the end state, each variant runs as its own virtual machine on the user's workstation.

The primary variant is the FKeysConsole — a keyboard-driven interface where each function key activates a different context within the corporate record system. It is designed to be fast, keyboard-driven, and stable across backend upgrades.

Other ConsoleOS variants:

| Variant | Purpose |
|---|---|
| FKeysConsole | Primary administrative terminal. F-key driven interface to archives |
| InputMachine | Data entry and record creation (the F12 function) |
| BIMConsole | Building Information Modelling for field engineers |
| MapsConsole | Geographic and location intelligence visualization |
| DataConsole | Analytics and data access (also called DataMarketplace) |
| BuildingPreferences | IoT device management and building state control |
| SiteSuper | Construction site coordination |

### InfrastructureOS (The Virtualization Substrate)

InfrastructureOS is the layer that physical hardware runs. It provides the virtualization environment for all other components — the CPU, memory, storage, and networking that ToteboxOS instances and OrchestrationOS hubs run on top of.

In the MVP phase, this is cloud compute (GCP/AWS). In the end state, it is a purpose-built PointSav operating system that can run on on-premises servers, leased dedicated servers, and public cloud simultaneously.

### NetworkAdminOS (The Network Manager)

NetworkAdminOS gives operators visibility and control over everything running in the Private Network. It maintains a registry of which components are paired to which — the MBA registry (see Part IV). It handles health monitoring, deployment status, and network isolation commands.

The distinction from InfrastructureOS is worth emphasizing: InfrastructureOS is the substrate (like a server rack), while NetworkAdminOS is the management console (like the dashboard that shows you what is running in the rack).

## The Canonical Naming Table

The table below lists canonical names and what NOT to substitute them with. Using alternate names in code or conversation creates confusion because the domain has a precise vocabulary.

| Canonical Name | Do NOT Call It |
|---|---|
| ToteboxOS | ArchiveOS, VaultOS, DataNode |
| OrchestrationOS | InterfaceOS, LogicLayer, AggregationService, MiddleTier |
| ConsoleOS | ClientApp, Terminal, Frontend |
| InfrastructureOS | HypervisorLayer, VMSubstrate |
| NetworkAdminOS | MeshController, NodeManager |
| ToteboxArchive | (generic name for any ToteboxOS instance with its data) |
| PersonnelArchive | HRArchive, PeopleNode |
| CorporateArchive | FinanceArchive, BusinessNode |
| PropertyArchive | RealPropertyArchive, BuildingArchive, AssetNode |
| CommandCentre | OrchestratorHub, AccessGateway |
| FKeysConsole | MainTerminal, AdminConsole |
| PairingAsPermission | RBAC, ACL, PermissionSystem |
| FreelyTransferable | portable, migratable |
| TheSnap | (cross-archive verification mechanism) |
| Contributor | employee, developer, team member |
| GeneralStaff | end users, employees |
| Assignment | ticket, story, task |

---

# Part IV: The Security Model

## Why This Matters

Most software systems grant access through usernames, passwords, and a database of permissions. An attacker who steals credentials — or who tricks an administrator into granting them a role — can move through the system as if they were an authorized user.

PointSav's security model rejects this entirely. Access is not granted through identity credentials. It is granted through the physical topology of the network itself. If two machines are not cryptographically paired, they cannot communicate. There is no credentials database to steal because there are no credentials.

This model is called **Pairing as Permission** and it is implemented through **Machine-Based Authorization (MBA)**.

## How It Works

Every node in the network has a machine identity — a cryptographic certificate tied to that specific hardware instance. When two nodes are paired, they exchange cryptographic keys and register the pairing in the MBA registry maintained by NetworkAdminOS.

From that point on, the question the system asks is not "Does this user have permission?" but rather "Is this ConsoleOS instance paired with this OrchestrationOS instance?" The answer is visible directly from the network topology.

**An example:** A project team needs access to two buildings and their associated staff records.

1. Deploy one CommandCentre (OrchestrationOS) instance.
2. Pair it to the two PropertyArchive instances for those buildings.
3. Pair it to the PersonnelArchive instances for the team members.
4. Pair each team member's FKeysConsole to the CommandCentre.

The access boundary is now defined by the wiring diagram. No permissions database was updated. No user roles were assigned. The architecture IS the access control. This is what Peter Woodfine calls **Geometric Security**.

## What This Means for the MVP

The full cryptographic hardware pairing is a future-state capability. In the MVP, the platform uses conventional authentication (currently OAuth2 via Azure AD, classified as a quarantined vendor dependency). However, this conventional auth is implemented behind a clean interface (`system-mba-shim`) so it can later be swapped for MBA without rewriting business logic.

When designing any component that involves access control, the question to ask is: "Could this access pattern be expressed through component topology rather than a permissions table?" If the answer is yes, prefer the topology-based pattern even if the underlying mechanism is still conventional.

## Capability-Based Security

Underneath the MBA model is a deeper principle: Capability-Based Security. In a conventional operating system, an application can often escalate its privileges if it finds a vulnerability — a compromised application can reach out and touch other parts of the system.

In the PointSav architecture, each component holds a cryptographic capability token that specifies exactly which other components it is allowed to communicate with. An application cannot communicate with something it doesn't hold a capability for. If an edge node (such as the MediaKitOS web layer) is compromised, the attacker is trapped inside that node's memory sandbox. They hold no capabilities to reach the ToteboxOS vaults. The blast radius is physically contained.

The seL4 kernel is the reference point for this approach because it is the only operating system kernel whose security properties have been mathematically proven — not just tested, but formally verified using machine-checked mathematical proofs.

## The One-Way Command Flow (Diode Standard)

Because components are physically locked into isolated memory sectors, the system enforces a strict, one-way command flow. An isolated Edge Delivery network (`os-mediakit`) cannot issue commands backward into the secure Totebox vault (`os-totebox`). Data moves in one direction only: from source to destination. No reverse channel. This applies to both network commands and the telemetry pull architecture.

## The Snap: Cross-Archive Integrity Verification

The Snap (formally the IssuerSnap) is a verification mechanism that cross-checks records across multiple archives. It is used primarily for producing authenticated quarterly reports as required of Reporting Issuers (publicly-traded limited partnerships).

The MediaKitOS pulls data from Corporate Archives (financial records) and Property Archives (property data), cross-references them against Personnel Archives (who is responsible for what), and generates a report whose data chain can be verified end-to-end. Each cross-reference uses the anchor identifiers described in Part III — a Corporate Archive references a person by their SIN, not by a local database ID.

---

# Part V: Data Architecture — Files Over Databases

## The Core Principle

A database is a running software engine. If the database software is shut down, deprecated, or becomes unavailable, your data is inaccessible. A flat file — a text file, a YAML file, a CSV spreadsheet — is just bytes on a disk. It can be read with any text editor on any computer, now or in one hundred years.

This is the philosophical foundation of PointSav's data architecture: store corporate knowledge as inert flat files, not in database engines. Software engines read and process those files, but the files exist independently of the engines.

The practical mandate is captured in the Totebox directory structure:

```
cluster-totebox-corporate/
├── app-console-input/      <-- execution software (processes the files)
├── assets/                 <-- physical vault (PDFs, images, contracts)
└── ledger/                 <-- state machine (YAML metadata, CSV ledgers)
```

The `assets/` directory holds the physical files. The `ledger/` directory holds the metadata and cryptographic checksums. The execution software in `app-console-input/` reads and writes to both — but the data exists without it.

## The Freely Transferable Standard

A ToteboxArchive must be exportable as a complete, self-contained package that a recipient can deploy on their own infrastructure without any proprietary runtime, vendor relationship, or platform subscription.

In the end state, this export artifact is a Bootable Disk Image — a virtual machine image file that boots on any standard hypervisor (bare metal, AWS, Azure, Google Cloud, Oracle Cloud).

**Important distinction:** A Docker container is NOT Freely Transferable. It requires Docker to run. A bootable disk image requires only a standard hypervisor, which is a universal commodity.

A practical example: When Woodfine sells a building, the entire history of that building — permits, contracts, IoT logs, maintenance records — lives in a PropertyArchive. The buyer receives a Bootable Disk Image of that archive. They boot it on their own infrastructure. The complete history is now under their control, with zero ongoing dependency on Woodfine's systems.

In the MVP phase, "Freely Transferable" means: no proprietary data formats, no vendor lock-in in the data path, and "export this archive" as a tested operation from day one.

## Cryptographic Ledgers

Because flat files cannot defend themselves — a rogue system administrator could silently edit a YAML file and backdate the change — PointSav enforces integrity at the filesystem level using SHA-256 checksums.

When a physical asset (a PDF contract, an invoice, a compliance document) is stored in a ToteboxArchive, the system performs two simultaneous operations:

1. The file is placed in the `/assets/` vault with execution permissions stripped. It becomes an inert binary — software cannot run it, only read it.

2. A `.yaml` file is created in the `/ledger/` directory containing the file's metadata (author, date, category) and a SHA-256 cryptographic checksum of the file's exact contents.

A SHA-256 checksum is a mathematical fingerprint. If anyone changes even a single character in the original file — a number in a financial record, a date on a contract — the fingerprint no longer matches. Any auditor can independently re-run the checksum against the stored file and compare it to the one in the ledger. If they match, the document is verified. If they don't, the archive has been tampered with.

This is the WORM principle — Write Once, Read Many. Once a record enters cold storage, it is mathematically sealed.

## Cryptographic Payload Attestation

For public-facing content (such as investor disclosures served through MediaKitOS), the system extends the integrity model to the browser. The JavaScript engine reads the visible text on screen, computes a SHA-256 hash using the browser's native `crypto.subtle.digest` API, and displays the resulting fingerprint live in the interface sidebar. Any institutional investor or auditor can independently copy the text, run their own hash, and verify it matches — proving the disclosure has not been tampered with in transit. This operation occurs 100% client-side with zero server execution.

## The Replacement Programme

Any digital transformation begins with a dependency on third-party software. Microsoft 365 for email, cloud providers for hosting, proprietary authentication systems. These dependencies represent a risk: if a vendor changes their terms of service, deprecates an API, or goes out of business, the corporate infrastructure that depends on them breaks.

PointSav tracks every third-party dependency as a formal technical debt entry. Foreign dependencies are quarantined in isolated directories (named `vendor-*`) where their behavior is tightly controlled. For each quarantined dependency, a corresponding Moonshot engineering initiative (in a `moonshot-*` directory) works on a native replacement.

When the native replacement reaches parity, it physically replaces the quarantined vendor code. This continuously shrinks the platform's dependency surface and eliminates vendor lock-in over time.

---

# Part VI: The Data Pipeline — How Information Flows

## Overview

The data pipeline describes how information from the outside world — primarily email — enters the ToteboxArchive system, gets processed, and becomes queryable corporate knowledge. This is the first implemented pipeline; additional service pipelines will be developed as the platform matures. There are seven named components in this pipeline.

```
 +================================+
 |   EXTERNAL WORLD               |
 |   (Microsoft 365, Email)       |
 +===============|================+
                 |  OAuth2
                 v
 +-------------------------------+
 |  service-email                |
 |  (Transport Interceptor)      |
 |  Pulls raw email from M365.   |
 |  Writes to tmp-maildir queue. |
 |  Zero intelligence applied.   |
 +---------------|---------------+
                 |
                 v
 +-------------------------------+
 |  service-parser               |
 |  (Traffic Controller)         |
 |  Strips MIME/JSON formatting. |
 |  Creates Entity Bundles.      |
 |  Routes by payload type.      |
 +--------|----------------|-----+
          |                |
          v                v
    Structured       Unstructured
    data             human text
    (.csv, .xlsx)    (email body, PDF)
          |                |
          v                v
    Deterministic    +----------------+
    services         |  service-slm   |
    (service-people, |  (AI Gateway)  |
     telemetry)      |  Sub-billion   |
                     |  param model.  |
                     |  Extracts facts|
                     |  then shuts    |
                     |  down.         |
                     +-------|--------+
                             |
                             v
                     +----------------+
                     | service-content|
                     | (Knowledge     |
                     |  Graph)        |
                     | Self-healing   |
                     | First          |
                     | Derivative.    |
                     +-------|--------+
                             |
                             v
                     +----------------+
                     | Verification   |
                     | Surveyor       |
                     | Human confirms |
                     | identity facts.|
                     | 10/day limit.  |
                     +-------|--------+
                             |
                             v
                     +----------------+
                     | service-search |
                     | (Inverted      |
                     |  Index)        |
                     | Microsecond    |
                     | search across  |
                     | all files.     |
                     +-------|--------+
                             |
                             v
                     +----------------+
                     | FKeysConsole   |
                     | (Operator HUD) |
                     | F2/F3/F4/F12   |
                     +-------|--------+
                             |
                             v
                     +----------------+
                     | service-message|
                     | -courier       |
                     | Outbound       |
                     | messaging via  |
                     | adapters.      |
                     +----------------+
```

## service-email — The Transport Interceptor

The email service's only job is to pull email out of the external cloud (Microsoft 365) and write it to a local disk. It uses an OAuth2 cryptographic handshake against the Microsoft Graph API to authenticate. It polls for unread messages, extracts the raw data, writes it to a temporary queue directory (`/assets/tmp-maildir/`), and marks the message as read on the external server to prevent it from being pulled again.

This service deliberately has no intelligence. It does not read the email content, categorize it, or make any decisions. It is a transport mechanism only. It is hard-capped to extract exactly 3 emails per folder per cycle (9 total maximum) to protect resource-constrained edge environments.

The monorepo contains two harvester implementations: an older EWS-based harvester (`ingress-harvester`) and a newer Graph API-based harvester (`master-harvester-rs`). The newer implementation dynamically discovers folder IDs, maintains a deduplication ledger, downloads in parallel, and does not delete messages from the mailbox — making it the operationally safer choice.

## service-parser — The Traffic Controller

The parser receives raw email from the queue and strips the proprietary formatting (MIME multipart, Base64 encoded attachments, JSON wrappers) to produce clean, readable content. It assigns a transaction ID to each email so all downstream artifacts can be traced back to their source.

It then routes the cleaned content based on what type of data it is:

- **Structured data** (spreadsheets, CSV files, telemetry logs) goes directly to the appropriate deterministic service. These have predictable formats that can be parsed with 100% accuracy using standard algorithms. The AI is forbidden from this path (SYS-ADR-07).
- **Unstructured human text** (email body text, PDFs, Word documents) is routed to service-slm for AI extraction.
- **Media files** (images, SVGs) are stored directly in the inert media vault.
- **Consumable media** (newsletters, marketing emails) is routed directly to the AI synthesis engine and then deleted — it is not kept in long-term storage.

The raw original file is always vaulted before any processing begins. This maintains chain of custody (SYS-ADR-07: forensic chain of custody rule).

## service-slm — The AI Gateway

The SLM service is an optional service installed directly on a Totebox Archive node. It requires minimum hardware specifications above the base tier. It operates in two roles simultaneously.

**Role 1 — Local AI processing.** A small, locally-running open-source model — Qwen, Phi, or equivalent — applies AI-assisted processing to unstructured human text within the archive. It reads, extracts structured facts, formats the output as clean Markdown, and terminates. It does not maintain a running connection. It processes one payload and shuts down.

This is the "point-in-time execution" model. It prevents two failure modes that plague commercial AI integrations:
1. Corporate data being absorbed into a third-party AI provider's training data.
2. The AI having ongoing influence over stored records, which could introduce invisible errors over time.

**Role 2 — The Doorman.** `service-slm` is the standardised gateway protocol between the Totebox Archive and any external AI model. Before any content leaves the private network, the local model sanitises the payload — stripping sensitive information, masking personally identifiable data, and removing physical coordinates. Only the anonymised structural skeleton routes outbound. When the external model returns a result, the local router re-hydrates the response with correct internal context before delivering it to the operator. The external model never holds actual corporate records.

**Three paths for AI on the platform:**

| Path | What the operator does | PointSav's role |
|---|---|---|
| No AI | Run ToteboxOS at base tier. Full WORM compliance and search with zero AI dependency. | None required |
| DIY via Doorman | Install `service-slm` and wire any external AI model to the gateway protocol independently. | Provides the open-source doorman standard only |
| Packaged product | Deploy `app-orchestration-slm` on OrchestrationOS. PointSav provides a packaged, pre-configured open-source model ready to run. | Packaging, configuration, and integration |

## service-people — The Personnel Ledger

The personnel ledger maintains the master contact database for the organization. It stores unique identifiers (SIN or Passport ID as the anchor), contact states, and communication history for every person in the corporate network.

It operates as a deterministic flat-file engine — a JSON-based state machine rather than a conventional database. It processes queries and updates from authorized execution adapters (the ConsoleOS F2 key interface) and executes read/write operations against the stored files.

## service-content — The Knowledge Graph

The content service maintains the "First Derivative" — the self-healing knowledge graph derived from all the raw source material in the archive. Where service-email holds the immutable originals, service-content holds the extracted, processed intelligence.

The knowledge graph is organized around four control valves that update at different rates to preserve stability:

| Control Valve | Update Rate | Contents |
|---|---|---|
| Archetypes | Every 24+ months | The psychological and functional identity of the firm |
| Chart of Accounts | Every 18-24 months | The structural and financial categories. Requires Executive override to change |
| Domains | Every 12+ months | The major theaters of operation: Corporate, Projects, Documentation |
| Themes | Every 3-8 months | Active operational narratives (e.g., "Co-Location Expansion") |

These slow update rates are deliberate. Stability in the taxonomy means that data recorded five years ago and data recorded today are comparable and searchable in the same terms.

## Verification Surveyor — The Human Checkpoint

The Verification Surveyor is an intentional bottleneck in the identity resolution pipeline. Before an extracted person record can be committed to the verified ledger, a human operator must confirm it.

The process is deliberately air-gapped: the system does not call the LinkedIn API or any external directory service to verify records. API integrations would require foreign tokens that conflict with the platform's data integrity principles, and high-volume API calls incur SaaS taxation and risk IP bans. Instead, it displays the extracted information at the operator's terminal, and the operator uses their own personal browser to verify the information independently. The machine never touches the external network during verification.

A hard throttle of 10 verifications per day is enforced. This is not a technical limitation — it is an intentional design choice. The goal is to make verification a high-value ritual rather than a checkbox exercise. Ten perfect records per day produces 3,650 verified relationships per year with zero data corruption. A higher limit would produce more records but would invite fatigue-driven errors.

## service-search — The Inverted Index

The search service provides rapid retrieval across all files in the archive. It is built using Tantivy, a Rust-based search library, and implements an inverted index — the same underlying structure used by the index at the back of a textbook.

The index is static and binary, meaning it does not require a running database engine. The entire index can be copied to a USB drive and searched instantly on any machine. This is what the DARP (Data Access and Retention Protocol) compliance standard requires: data must be searchable without proprietary software.

## service-message-courier — Outbound Messaging

The message courier handles outbound communications — sending messages to external platforms on behalf of the organization. It uses a headless browser automation approach and is built on the Adapter Pattern: the core engine contains no hard-coded targets or credentials. All platform-specific logic is injected at runtime through scripts in a private adapters directory that is excluded from version control.

This design keeps client-specific operational data (credentials, target URLs) out of the public codebase while maintaining a single, generic execution engine.

---

# Part VII: The Operator Interface

## The Command Ledger

The operator's primary interface is the Command Ledger — accessible at `console.woodfinegroup.com`. It is built as a Heads-Up Display (HUD): a window that routes data between the operator and the underlying archives, not a destination in itself (SYS-ADR-18). The ConsoleOS is explicitly not a "web app" — it is a routing terminal.

The HUD exposes three layers of the Derivative Architecture:

- **Base Assets** (the raw, immutable originals — emails, PDFs)
- **First Derivative** (the processed, searchable knowledge graph)
- **Third Derivative** (generated outputs — drafts, reports, CSV exports)

The interface is designed around function keys. Each F-key activates a specific context within the system. This keyboard-first design is intentional: it is fast, does not require a mouse, and remains stable across backend upgrades.

## Zero-Form Architecture

Traditional web applications show you forms — boxes to fill in, dropdowns to select, buttons to click. The Command Ledger is designed to minimize this. The operator speaks to the system in natural English ("Find John from the plumbing company"), and the SLM silently translates that into a structured query against the knowledge graph. The result is not a form submission — it is a physical file (a CSV index card or a Markdown document) dropped to the operator's desktop.

This Zero-Form design eliminates the overhead of navigating through nested menus and filling in structured forms for every query. The operator works at the speed of language, not at the speed of a form wizard.

## F-Key Taxonomy

| F-Key | Name | What It Does | Output |
|---|---|---|---|
| F2 | People | Search and view personnel entities and contact records | CSV "Index Card" to desktop |
| F3 | Email | View cold-stored base email assets (the originals) | Display / export |
| F4 | Drafting | Draft communications with SLM assistance. The SLM passively highlights alignment with active Themes and Archetypes | Markdown file to desktop |
| F8 | Network Command | PPN mesh management. Issue fleet commands. Broadcast network health checks. Isolate nodes | Real-time network action |
| F12 | Input Machine | The Anchor. Human-in-the-loop gateway for committing base assets to cold storage. Must manually select Destination Totebox, Service, and Chart of Accounts | Immutable ledger entry |

**Critical note on F12:** The Input Machine is the ground truth of the entire system. Nothing enters long-term storage without passing through F12. The SLM may suggest a Chart of Accounts classification, but the human operator must physically authorize the commit. This is the Fiduciary Anchor — the point where a human's judgment becomes the irreversible fact of record.

## The Verification Airgap (Git for Knowledge)

The ConsoleOS operates with a Git-like Checkout/Commit model for institutional knowledge (SYS-ADR-19). This is designed to prevent automated AI systems from silently publishing hallucinated content to verified corporate ledgers.

**Checkout:** The F8 Command Ledger presents AI-staged "Overlays" — draft content the system has prepared. Operators physically eject (checkout) the raw Markdown/JSON drafts to their local, air-gapped desktop for native editing.

**Commit:** The F12 Input Machine is the Commit Gateway. Operators drag-and-drop their human-verified, perfected files back into the Totebox. This explicit physical action constitutes the verification required to overwrite the long-term support ledgers.

Automated AI publishing to verified ledgers is mathematically forbidden. The Totebox Orchestration system (os-totebox, os-console) is strictly isolated from Independent Systems (os-mediakit, os-privategit), which act solely as downstream consumers of F12-verified ledgers.

## Micro-Frontend Cartridge Architecture

The ConsoleOS is built as a strict two-part system (SYS-ADR-11):

**The Chassis** (os-console): An empty picture frame. It holds the global stylesheet, the F-key routing logic, and the Machine-Based Authorization parameters. It contains zero business data and zero application logic. It does not know what a PersonnelArchive is.

**The Cartridges** (app-console-*): Isolated HTML/JavaScript fragments, one per F-key function. Each cartridge is completely unaware of the broader system. When an operator presses an F-key, the Chassis dynamically fetches and mounts the corresponding cartridge into the viewport using the browser's native fetch() API.

This separation means that a change to the F4 Drafting cartridge cannot break the F12 Input Machine cartridge. They do not share code. Each cartridge can be updated, replaced, or extended independently.

**Port splitting (SYS-ADR-14):** The NGINX Cloud Shield routes traffic asymmetrically. Static UI requests go to port 8888 (the Chassis HTML/CSS/JS). API calls go to port 8080 (the Rust backend over WireGuard). This prevents the Rust binaries from acting as file servers, which would violate memory isolation.

**Cache busting (SYS-ADR-15):** Browsers aggressively cache HTML/JS files, which creates a risk of operators seeing outdated UI state. NGINX injects cache-destroying headers on every outbound UI payload (no-store, no-cache, max-age=0), and the client-side fetch() API appends a dynamic timestamp parameter to force fresh asset requests.

---

# Part VIII: Fleet Deployment

## Physical Infrastructure

Woodfine's fleet consists of three infrastructure tiers. On-premises hardware handles the most sensitive, control-plane functions. Cloud relays handle the operational workloads. Operator terminals are the user-facing endpoints.

| Fleet Directory | Role | Current Hardware |
|---|---|---|
| fleet-infrastructure-onprem | On-premises hardware nodes | MacBook Pro (NODE-LAPTOP-A) — Provisioning |
| fleet-infrastructure-cloud | Cloud relay nodes. Operational workloads | GCP e2-micro (NODE-GCP-CORPORATE) — Active Testing |
| fleet-infrastructure-leased | Dedicated leased servers (future) | — |

The master routing node — iMac 12.1 (NODE-IMAC-12) on the Executive's desk — holds the cryptographic keys for the entire network. It is not a deployable fleet directory. It dials OUT to the cloud relay. The public internet cannot dial IN to it.

## The PointSav Private Network (PPN) Topology

The Private Network is the encrypted mesh that connects all fleet nodes. It runs over WireGuard — an open-source VPN protocol. The key architectural decision (SYS-ADR-13) is that the master routing node and its cryptographic keys physically reside on the Executive's desk (the iMac Command Authority), not on a cloud server.

This means: if the cloud provider disappears tomorrow, the organization retains physical custody of its network keys. The Command Authority dials OUT to the cloud relay — the public internet cannot dial IN to it.

```
  +----------------------------------------------------+
  |  EXECUTIVE DESK (On-premises)                       |
  |  +-----------------------------+                   |
  |  |  iMac 12.1 (NODE-IMAC-12)   |                   |
  |  |  Master Routing Node        |                   |
  |  |  Type-II VM running         |                   |
  |  |  os-network-admin           |                   |
  |  |  Holds: cryptographic keys  |                   |
  |  |  Dials OUT to Node 2        | <-- No inbound    |
  |  +-------------|---------------+    from internet  |
  +-----------------|---------------------------------+
                    | WireGuard tunnel (outbound only)
                    |
  +-----------------v---------------------------------+
  |  CLOUD (GCP / AWS)                                  |
  |  +-----------------------------+                   |
  |  |  Node 2: Cloud Relay        |                   |
  |  |  fleet-infrastructure-cloud |                   |
  |  |  NGINX Cloud Shield         |                   |
  |  |  cluster-totebox-corporate  |                   |
  |  +-----------------------------+                   |
  +----------------------------------------------------+
```

**Network command protocol (SYS-ADR-16):** WireGuard operates as a Layer-3 point-to-point tunnel and cannot route broadcast addresses. The F8 Control Plane therefore uses application-level unicast — it maintains an array of known peer IP addresses and sends each command individually to each node, rather than broadcasting.

## Fleet Node Directory

The full fleet deployment directory maps to the following roles:

| Directory | Type | Purpose |
|---|---|---|
| cluster-totebox-corporate | ToteboxOS | CorporateArchive: financial records, minute books, ledgers |
| cluster-totebox-personnel | ToteboxOS | PersonnelArchive: contacts, identity records |
| cluster-totebox-property | ToteboxOS | PropertyArchive: properties, permits, BIM |
| fleet-infrastructure-cloud | InfrastructureOS | Cloud compute nodes |
| fleet-infrastructure-leased | InfrastructureOS | Dedicated leased server nodes |
| fleet-infrastructure-onprem | InfrastructureOS | On-premises hardware nodes |
| gateway-interface-command | OrchestrationOS | CommandCentre: aggregates archives for administration |
| media-knowledge-corporate | MediaKitOS | Corporate knowledge wiki |
| media-knowledge-distribution | MediaKitOS | Distribution / newsroom |
| media-knowledge-projects | MediaKitOS | Projects wiki |
| media-marketing-landing | MediaKitOS | Public-facing marketing site |
| node-console-operator | ConsoleOS | FKeysConsole operator terminal |
| route-network-admin | NetworkAdminOS | PPN management and MBA registry |
| vault-privategit-source | PrivateGitOS | Air-gapped version control |

---

# Part IX: Operational Guides

## Cold Storage Sync — Pulling Telemetry

Website visitor data and fleet telemetry are collected by a zero-knowledge pipeline (no cookies, no session IDs, no third-party trackers) and stored in the cloud node. The V4 Intent Beacon captures viewport geometry, timezone, device hardware, dwell time, scroll depth, and interaction targets — all via native browser APIs without PII collection. To pull this data to your local machine:

```bash
~/Foundry/pull_sovereign_telemetry.sh
```

This script executes a strict one-way transfer (a "pull diode") — data flows from the cloud to your local machine, never in the other direction. Reports are placed in the `outbox/` directory of the respective fleet node. A 9-day local retention cycle is enforced automatically.

A synthesis step must run first if you want human-readable Markdown reports rather than raw CSV data:

```bash
# Step 1: Trigger synthesis on the cloud node
tool-telemetry-synthesizer.sh

# Step 2: Pull synthesized reports to local machine
tool-telemetry-pull.sh
```

Reports are generated for the following time windows: 1 Day, 1 Week, 30 Days, 60 Days, 90 Days, Year-to-Date, and Inception-to-Date. The Rust telemetry daemon handles backward compatibility automatically — if an older payload format arrives from a cached client, it writes `unknown` or `0` to missing fields rather than failing.

## SLM Point-in-Time Execution

The AI extraction service (`service-slm`) is never run as a persistent daemon. It is triggered once, processes its input, and terminates. This is the "point-in-time execution" standard:

```bash
# Correct: pipe input through the service and route output to the knowledge graph
service-slm < input_payload.txt >> service-content/knowledge-graph/

# Verify the process has terminated before proceeding
```

The service must be invoked via standard input (STDIN) and must route its output to the knowledge graph directory. Verify that the process has terminated before any downstream operations read from the knowledge graph.

## Personnel Ledger — Verification Surveyor

When service-people surfaces an unverified identity fragment (extracted from email), the Verification Surveyor workflow begins:

1. The system displays the extracted text at your terminal — name, company, contact details as the AI understood them.
2. Open your **personal browser** (not a company browser, not via any API) and locate the individual on LinkedIn or a corporate directory.
3. Verify their current employment status matches what was extracted.
4. Paste the verified profile URL back into the terminal.
5. The record is committed to the verified ledger.

The daily throttle is enforced by the system at 10 verifications per day. When the limit is reached, the system will not accept further verification inputs until the following day. This is intentional — do not attempt to bypass it.

## Archive Search

To search across all archived files:

1. Use the F3 (Email) or F2 (People) interface in the FKeysConsole.
2. Type your query in plain English: "Find the lease renewal letter from Apex Plumbing last spring."
3. The SLM translates your query into a search against the inverted index.
4. Results are displayed as a list of matching records. Select any record to export as a CSV Index Card to your desktop.

The search index operates without a running database engine. If you need to perform a search on an air-gapped machine (no network connection), copy the entire `service-search/` index directory to the target machine and run the search binary locally.

## Telemetry Operations — Daily Governance Extraction

To access the latest fleet telemetry from the primary Management Terminal:

```bash
~/Foundry/pull_sovereign_telemetry.sh
```

Reports manifest in the `outbox/` directories of the respective fleet nodes. The telemetry architecture uses a 4-tier routing model:

1. **Ingest (Cloud):** NGINX relays capture organic JSON payloads at the edge.
2. **Transit (Cryptographic Tunnel):** Payloads traverse the WireGuard mesh.
3. **Process (Local Execution):** The Rust telemetry daemon ingests packets to local CSV ledgers.
4. **Govern (Control Node):** The Command Authority executes the Master Diode pull.

## VPN Setup — F8 Network Command

The F8 interface in the FKeysConsole is the terminal for the PPN (PointSav Private Network). The system-slm functions exclusively as a compiler here — it reads human intent and outputs a deterministic JSON proposal. The underlying mesh architecture rejects any command that has not received explicit human authorization. It supports natural-language commands:

**Example: Health Check**
- Type: `Check the health of the network.`
- The system proposes: `[PROPOSED] ACTION: PING, TARGET: ALL`
- Review the proposed action and click EXECUTE.
- All active nodes reply with CPU, RAM, and routing status.

**Example: Node Isolation**
- Type: `Lock down the laptop node immediately.`
- The system proposes: `[PROPOSED] ACTION: ISOLATE, TARGET: NODE-LAPTOP-A`
- Review and confirm.
- The target node drops all routing tables except the master link to the Command Authority.

Every F8 command follows the Two-Step Protocol: (1) submit your intent in plain English, (2) review the machine-translated action, (3) explicitly authorize execution. The system will never execute a network command without your visible confirmation.

## Physical Egress — Regulatory Printing

When printing official documents from the web interface for regulatory submission, enforce the following browser print configuration to achieve 1:1 parity with the official PDF format:

| Setting | Value |
|---|---|
| Orientation | Portrait |
| Headers/Footers | OFF |
| Margins | Default or exactly 0.5 in |
| Background Graphics | As needed per document |

The CSS print stylesheet handles the rest automatically — it hides the digital infrastructure block and ensures the contact block renders directly before the legal disclosures.

---

# Part X: Architecture Decisions

Architecture Decision Records (ADRs) are formal commitments to specific design choices. They are not proposals — they represent decisions already made and implemented. Each ADR includes the problem it solves and the rule it establishes.

## SYS-ADR-06: Immutable Ledgers vs. Self-Healing Intelligence

**The problem:** A compliance archive and an active intelligence system have incompatible requirements. A compliance archive must never change — its value as evidence depends on it being untouched. An active intelligence system must constantly update — its value depends on reflecting current reality.

**The decision:** These are two physically separate systems that serve completely different purposes.

- `service-email` is the **Compliance Layer**: an immutable WORM (Write Once, Read Many) archive of every raw email received. It is a legal record. It is mathematically forbidden from modification.
- `service-content` is the **Intelligence Layer**: a self-healing knowledge graph that continuously updates as new information arrives. Old facts are replaced with current truths to give the AI a clean, accurate picture of the present.
- `service-slm` is the **Bridge**: it reads the compliance layer, extracts the intelligence, and writes exclusively to the intelligence layer. The bridge only flows in one direction.

**The implication:** Never query the compliance archive for intelligence purposes. It will return noise. Never expect the intelligence layer to serve as a legal record. It will not be immutable.

## SYS-ADR-07: Bifurcated Ingestion (Two Roads for Data)

**The problem:** Applying an AI model to every piece of incoming data is wasteful and introduces errors. A spreadsheet with financial figures should be parsed with a deterministic algorithm that guarantees 100% accuracy. Applying an AI to it introduces a small but unacceptable probability of misreading a number.

**The decision:** Every inbound payload is classified before processing. The classification determines which engine handles it.

- **Path A (Deterministic):** Structured data (CSV, JSON, XLSX, telemetry logs) goes to dedicated Tier-5 services using standard parsing algorithms. The AI is forbidden from this path.
- **Path B (Probabilistic):** Unstructured human text (email body, PDF, Word documents) goes to `service-slm` for semantic extraction.

All inbound compound files (e.g., MIME multipart .eml files) must be vaulted in their raw, unaltered state before extraction logic is applied. Splintering and routing occurs strictly as a secondary, non-destructive read operation.

**The implication:** Before processing any inbound data, determine its type. Do not route structured data through the AI. Do not apply rigid parsing rules to human text.

## SYS-ADR-08: Systemd Quarantine

**The problem:** The cloud relay servers run Debian Linux, which is permanently entangled with `systemd` — a Linux process supervisor that has absorbed network management, logging, and other responsibilities into a 1.5-million-line monolith. This conflicts with the microkernel isolation principle.

**The decision:** `systemd` is formally classified as a Quarantined Foreign Component — a piece of technical debt that is accepted temporarily. It is used for the 5-second automated ingestion loops because removing it from Debian would cause a system failure. It is not used for anything beyond basic process supervision.

**The implication:** Do not add new `systemd` integrations. Do not rely on `systemd` for any function beyond basic process management. When the cloud relays are migrated to a FreeBSD substrate or native seL4, `systemd` will be replaced with minimalist text-based supervision (rc.d or runit).

## SYS-ADR-10: The Fiduciary Anchor (F12 Input Machine)

**The problem:** Fully automated AI categorization of documents is a fiduciary liability. If an AI autonomously assigns a source document to the wrong account in the Chart of Accounts, every downstream calculation, report, and compliance record built on that document is wrong. By the time the error is discovered, it may have propagated through years of records.

**The decision:** The F12 Input Machine is the mandatory human checkpoint for all base asset ingestion. The human operator must manually select three things before any document enters cold storage: (1) the destination ToteboxArchive, (2) the Totebox Service, and (3) the Chart of Accounts category.

The SLM may silently suggest a classification while the operator reviews the document. It is strictly forbidden from executing the commit. The human must physically authorize the ledger entry.

**The implication:** There is no batch import path that bypasses F12. Every base asset enters the system through a human decision point.

## SYS-ADR-11: Micro-Frontend Cartridge Architecture

**The problem:** A monolithic user interface becomes increasingly fragile as it grows. A bug in one feature can break an unrelated feature. Deploying a small update requires re-deploying and re-testing the entire interface.

**The decision:** The ConsoleOS is split into two physically separate components:

- **The Chassis** (`os-console`): A static shell with no business logic. It holds the layout, routing rules, and authentication parameters. When an operator presses an F-key, the Chassis fetches the corresponding Cartridge.
- **The Cartridges** (`app-console-*`): Isolated fragments of HTML and JavaScript. Each Cartridge is self-contained and knows nothing about the broader system. The Chassis mounts them on demand using the native browser `fetch()` API.

**The implication:** Each F-key function is a separate deployable unit. Updating F4 does not affect F12. Adding a new F-key function means creating a new Cartridge, not modifying the Chassis. Each Cartridge must be completely self-contained — no shared state with other Cartridges.

## SYS-ADR-13: Type-II Hypervisor Command Authority

**The problem:** If the master routing node and cryptographic keys reside on a commercial cloud provider, the cloud provider ultimately owns the network. A state actor or corporate breach at the hyperscaler level would compromise the entire PPN.

**The decision:** The Command Authority (Node 3) runs as a Type-II VM within the secure Foundry Host (an iMac on the Executive's desk). It dials OUT to the cloud relay (Node 2) to join the mesh. The public internet cannot dial IN to the Command Authority.

**The implication:** The Executive retains absolute, physical custodial control over the network's cryptographic keys. The Command Authority is never hosted on infrastructure the organization does not physically possess.

## SYS-ADR-14: Asynchronous UI/API Port Splitting

**The problem:** Routing both static UI requests and live API commands through a single backend port forces the Rust binaries to act as file servers, violating memory isolation and increasing the attack surface.

**The decision:** The NGINX Cloud Shield routes traffic asymmetrically based on the URI path:
- Port 8888: Static HTML/CSS/JS Chassis (the UI frame)
- Port 8080: Active Rust daemon for secure payload execution (API calls route down the WireGuard mesh)

**The implication:** The UI and the API are physically decoupled at the proxy layer. A vulnerability in the static file server cannot reach the Rust execution engine.

## SYS-ADR-15: Violent Cache-Busting Protocol

**The problem:** Modern web browsers aggressively cache HTML/JS files behind Basic Authentication. This creates a "Cache Phantom" where the operator sees an outdated UI state, potentially leading to catastrophic routing errors or misaligned physical execution.

**The decision:** Two-layer cache destruction:
1. NGINX injects violent cache-destroying headers into every outbound UI payload (no-store, no-cache, max-age=0).
2. The client-side `fetch()` API appends a dynamic timestamp parameter (`?v=Date.now()`) to physically force the browser to request a fresh asset.

**The implication:** The operator always sees the current state. There is zero tolerance for stale UI rendering in a system where operators issue physical network commands.

## SYS-ADR-16: Application-Level Unicast over WireGuard

**The problem:** WireGuard operates strictly as a Layer-3 point-to-point cryptographic tunnel. Native subnet broadcasts (e.g., `10.50.0.255`) fail because the kernel cannot assign a specific peer's public key to a broadcast address.

**The decision:** The F8 Control Plane maintains an array of known peer IP addresses and executes a high-speed unicast loop, injecting the payload to each node individually rather than broadcasting.

**The implication:** UDP broadcast addresses are forbidden. Every network command is delivered individually to each peer. This adds slight complexity but guarantees reliable delivery across the WireGuard mesh.

## SYS-ADR-17: The Derivative Architecture

**The problem:** Corporate data needs to serve two conflicting purposes: (1) preserve the exact original documents for legal and compliance reasons, and (2) provide a clean, current, queryable view of the organization's operational reality.

**The decision:** The Totebox Archive is modeled on financial derivatives with three layers:
- **Base Asset (Ground Truth):** Immutable physical files (.pdf, .eml) locked to disk via F12 Input Machine and service-email cold storage.
- **First Derivative (Structural Index):** Archetypes, Chart of Accounts, Domains, and Themes — synthesized passively by the SLM reading the Base Assets. A self-healing, autonomous map of the company's reality.
- **Third Derivative (Output and Utility):** Generated content (memos, emails via F4 Drafting) and pristine CSV/MD exports for data marketplaces.

**The implication:** Dynamic taxonomy is the synthesized output of the system, not a manual data-entry requirement. The operator does not maintain the knowledge graph — the system maintains it autonomously from the ground truth.

## SYS-ADR-18: The Command Ledger HUD

**The problem:** Traditional web applications trap operators in a destination — a web page they must navigate and interact with using forms, menus, and dropdowns. This is slow, fragile, and incompatible with the keyboard-driven operational model.

**The decision:** The os-console is a strict Heads-Up Display (HUD) — a window that routes physical data between the operator and the underlying Derivative Architecture. Each F-key maps to a specific layer:
- F12 Input: Grounding Anchor (Base Assets)
- F3 Email / F2 People: Querying the Index (First Derivative, output as CSV Index Cards)
- F4 Drafting: Forging the Third Derivative (output as Markdown files)

**The implication:** The ConsoleOS is not a destination. It is a routing terminal. All outputs are physical files dropped to the operator's desktop.

## SYS-ADR-19: The Verification Airgap (Git for Knowledge)

**The problem:** Autonomous AI systems ("Agentic AI") that can read, write, and publish data are a fiduciary liability. Automated merging introduces invisible hallucination creep into verified corporate ledgers.

**The decision:** A Git-like Checkout/Commit workflow for institutional knowledge:
- **Checkout:** The F8 Command Ledger presents AI-staged "Overlays." Operators physically eject drafts to their local, air-gapped desktop for native editing.
- **Commit:** The F12 Input Machine is the Commit Gateway. Operators drag-and-drop verified files back into the Totebox to overwrite LTS ledgers.
- Automated AI publishing to verified ledgers is forbidden.

**The implication:** Totebox Orchestration (os-totebox, os-console) is strictly isolated from Independent Systems (os-mediakit, os-privategit). Independent Systems act solely as downstream consumers of F12-verified LTS ledgers.

---

# Part XI: The Road Ahead — Moonshots

## The Philosophy

Every third-party dependency is a liability. Not a theoretical liability — a practical one. If Microsoft changes its Graph API, `service-email` breaks. If a cloud provider deprecates a service, the infrastructure must be rebuilt. If a hardware vendor's driver stops working, the system stops working.

The Replacement Programme is the ongoing effort to eliminate these dependencies one by one. Each dependency is quarantined, tracked, and replaced with a native alternative when that alternative reaches functional parity.

## Technical Debt Ledger

The monorepo tracks quarantined vendor dependencies and their corresponding moonshot replacements:

| Quarantined Dependency | Vendor Directory | Replacement Target | Moonshot Directory |
|---|---|---|---|
| Microsoft Graph API (email) | `vendor-microsoft-graph` | Native mail transport | `moonshot-protocol` |
| Azure AD (authentication) | `vendor-azure-auth` | Machine-Based Authorization (MBA) | `system-mba-shim` (active) |
| systemd (process supervision) | `vendor-linux-systemd` | rc.d or runit on FreeBSD / seL4 | `moonshot-kernel` |
| GPU drivers | `vendor-gpu-drivers` | Native GPU interface | `moonshot-gpu` |
| Phi-3 Mini (SLM engine) | `vendor-phi3-mini` | Native language model | `moonshot-toolkit` |
| SLM vendor engine | `vendor-slm-engine` | Native SLM execution | `moonshot-toolkit` |
| seL4 kernel (reference) | `vendor-sel4-kernel` | Production seL4 VMM integration | `moonshot-sel4-vmm` |
| VirtIO (virtualization) | `vendor-virtio` | Native hypervisor | `moonshot-hypervisor` |
| WireGuard (VPN) | `vendor-wireguard` | Native private mesh | `moonshot-network` |
| MaxMind GeoIP | `app-mediakit-telemetry/assets/` (GeoLite2) | Native geographic resolution | `moonshot-index` |
| Databases (PostgreSQL, etc.) | (classified as debt) | Flat-file state machine (partial parity) | `moonshot-database` |

## The Moonshot Pipeline

For each quarantined dependency, an engineering initiative is underway in the corresponding `moonshot-*` directory. These are real engineering projects — not aspirational notes. When a Moonshot component achieves structural parity with the component it is replacing, it physically replaces the quarantined vendor code.

The Sovereign Data Foundation in Denmark is intended to audit this pipeline to verify that the organization is on a credible trajectory toward operational independence.

## Where the Architecture Is Going

The end state is a platform where:

1. Every component runs as its own kernel-isolated virtual machine.
2. Every archive is exportable as a Bootable Disk Image — portable, proprietary-free, and bootable on any hypervisor.
3. Authentication is entirely machine-based — no passwords, no credential databases, no social engineering vectors.
4. Every software dependency is either owned outright or replaceable without rewriting business logic.
5. The data pipeline operates entirely within the organization's hardware perimeter — AI assistance is local, external models receive only anonymized structural skeletons.

None of this requires over-engineering today. It requires awareness: choosing an interface boundary over a direct function call, a configuration file over a hardcoded connection string, an open file format over a proprietary one. Each of these is a low-cost choice now that keeps the path to the end state clear.

---

# Appendix: Glossary

| Term | Plain-Language Definition |
|---|---|
| Archive Preset | One of three standard ToteboxArchive configurations: PersonnelArchive, CorporateArchive, or PropertyArchive. Each is pre-loaded with the right services for its asset type. |
| Assignment | What PointSav calls a task or work ticket. Assignments must compile and function before they are considered complete. |
| Base Asset | The raw, immutable original file — an email (.eml), a contract (.pdf), a spreadsheet (.xlsx). Stored in cold storage via F12. Cannot be altered after entry. |
| BIM | Building Information Modelling. A digital 3D representation of a physical building that includes structural, mechanical, and spatial data. |
| Bootable Disk Image | A VM image file that contains a complete operating system and can be started on any compatible hypervisor without additional software. The end-state artifact for Freely Transferable archives. |
| Capability-Based Security | A security model where each software component must hold a cryptographic token (a "capability") to communicate with any other component. No capability means no communication, regardless of network access. |
| Cartridge | An isolated HTML/JS micro-frontend fragment loaded into the Console OS Chassis on demand via fetch(). Each cartridge is self-contained and unaware of the broader system (SYS-ADR-11). |
| Chart of Accounts | A structured list of financial and operational categories used to classify every document and transaction. Requires Executive override to change (update rate: 18-24 months). |
| Chassis | The empty UI frame (os-console) holding CSS, F-Key routing, and MBA parameters. Contains zero business data. The "picture frame" into which Cartridges are mounted (SYS-ADR-11). |
| CommandCentre | The primary OrchestrationOS variant. Aggregates PersonnelArchives and CorporateArchives for administrative use. |
| ConsoleOS | The user-facing delivery layer. Each variant is a purpose-built interface for a specific workflow. |
| Contributor | A PointSav developer or designer. Individual contributors, not agency teams. |
| Control Valve | One of four CSV ledgers (Archetypes, Chart of Accounts, Domains, Themes) governing the self-healing speed of service-content. Each updates at a different rate to preserve longitudinal data stability. |
| CorporateArchive | A ToteboxArchive for a legal entity. Anchored to a Business Incorporation Number or Tax ID. Contains financial records, minute books, and statutory ledgers. |
| CostingEmail | What PointSav calls a statement of work or work order. The commercial document that defines an assignment. |
| Customer | An organization that pays for institutional-scale Totebox Orchestration using proprietary OrchestrationOS components. |
| DARP | Data Access and Retention Protocol. The compliance standard governing how data must be stored, accessed, and retained. Requires data to be searchable without proprietary software. |
| Derivative Architecture | The three-tier data model: Base Assets (ground truth) -> First Derivative (indexed knowledge) -> Third Derivative (generated outputs). Defined in SYS-ADR-17. |
| DiodeStandard | The one-way command flow principle. Data moves in one direction only: from source to destination. No reverse channel. Applies to both the telemetry pull and the overall security architecture. |
| F-Key | A hardware function key on the keyboard that activates a specific context in the FKeysConsole. F2 = People, F3 = Email, F4 = Drafting, F8 = Network, F12 = Input Machine. |
| First Derivative | The processed, self-healing knowledge graph derived from Base Assets by service-slm. Contains the organization's current operational reality in a clean, queryable form. Governed by the four Control Valves. |
| FKeysConsole | The primary ConsoleOS variant for administrative work. A keyboard-driven HUD where each function key activates a different layer of the Derivative Architecture (SYS-ADR-18). |
| Freely Transferable | The non-negotiable standard that every ToteboxArchive must be exportable as a complete, self-contained package deployable without proprietary runtimes or vendor relationships. |
| GeneralStaff | The Woodfine employees who use the platform for daily operations. |
| Geometric Security | Peter Woodfine's term for the Machine-Based Authorization model. The topology of the network defines the access control. |
| GIS | Geographic Information System. Software for capturing, storing, and analyzing spatial and geographic data. |
| InfrastructureOS | The virtualization substrate. The operating system that physical hardware runs. Provides the environment for all other components. |
| OrchestrationOS | The stateless logic and compute layer. Holds no data. Connects ConsoleOS terminals to one or more ToteboxArchives and provides extended compute capacity for BIM, GIS, SLM, and data warehouse operations. Required for multi-archive use cases (the monetization boundary). |
| Inverted Index | A search structure where each word in a corpus maps to a list of files containing that word — like the index at the back of a textbook. Enables microsecond search without a running database. |
| IoTConnect | An OrchestrationOS variant that bridges IoT sensor data into the ToteboxArchive system. |
| IssuerSnap | See: TheSnap. |
| Machine-Based Authorization (MBA) | A security model where access is granted through cryptographic hardware pairing between machines, not through usernames and passwords. |
| MediaKitOS | The web framework and CMS layer for public-facing web properties and Reporting Issuers' disclosure obligations. |
| Minute Book | The corporate governance record required by law for any incorporated entity. Records resolutions, meetings, and structural changes of the corporation. |
| Moonshot | An engineering initiative to build a native replacement for a quarantined third-party dependency. Named `moonshot-*` in the codebase. Prohibited from production until parity is achieved. |
| NetworkAdminOS | The management layer for the Private Network. Maintains the MBA registry, monitors node health, and provides fleet control interfaces. |
| Operator | Someone who manages a ToteboxOrchestration at the infrastructure level. Distinct from an Owner (who owns the archives). |
| Owner | The entity that owns the ToteboxArchives. Distinct from the Operator who administers them. |
| PairingAsPermission | The core security principle: access is determined by which machines are cryptographically paired to which, not by user credentials or role assignments. |
| PersonnelArchive | A ToteboxArchive for an individual person. Anchored to a SIN (Social Insurance Number) or Passport ID. Contains professional network records, identity data, and communication history. |
| PointSav Private Network (PPN) | The encrypted mesh network connecting all fleet nodes. Runs over WireGuard. The master routing node resides on the Executive's physical desk (SYS-ADR-13). |
| PrivateGitOS | The self-hosted version control system. Preferred on-premises for physical IP possession. |
| RAG | Retrieval-Augmented Generation. An AI technique where a model is given relevant context documents before generating a response, rather than relying solely on its training data. |
| PropertyArchive | A ToteboxArchive for a physical property. Anchored to a Land Title PIN or legal address. Contains permits, lifecycle records, BIM drawings, IoT data, lease register, and maintenance history. |
| Reporting Issuer | A publicly-traded limited partnership that is required by securities law to disclose financial and operational information to investors on a regular schedule. Woodfine LPs are Reporting Issuers. |
| seL4 | A mathematically proven microkernel. The seL4 security properties have been formally verified using machine-checked mathematical proofs — not just tested but proven. Reference point for the ToteboxOS kernel. |
| ServiceProvider | External professionals (lawyers, accountants, architects, trades) who interact with the organization. |
| SLM | Small Language Model. A compact AI model (sub-one-billion parameters) that runs entirely on the organization's own hardware. Used for text extraction and AI routing. Operates as a point-in-time process that terminates after execution. |
| Verification Airgap | A Git-like Checkout/Commit workflow for institutional knowledge. Operators check out AI-staged drafts to their local desktop, verify them, and commit back via F12. Automated AI publishing to verified ledgers is forbidden (SYS-ADR-19). |
| TheSnap | The cross-archive integrity verification mechanism. Pulls verified data from Corporate and Property Archives, cross-references against Personnel Archives, and generates authenticated quarterly reports. Used for Reporting Issuer compliance. |
| Third Derivative | Generated outputs synthesized from the First Derivative: drafts, memos, reports, CSV exports for data markets. |
| ToteboxArchive | A self-contained data store for one specific asset. Contains both the data and the services that process it. The unit of ownership, backup, and export. |
| ToteboxOrchestration | A complete, interconnected deployment of all platform components — ToteboxOS archives, OrchestrationOS hubs, ConsoleOS terminals, and NetworkAdminOS. |
| ToteboxOS | The operating system of a ToteboxArchive. In the end state, a unikernel running as a kernel-isolated virtual machine. |
| Unikernel | A minimal operating system containing only the kernel, the libraries, and the application code needed for one specific purpose. No shell, no package manager, no multi-user support. Fast to boot, tiny attack surface. |
| Vendor-* | Quarantined third-party dependencies. Isolated in their own directories. Tracked as technical debt with corresponding moonshot replacements. |
| Verification Surveyor | The human-in-the-loop checkpoint for identity records. Operators verify extracted person records using their own browser. Throttled to 10 verifications per day to ensure quality. |
| WorkplaceOS | A Linux-based desktop environment for General Staff. Separate from Totebox Orchestration. The alternative delivery path for users who prefer a full desktop OS over running ConsoleOS as a virtual machine. |
| WORM | Write Once, Read Many. A data storage principle where records cannot be modified after they are written. The compliance archive (service-email) is WORM. Enforced by SHA-256 cryptographic checksums. |
| WireGuard | An open-source VPN protocol used to create the encrypted tunnels of the PointSav Private Network. Operates as a Layer-3 point-to-point tunnel (SYS-ADR-16). |
| WoodfineLP | A limited partnership within the Woodfine Capital Projects network. These are the Reporting Issuers subject to securities disclosure requirements. |
| Zero-Form | UI paradigm rejecting traditional web forms. Data entry via file drop, queries via natural language, outputs as physical files to the desktop. The operator works at the speed of language, not at the speed of a form wizard. |

---

## CHANGELOG

### 2026-03-30_14-31-28
- **Added**: ADRs 13-19 to Part X (Type-II Hypervisor Command Authority, Port Splitting, Cache Busting, Unicast over WireGuard, Derivative Architecture, Command Ledger HUD, Sovereign Airgap)
- **Added**: Sovereign Airgap section in Part VII covering Git-like Checkout/Commit workflow (SYS-ADR-19)
- **Added**: Port splitting and cache busting details in Part VII Cartridge Architecture section (SYS-ADR-14, SYS-ADR-15)
- **Added**: Cryptographic Payload Attestation subsection in Part V for client-side SHA-256 verification of public disclosures
- **Added**: Sovereign AI Routing paragraph in service-slm section explaining anonymized cloud routing with local re-hydration
- **Added**: Diode Standard subsection in Part IV Security Model
- **Added**: 4-tier telemetry routing model in Part IX Telemetry Operations
- **Added**: Glossary entries for Cartridge, Chassis, Control Valve, Sovereign Airgap, Zero-Form with ADR cross-references
- **Changed**: Technical Debt Ledger in Part XI updated to match actual monorepo vendor-*/moonshot-* directories (11 entries vs previous 5)
- **Changed**: ConsoleOS description updated from "Linux applications" to "browser-based HUD" to align with SYS-ADR-18 and implementation
- **Changed**: SYS-ADR-07 entry expanded to include forensic chain of custody rule (raw files vaulted before extraction)
- **Changed**: MBA shim noted as active workspace crate (system-mba-shim) providing conventional auth behind clean interface
- **Changed**: Verification Surveyor section expanded with API-avoidance rationale from topic-verification-surveyor.md
- **Removed**: Generic moonshot-message-transport, moonshot-auth, moonshot-unikernel, vendor-database, vendor-containers entries that did not correspond to actual monorepo directories
- **Sources**: All 12 ADR files (sys-adr-06 through sys-adr-19), 20 topic files, monorepo Cargo.toml and directory structure, TOPIC_TELEMETRY_ARCHITECTURE.md, topic-crypto-attestation.md, topic-sovereign-ai-routing.md

---

*End of User Guide*
