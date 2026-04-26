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

## Licencia

Consulte el archivo `LICENSE` del repositorio. La asignación de
licencias a nivel de componente está regida por
`LICENSE-MATRIX.md` en
`pointsav/factory-release-engineering`.
