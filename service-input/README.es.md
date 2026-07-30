# service-input

[ 🇬🇧 Read this document in English ](./README.md)

Servicio de ingesta de límites del Ring 1 para la entrada genérica
de documentos. Acepta archivos de formatos compatibles (PDF, DOCX,
XLSX, Markdown) en el límite de cada inquilino, los normaliza y
escribe la carga útil analizada a través de `service-fs` en el
Libro Mayor Inmutable WORM (Write-Once-Read-Many). Los consumidores
de Ring 2 (`service-extraction`, `service-content`,
`service-search`) leen del libro mayor; nunca tocan el documento
original.

## Posición en la arquitectura

- **Ring:** 1 (ingesta de límites, por inquilino) — ver
  `~/Foundry/conventions/three-ring-architecture.md`.
- **Escribe a:** `service-fs` (libro mayor WORM).
- **Leído por:** `service-extraction` (Ring 2) vía protocolo de
  cable MCP.
- **Multi-inquilino:** un proceso por `moduleId`.

## Reglas estrictas

- **ADR-07: cero IA en Ring 1.** El análisis es determinista; sin
  inferencia de LLM, sin modelos de incrustación, sin normalización
  asistida por IA.
- **WORM vía `service-fs`.** Este crate nunca persiste directamente
  a disco; cada escritura pasa por la interfaz MCP de `service-fs`
  para que la invariante de sólo-añadir se aplique en un único
  límite.

## Estado

Reserved-folder. Creado el 2026-04-25 por Task Claude en el
cluster `project-data`. Sin código aún — la siguiente sesión
activará el proyecto según `~/Foundry/CLAUDE.md` §9 y añadirá el
esqueleto inicial del despachador de analizadores.

## Licencia

Consulte el archivo `LICENSE` del repositorio. La asignación de
licencias a nivel de componente está regida por
`LICENSE-MATRIX.md` en
`pointsav/factory-release-engineering`.
