# app-mediakit-knowledge

Motor wiki HTTP de patrón Wikipedia para `os-mediakit`. Sirve el repositorio
`content-wiki-documentation` como una wiki completamente navegable en
`documentation.pointsav.com`. Construido en Rust. Sin base de datos. Sin
dependencias en tiempo de ejecución más allá del binario compilado.

<div align="center">

[ 🇬🇧 Read this document in English ](./README.md)

</div>

Motor wiki para la plataforma de conocimiento de PointSav. Un solo
binario en Rust que sirve un directorio de archivos Markdown
(CommonMark con wikilinks) como una superficie de lectura y edición
de estilo Wikipedia.

## Estado

Las fases 1 a 5 (núcleo) se han desplegado y están operativas en
producción en `documentation.pointsav.com`. Consulte
[`ARCHITECTURE.md`](./ARCHITECTURE.md) para el plan completo de fases.

## Principio de diseño

**Los archivos Markdown en un árbol Git son la fuente de verdad.**
Cualquier base de datos, índice o caché es estado derivado,
reconstruible mediante `git checkout && reindex`. La identidad de
una página es una ruta; el historial de revisiones es `git log`;
los metadatos son frontmatter YAML; los espacios multi-contenido
son archivos hermanos. No hay escalera de migración de esquema
porque no hay esquema canónico en la base de datos — la base de
datos es un índice regenerable del árbol de archivos.

## Ejecutar

```
cargo run -- serve --content-dir <ruta-al-contenido>
```

El servidor enlaza `127.0.0.1:9090` por defecto. Se puede cambiar con
`--bind` o `WIKI_BIND`. Para compilar el binario de producción, ejecute
`cargo build --release` dentro de este directorio (no desde la raíz del
monorepo — el acoplamiento del workspace con `service-content` requiere
compilación local al crate).

## Fases de construcción

| Fase | Agrega | Estado |
|---|---|---|
| 1 | renderizado — GET /wiki/{slug}, /static/, /healthz | desplegado |
| 1.1 | interfaz Wikipedia — pestañas, TOC, hatnote, selector de idioma | desplegado |
| 2 | edición — CodeMirror 6, JSON-LD, guardado atómico, señalización SAA, autocompletado de citas | desplegado |
| 3 | búsqueda + feeds — Tantivy BM25, Atom, JSON Feed, sitemap, llms.txt | desplegado |
| 4 | sync Git + MCP — git2, historial/blame/diff, grafo redb, blake3, MCP JSON-RPC 2.0, OpenAPI 3.1 | desplegado |
| 5 núcleo | autenticación + revisión de ediciones — sesiones por cookie, argon2id, cola de revisión | desplegado |
| 5.1 | enrutamiento bilingüe /es/ — inicio y artículos en español, respaldo EN, redirección Accept-Language, hreflang | desplegado |
| 5.2+ | ACLs por página, SSO OIDC, suscripciones webhook, AsyncAPI 3.1 | planificado — condicionado a BP5 |
| 6 | resolución de wikilinks + identidad portátil | planificado |
| 7 | interfaz de federación (direccionamiento de contenido blake3 + ActivityPub) | planificado |
| 8 | modo de divulgación + sellado criptográfico de tiempo | planificado |

La Fase 8 está prevista como el foso del producto: la combinación
de autoría nativa en Markdown, extracción de datos estructurados
para los bloques de estado financiero requeridos por reguladores,
sellado de tiempo criptográfico, y adaptadores de exportación
por jurisdicción. Información prospectiva; sujeta a supuestos
materiales y decisiones del operador.

## Contexto del clúster

Este crate es parte del clúster `project-knowledge`
(según `~/Foundry/PROJECT-CLONES.md`), junto con:

- [`content-wiki-documentation`](../../../content-wiki-documentation/) —
  contenido TOPIC que el motor renderiza
- [`pointsav-fleet-deployment/media-knowledge-documentation/`](../../../pointsav-fleet-deployment/) —
  manuales de catálogo para el despliegue

## Convenciones

- Despliegue en binario único con systemd, sin Docker
  (según `conventions/zero-container-runtime.md`)
- READMEs bilingües (según `CLAUDE.md` §6 del workspace)
- Disciplina de citas (según `conventions/citation-substrate.md`)
- Postura de divulgación continua BCSC para el contenido
  (según `conventions/bcsc-disclosure-posture.md`)

## Licencia

Apache-2.0.
