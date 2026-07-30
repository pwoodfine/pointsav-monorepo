# service-fs

[ 🇬🇧 Read this document in English ](./README.md)

Servicio de ingesta de límites Ring 1: el Libro Mayor Inmutable
WORM (Write-Once-Read-Many) por inquilino a través del cual
escriben los demás servicios Ring 1 (`service-people`,
`service-email`, `service-input`). Los consumidores Ring 2
(`service-extraction`) leen del libro mayor; nunca tocan el
servicio originador directamente.

## Posición en la arquitectura

- **Ring:** 1 (ingesta de límites, por inquilino) — ver
  `~/Foundry/conventions/three-ring-architecture.md`.
- **Multi-inquilino:** un proceso por `moduleId`; el límite por
  inquilino se aplica tanto por infraestructura (procesos
  separados) como por verificación del encabezado
  `X-Foundry-Module-ID` en cada solicitud.
- **Tiempo de ejecución:** servidor HTTP Tokio + axum alojado bajo
  systemd (per
  `~/Foundry/conventions/zero-container-runtime.md`).
- **Protocolo de cable:** JSON sobre HTTP hoy; interfaz
  MCP-server superpuesta según el contrato Ring 1 (siguiente
  elemento NEXT.md después de que `cargo check` pase limpio).

## Reglas estrictas

- **ADR-07: cero IA en Ring 1.** Sin inferencia de LLM, sin
  filtrado basado en incrustaciones, sin normalización asistida
  por IA.
- **Invariante de sólo-añadir.** Ninguna API pública muta o
  elimina una entrada previamente persistida. Las pruebas en
  `src/ledger.rs` aplican la invariante a nivel de API.
- **Límite por inquilino.** La variable de entorno
  `FS_MODULE_ID` es requerida; las solicitudes cruzadas entre
  inquilinos se rechazan con 403.

## Estado

Activo desde 2026-04-25 (`ee209e3`). El esqueleto MCP-server
Tokio alojado aterrizó el 2026-04-26 según la Decisión 1 del
Master Claude, reemplazando el andamiaje anterior de unikernel
seL4 bare-metal (reubicado a `vendor-sel4-fs/` según la Decisión
2 del Master). La membresía del workspace está retenida hasta
que `cargo check` pase limpio (Decisión 3 del Master).

## Estándares y postura de cumplimiento

`service-fs` apunta a dos estándares WORM externos más los
Criterios de Servicios de Confianza SOC 2 más relevantes para el
almacenamiento inmutable:

- **SEC Rule 17a-4(f)** (EE.UU., registro electrónico de
  broker-dealer; enmienda 2022 vigente desde 2023-05-03).
- **Servicio cualificado de preservación eIDAS** (UE; Reglamento
  de Ejecución 2025/1946 vigente desde 2026-01-06).
- **SOC 2 TSC** — CC6, CC7, PI1, PI4.

Más postura interna de Foundry: cumplimiento legal WORM por
MEMO §6.3; DARP por DOCTRINE §IX; ADR-07 cero-IA en Ring 1;
Pilar 1 sólo texto plano; Pilar 2 legibilidad de 100 años;
Invención #7 anclaje mensual a Sigstore Rekor.

Postura completa en `SECURITY.md`. Lo que NO se promete hoy se
indica explícitamente allí.

## Arquitectura

Pila de cuatro capas — **L1** almacenamiento por azulejos
(POSIX hoy, `moonshot-database` mediado por capacidades a largo
plazo); **L2** trait Rust del Libro Mayor WORM (contrato
independiente del objetivo); **L3** protocolo de cable (axum
HTTP hoy, MCP-server superpuesto); **L4** anclaje mensual a
Sigstore Rekor (a nivel de workspace según Invención #7).

Dos envolturas de arranque comparten el mismo protocolo de cable
y el mismo formato de azulejo: **Envoltura A** demonio Linux/BSD
bajo systemd (hoy); **Envoltura B** dominio de protección
unikernel seL4 Microkit (a largo plazo, nativo del Totebox
Archive).

Resumen completo en `ARCHITECTURE.md`. Síntesis completa con
alternativas consideradas + diez decisiones de ratificación para
el Master en `RESEARCH.md`.

## Licencia

Consulte el archivo `LICENSE` del repositorio. La asignación de
licencias a nivel de componente está regida por
`LICENSE-MATRIX.md` en
`pointsav/factory-release-engineering`.
