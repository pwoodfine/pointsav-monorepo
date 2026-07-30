# app-mediakit-foodservice

El motor de la plataforma de servicio de alimentos para `foodservice.woodfinegroup.com`.

[ 🇬🇧 Read this document in English ](./README.md)

> **Estado:** Activo (andamiaje P1, 2026-06-24).
> Registro del proyecto: `pointsav-monorepo/.agent/rules/project-registry.md`.
> Archivo: `clones/project-foodservice`.

## Descripción

Binario único en Rust (axum 0.8) que renderiza páginas **en el servidor** a
partir de manifiestos de secciones tipadas. Sigue el patrón
`app-mediakit-marketing`: chasis compartido desde `app-mediakit-shell`,
autoría MCP orientada a agentes, puerta de aprobación humana F12
(SYS-ADR-10, SYS-ADR-19).

- **Modelo de contenido:** `content/<slug>/page.yaml` — secciones tipadas ordenadas
- **Chasis:** `app-mediakit-shell` (encabezado, pie de página, todo el CSS)
- **Puerto:** `127.0.0.1:9103` (predeterminado)
- **Prefijo de variables de entorno:** `SERVICE_FOODSERVICE_*`

## Ejecutar

```
cargo run -p app-mediakit-foodservice -- serve \
  --content-dir app-mediakit-foodservice/content \
  --state-dir /tmp/foodservice-state \
  --module-id woodfine \
  --bind 127.0.0.1:9103 \
  --enable-mcp
```

## Compilar y probar

```
cargo check -p app-mediakit-foodservice
cargo test -p app-mediakit-foodservice
cargo clippy -p app-mediakit-foodservice -- -D warnings
```

## Estado

Andamiaje P1: estructura del paquete, páginas de contenido iniciales
(inicio, contacto, aviso legal). El sitio activo `foodservice.woodfinegroup.com`
no estaba disponible al momento del andamiaje (2026-06-24) — no se migró
contenido. La autoría completa del contenido es una fase posterior.
