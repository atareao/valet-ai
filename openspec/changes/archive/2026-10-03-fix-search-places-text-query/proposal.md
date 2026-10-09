# Change: `search_places` — usar searchText para consultas en texto libre

## Why

`search_places` incumple su propia spec (`tools/geo`). El método
`SearchPlacesTool::execute` llama **siempre** a `search_nearby` y pasa el argumento `query` como
`includedTypes`:

```rust
let types = if query.is_empty() { vec![] } else { vec![query] };
let places = client.search_nearby(lat, lon, radius as u32, &types).await?;
```

Una consulta real como `"restaurantes en Madrid"` viaja como *tipo* de lugar. Google no reconoce ese
tipo y devuelve vacío o error, así que la tool falla en silencio para el caso de uso más natural
(búsqueda por texto). El cliente ya tiene implementado `search_text` (`textQuery` + `languageCode`)
pero **nadie lo llama**. Es una discrepancia spec↔código, no una mejora.

## What Changes

- **Despacho por argumentos.** Si la llamada incluye `radius`, usar
  `POST places:searchNearby` con `locationRestriction.circle`. Si no lo incluye, usar
  `POST places:searchText` con `textQuery` y `languageCode: "es"`.
- **La query de texto libre deja de enviarse como `includedTypes`.** En la ruta texto se envía como
  `textQuery`; en la ruta cercanía, `query` actúa como tipo incluido.
- **Respuesta unificada.** Ambas rutas devuelven `ToolResult.data` con `{ places, maps_link }`.

## Capabilities

### Modified Capabilities
- `tools/geo`: el requirement `Google Places API integration` pasa a exigir el despacho correcto
  (`searchText` sin `radius`, `searchNearby` con `radius`) y los escenarios ya descritos en la spec.

## Impact

- Backend: `src/tools/google_places.rs` (`SearchPlacesTool::execute`).
- Tests: unitarios del tool (despacho a `searchText` vs `searchNearby`).
- Contrato LLM sin cambios: mismos parámetros de entrada (`query`, `latitude`, `longitude`, `radius`).
- Sin migración de datos ni cambios de esquema. `docker-compose.prod.yml` no se toca.

### Fuera de alcance

- H6 (timeout HTTP de `weather`): change aparte.
