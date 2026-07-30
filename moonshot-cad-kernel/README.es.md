# moonshot-cad-kernel

Base soberana del núcleo CAD para la herramienta CAD/BIM del espacio de trabajo
(`BRIEF-workplace-bim-cad-tool`). Fase 0: primitivas geométricas 2D + el **modelo de
documento basado en registro de operaciones** (estilo git) que es la joya de la
herramienta: cada cambio es una operación tipada, el estado del dibujo es una proyección
reconstruible del registro, y deshacer/rehacer + un historial comparable surgen de forma
natural.

## Contenido (Fase 0)

- **Geometría:** `Point`, `Bounds` y `Entity` (Line, Circle, Arc, Polyline) con
  `length()` y `bounds()`.
- **Modelo de documento:** `Document` (capas + entidades colocadas) se reconstruye de
  forma determinista reproduciendo un registro de operaciones `Op`.
- **`Drawing`:** el registro + estado actual + cursor de deshacer/rehacer; `apply`,
  `undo`, `redo`, constructores (`add_layer`, `add_entity`) y guardado/carga en
  **JSON-Lines** (`to_jsonl` / `from_jsonl`) — el formato canónico, comparable y compatible
  con git.

## Pendiente (fases posteriores, según el BRIEF)

- Renderizado 2D/3D con `wgpu` (una sola tubería, sin ruta de render en JS).
- Un solucionador de restricciones 2D estilo ISOtope.
- Bifurcar `truck` para 3D B-rep (boceto→extrusión).
- Vinculación de tokens/tipos/ocurrencias BIM vía `moonshot-bim-engine` (dos capas, un núcleo).

## Notas de diseño

- Soberano, sin conexión, listo para WASM. Dependencias: solo `serde` + `serde_json`
  (Apache/MIT).
- El registro de operaciones **es** el documento — esto hace que el historial de diseño
  estilo git y (más adelante) la colaboración con `moonshot-crdt` sean nativos.
- Rust determinista; ninguna inferencia de IA toca el modelo (SYS-ADR-07).
