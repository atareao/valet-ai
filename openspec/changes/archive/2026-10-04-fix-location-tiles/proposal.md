# Proposal

## Why

`LocationWidget` usa tiles de CARTO (`{s}.basemaps.cartocdn.com/dark_all/...`), pero CARTO **ahora
exige API key**: sirve una imagen-placeholder «API KEY REQUIRED» (2.5 KB, idéntica en `@1x` y `@2x`) en
lugar de cartografía. Resultado en la UI: el mapa monta pero sale en blanco. Es un fallo en producción,
no de la tubería de widgets.

## What Changes

- **`frontend` (MODIFIED)**: el mapa de `LocationWidget` pasa a usar **tiles de OpenStreetMap**
  (`https://tile.openstreetmap.org/{z}/{x}/{y}.png`, sin API key) con su atribución, y un **filtro CSS**
  sobre el panel de tiles para el aspecto oscuro. El resto del widget no cambia.

**Fuera de alcance:** los widgets de mapa aún no implementados (Route/PlacesExplore/WeatherRadar); sus
proveedores se decidirán al diseñarlos (se documenta OSM+CSS como patrón por defecto).

## Capabilities

### Modified Capabilities

- `frontend` (se modifica el requisito del widget `LocationWidget` para fijar el proveedor de tiles).

## Impact

- **Frontend**: `LocationWidget.tsx` (constantes `TILE_URL`/`TILE_ATTRIBUTION`), un CSS propio para el
  filtro oscuro, y un test que fija el proveedor (evita volver a un proveedor con key).
- **No** toca backend ni `docker-compose.prod.yml`.
