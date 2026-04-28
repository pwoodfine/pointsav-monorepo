---
schema: foundry-draft-v1
state: draft-pending-language-pass
originating_cluster: project-data
target_repo: content-wiki-documentation
target_path: ./
target_filename: topic-worm-ledger-architecture.es.md
audience: vendor-public
bcsc_class: current-fact
language_protocol: TRANSLATE-ES
companion_english_draft: topic-worm-ledger-architecture.draft.md
authored: 2026-04-28T00:30:00Z
authored_by: task-project-data (session e509c13609b4b632)
authored_with: opus-4-7
references:
  - topic-worm-ledger-architecture.draft.md  # canonical English bulk draft (companion)
  - ~/Foundry/conventions/cluster-wiki-draft-pipeline.md  # §2.1 audience routing
  - ~/Foundry/conventions/project-tetrad-discipline.md  # claim #37 wiki-leg requirement
  - ~/Foundry/DOCTRINE.md  # §XII Strategic Adaptation — Spanish overview, not 1:1 translation
notes_for_editor: |
  SKELETON ONLY per Tetrad backfill discipline (2026-04-28 Master
  inbox v0.0.10 / claim #37). The English bulk draft is at
  topic-worm-ledger-architecture.draft.md — substance is there.
  This Spanish overview reserves the structural slot per CLAUDE.md
  §6 bilingual rule + DOCTRINE §XII strategic-adaptation pattern
  ("Spanish is overview, not 1:1 translation").

  Why I'm staging a SKELETON not a bulk Spanish translation:

  - v0.1.31 drafts-outbound discipline ("Don't generate the
    bilingual `.es.md` — project-language generates per DOCTRINE
    §XII") forbids me from authoring a full Spanish translation.
    project-language is the editorial gateway that adapts the
    refined English version into the Spanish overview.
  - v0.0.10 / claim #37 Tetrad backfill discipline asks for both
    English + Spanish overview file presence as part of the
    skeleton signal of intent.
  - Resolution: this file IS the skeleton. Headings + cluster-
    relevant Spanish-language placeholders + a clear marker that
    project-language generates the substantive Spanish overview
    when it refines the canonical English at
    topic-worm-ledger-architecture.draft.md. No content
    commitment beyond reserving section structure + flagging a
    handful of terminology choices for the editor's reference.

  Spanish-overview structural template per DOCTRINE §XII:
  Spanish should orient a Spanish-reading technical audience to
  the architecture WITHOUT being a 1:1 translation of the
  English. Acceptable to drop technical-implementation depth that
  the English carries; preserve (1) las cuatro capas (the four
  layers), (2) los dos sobres de arranque (the two boot
  envelopes), (3) la trazabilidad pública vía Sigstore Rekor
  (public verifiability via Sigstore Rekor anchoring), (4)
  alineación estructural con normas WORM externas (structural
  alignment with external WORM standards).

  Suggested Spanish title: "El Substrato WORM de Foundry —
  Arquitectura del Ledger Inmutable de Cuatro Capas". Open to
  project-language editorial preference.

  Terminology choices for project-language to ratify (Spanish
  technical register varies by audience; these are first-pass
  proposals):
    - "WORM ledger" → "ledger inmutable WORM" (keep WORM as
      abbreviation; "ledger" reads as English loanword in modern
      Spanish-language fintech writing — alternatives: "registro
      inmutable WORM", "libro mayor inmutable")
    - "boundary-ingest service" → "servicio de ingestión de
      frontera" (literal) or "servicio de borde de ingesta"
    - "checkpoint" → "punto de control" (loanword acceptable too)
    - "anchoring" → "anclaje" (well-established in cryptographic
      Spanish)
    - "shard" (Rekor v2 year-shard) → "fragmento anual" or just
      "shard" (English loanword common in distributed-systems
      Spanish)
    - Ring 1 / Ring 2 / Ring 3 → "Anillo 1 / Anillo 2 / Anillo 3"
      (consistent with three-ring-architecture convention's
      Spanish references)
    - "tenant" → "tenant" (English loanword) or "inquilino"
      (literal) — multi-cloud Spanish writing increasingly uses
      the English form

  This is a skeleton-stage staging file only. project-language's
  refinement pass produces the final Spanish overview as a
  derivative of the refined English canonical.
---

# El Substrato WORM de Foundry — Arquitectura del Ledger Inmutable de Cuatro Capas

> **Estado del borrador:** SKELETON pendiente del pase editorial
> de project-language. La sustancia técnica completa vive en el
> borrador canónico en inglés (`topic-worm-ledger-architecture.draft.md`).
> Este archivo en español reserva el espacio estructural por
> CLAUDE.md §6 (regla bilingüe) + DOCTRINE §XII (patrón de
> adaptación estratégica: el español es resumen, no traducción
> 1:1). project-language genera la versión en español refinada
> a partir del inglés canónico.

## 1. Posición en el sistema

(draft-pending — sustancia en hito N+1 tras pase de project-language)

`service-fs` es el ledger inmutable WORM (Write-Once-Read-Many) por
tenant del Anillo 1 al que escriben todos los servicios de
ingestión de frontera. Diagrama estructural por reproducir desde la
versión canónica en inglés.

## 2. Las cuatro capas

(draft-pending — sustancia en hito N+1)

- L1 — primitiva de almacenamiento de tiles (formato C2SP tlog-tiles)
- L2 — API del ledger WORM (trait Rust independiente del objetivo)
- L3 — protocolo de cable (HTTP/MCP por tenant)
- L4 — anclaje (anclaje mensual a Sigstore Rekor v2 — Doctrina
  Invención #7)

## 3. Los dos sobres de arranque

(draft-pending — sustancia en hito N+1)

- Sobre A — daemon Linux/BSD bajo systemd (hoy)
- Sobre B — unikernel seL4 Microkit Protection Domain (largo plazo)

Ambos sobres comparten formato de almacenamiento y protocolo de
cable; sólo difieren el runtime y el mecanismo de I/O de
almacenamiento.

## 4. Formato de tile — C2SP tlog-tiles

(draft-pending — sustancia en hito N+1)

Mismo formato usado por RFC 9162 v2 (Certificate Transparency v2),
Trillian-Tessera, y Sigstore Rekor v2. Texto plano, base64,
inspeccionable con herramientas Unix estándar.

## 5. Formato de checkpoint — C2SP signed-note

(draft-pending — sustancia en hito N+1)

Firmado por la clave del tenant; testigos coordinados (workspace
de Foundry + tercero elegido por el cliente).

## 6. Flujo de escritura y lectura

(draft-pending — sustancia en hito N+1)

## 7. Sub-ledger de auditoría ADR-07

(draft-pending — sustancia en hito N+1)

## 8. Agilidad criptográfica

(draft-pending — sustancia en hito N+1)

## 9. Alineación con normas WORM externas

(draft-pending — sustancia en hito N+1)

Alineación estructural con SEC Rule 17a-4(f) (corredores-agentes
estadounidenses) y servicio cualificado de preservación de eIDAS
(Reglamento de Ejecución (UE) 2025/1946). Foundry no afirma
certificación; la conformidad es estructural, no de política.

## 10. Trayectoria de largo plazo

(draft-pending — sustancia en hito N+1; afirmación prospectiva
sujeta a las advertencias del régimen de divulgación BCSC)

El sobre B (unikernel seL4) y la persistencia con conciencia de
capacidades vía `moonshot-database` son elementos de la trayectoria
v1.0.0+ por MEMO §7 Active Sovereign Replacements.

## 11. Referencias

Ver el borrador canónico en inglés
(`topic-worm-ledger-architecture.draft.md`) para la lista completa
de referencias internas (convenciones Foundry) y externas (especs
C2SP, RFC, ETSI / CEN, Sigstore Rekor v2, etc.).

---

*Skeleton autorizado 2026-04-28 por la sesión Task de project-data
para satisfacer el requisito de la pierna `wiki:` del Tetrad de
proyecto (Doctrina v0.0.10 / claim #37). project-language genera
el contenido en español refinado a partir del pase de refinamiento
del canónico en inglés.*
