# app-mediakit-knowledge

Reescritura desde cero del motor wiki de conocimiento de PointSav.

Un wiki HTTP de un solo binario sobre un árbol de archivos Markdown. Estructurado
según Wikipedia Vector 2022 — franja de aviso del sitio, cabecera blanca fija,
pestañas de acción del artículo sobre el título, dos columnas (barra lateral +
contenido) y pie institucional — con un sistema visual de marca como acento por
instancia. Tres instancias: documentation (PointSav), projects y corporate
(Woodfine).

Los archivos Markdown en un árbol Git son la fuente de verdad. El índice de
búsqueda, el grafo de enlaces y el historial son estado derivado y regenerable.

## Compilación

```
cargo build -p app-mediakit-knowledge
```

## Ejecución

```
app-mediakit-knowledge serve --knowledge-toml /etc/local-knowledge/documentation.toml
```

## Estado

En reescritura activa. Véase el plan `virtual-twirling-parasol` y
`.agent/briefs/BRIEF-knowledge-ng-rewrite.md` para el programa por fases.
