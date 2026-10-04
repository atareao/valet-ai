# Tasks

## 1. Frontend — RED

- [x] 1.1 Exportar `TILE_URL` y `TILE_ATTRIBUTION` desde `LocationWidget.tsx` y añadir un test que exija `TILE_URL` con `openstreetmap.org` (y sin `cartocdn`) y la atribución con `OpenStreetMap`.
- [x] 1.2 `npm test` → el test falla y el resto sigue verde.

## 2. Frontend — GREEN

- [x] 2.1 Cambiar `TILE_URL`/`TILE_ATTRIBUTION` a OSM y añadir `LocationWidget.css` con el filtro oscuro aplicado vía `className="valet-map-dark"` en el contenedor del mapa.
- [x] 2.2 `npm test` verde; `npm run typecheck` y `npm run lint`.

## 3. Revisión y cierre

- [x] 3.1 Revisión `react-reviewer`.
- [ ] 3.2 PR a `development`.
- [ ] 3.3 Prueba real: redesplegar y comprobar en la UI que el mapa muestra cartografía oscura.
- [ ] 3.4 `openspec archive fix-location-tiles` y PR de archivado.
