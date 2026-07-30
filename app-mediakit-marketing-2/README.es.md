# app-mediakit-marketing-2

Motor de la plataforma de marketing — reescritura desde cero. Nombre temporal
`-2`; se renombrará a `app-mediakit-marketing` en el momento del reemplazo, una
vez alcanzada la paridad y con la aprobación del operador (fase P8 del plan).

## Por qué una reescritura

El motor retirado `app-mediakit-marketing` + `app-mediakit-shell` sigue
activo en home.woodfinegroup.com / home.pointsav.com y no se modifica en este
paquete. Sirve como referencia de contrato de solo lectura y catálogo de
antipatrones — no se reutiliza código, solo las dependencias de terceros y el
contrato externo (rutas, variables de entorno `SERVICE_MARKETING_*`, esquema
de manifiestos de contenido), de modo que el reemplazo eventual requiera un
cambio de configuración mínimo o nulo.

## Programa de fases

P0 andamiaje → P1 canal de contenido → P2 sistema de diseño → P3 páginas
principales → P4 SEO/descubrimiento → P5 MCP + cola de revisión → P6 suite de
pruebas → P7 paridad + despliegue sombra → P8 reemplazo + retiro del paquete
anterior (alcance de la Sesión de Comando).

## Ejecutar

```bash
cargo run -p app-mediakit-marketing-2 -- serve \
  --content-dir ../app-mediakit-marketing/content \
  --module-id woodfine \
  --bind 127.0.0.1:9202
```

`cargo run -p app-mediakit-marketing-2 -- check --content-dir <dir>` valida la
configuración/contenido sin servir.

## Estado

P0 — andamiaje. Compila sin errores; `/healthz` + `/static/*path` servidos;
el contrato de CLI/entorno coincide con el motor retirado.
