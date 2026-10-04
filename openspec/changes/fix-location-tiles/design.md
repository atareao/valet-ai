# Design

## Context

- CARTO devuelve placeholder «API KEY REQUIRED» (verificado: 2513 B idénticos en `@1x` y `@2x`, en vez
  de un PNG real).
- Alternativas verificadas con bytes reales: OSM estándar (6.9 KB), OSM France (13 KB), Esri Dark Gray
  (11 KB JPEG).
- El usuario prefiere **sin API key** y aspecto oscuro.

## Goals / Non-Goals

- **Goal:** que el mapa muestre cartografía real, sin key, con aspecto oscuro, sin cambiar el resto del
  widget.
- **Non-Goal:** proveedores con key (CARTO/Mapbox/Stadia); widgets de mapa futuros.

## Decisions

- **Proveedor: OpenStreetMap estándar.** `https://tile.openstreetmap.org/{z}/{x}/{y}.png`, sin
  subdominios ni `{r}`. Atribución obligatoria:
  `&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors`.
- **Aspecto oscuro por filtro CSS** sobre el panel de tiles, sin tocar los marcadores:
  ```css
  .valet-map-dark .leaflet-tile-pane {
    filter: invert(1) hue-rotate(180deg) brightness(0.95) contrast(0.9);
  }
  ```
  Se aplica solo a `.leaflet-tile-pane`; el marcador (overlay) y el popup quedan sin filtrar.
- **Se fija en un test** que `TILE_URL` apunta a `openstreetmap.org` y no a un proveedor con key.

## Risks / Trade-offs

- [Los rótulos de OSM se ven algo apagados con el filtro] → Aceptable por el usuario; es el precio de
  no depender de una key.
- [Política de uso de tiles de OSM] → Uso personal y de bajo volumen con atribución; conforme.
- [El filtro afecta solo a los tiles] → Verificado: `.leaflet-tile-pane` no incluye marcadores ni popup.

## Migration Plan

Sin migración. Solo frontend. `docker-compose.prod.yml` no se toca.
