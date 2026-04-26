# vendor-sel4-fs

[ 🇬🇧 Read this document in English ](./README.md)

Andamiaje Reserved-folder para un futuro servicio de sistema de
archivos bare-metal basado en seL4. Aloja el andamiaje
`#![no_std] #![no_main]` reubicado desde `service-fs` el
2026-04-26 (mensaje de buzón de salida del cluster
`ring1-scaffold-runtime-model-drift`; ratificado por la
Decisión 2 del Master Claude la misma fecha).

## Por qué existe este directorio

`service-fs` es un servicio de ingesta de límites Ring 1 en la
arquitectura de tres anillos (ver
`~/Foundry/conventions/three-ring-architecture.md`) y se ejecuta
como un proceso MCP-server alojado bajo systemd
(`~/Foundry/conventions/zero-container-runtime.md`). El
andamiaje anterior en `service-fs/src/main.rs` era en cambio
`#![no_std] #![no_main]` con un punto de entrada `_start`
hecho a mano y un bucle de panic — un encuadre de unikernel
seL4 bare-metal que contradecía ambas convenciones ratificadas.

El linaje seL4 ya tiene su propio hogar en el registro:

- `vendor-sel4-kernel` (1074 archivos; código fuente del kernel
  seL4 vendido)
- `moonshot-sel4-vmm` (4 archivos; monitor de máquina virtual
  seL4)
- `system-substrate-broadcom`, `-freebsd`, `-wifi` (puentes de
  hardware)

`vendor-sel4-fs` se une a ese linaje como el hogar natural para
un eventual servicio de sistema de archivos basado en seL4. Hoy
es un **marcador de posición Reserved-folder** — el andamiaje de
26 líneas reubicado más este par de READMEs.

## Estado

Reserved-folder. Creado el 2026-04-26 por Task Claude en el
cluster `project-data` como destino de reubicación para el
andamiaje seL4 previamente en `service-fs/`. Aún no activado,
aún no en miembros del workspace, aún no conectado a ninguna
compilación.

## Reglas estrictas (cuando este proyecto se active)

- **Bare-metal puro.** Cero dependencias del sistema operativo
  permitidas. Cuando el alcance se expanda, las adiciones son
  solo compatibles con bare-metal.
- **Fuera de alcance: trabajo del proceso alojado Ring 1.** La
  ingesta de límites del MCP-server alojado Ring 1 es trabajo
  de `service-fs`, no de `vendor-sel4-fs`. Los dos tienen
  modelos de tiempo de ejecución diferentes.

## Licencia

Consulte el archivo `LICENSE` del repositorio. La asignación de
licencias a nivel de componente está regida por
`LICENSE-MATRIX.md` en
`pointsav/factory-release-engineering`.
