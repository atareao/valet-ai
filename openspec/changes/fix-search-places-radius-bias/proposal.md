# Proposal

## Why

`search_places` falla con `400 Unsupported types: <query>` cuando la llamada incluye `radius` y la
consulta es un nombre de negocio. En la ruta con `radius`, el texto libre del usuario se envía como
`includedTypes`, y la Google Places API (New) exige que `includedTypes` sea un tipo de Place válido
(`restaurant`, `cafe`, `museum`…). Cualquier consulta que no sea un tipo exacto —lo habitual— provoca
un `INVALID_ARGUMENT`.

Evidencia en producción (2026-10-03T19:54:36Z):

```
tool_args={"latitude":39.4676,"longitude":-0.3771,"query":"Querida Jacinta restaurante","radius":5000}
error=Execution error: Google Places API returned 400:
{"error":{"code":400,"message":"Unsupported types: Querida Jacinta restaurante.","status":"INVALID_ARGUMENT"}}
```

El change `fix-search-places-text-query` (H5) arregló la ruta **sin** `radius` (`places:searchText`
con `textQuery`), pero dejó la ruta **con** `radius` enviando la consulta como tipo. Su test usaba
`query: "cafe"`, que sí es un tipo válido, así que el fallo pasó inadvertido.

## What Changes

- **Un único endpoint: `places:searchText`.** `search_places` envía la consulta como `textQuery`
  siempre, con o sin `radius`.
- **El radio pasa a ser un sesgo.** Cuando la llamada incluye `radius`, el body añade
  `locationBias.circle` con `center` (lat/lon) y `radius` en metros. La doc de Google admite círculo
  en `locationBias` (0–50000 m) y solo rectángulo en `locationRestriction`.
- **Desaparece `searchNearby`.** Se elimina la ruta y el método `search_nearby`, que solo existía para
  mandar la consulta como tipo.
- **Nunca se envía texto libre como tipo.** La tool no envía `includedTypes` en ningún caso.

## Capabilities

### Modified Capabilities

- `tools/geo`: el requisito «Google Places API integration» deja de alternar endpoint y pasa a usar
  siempre `searchText`, con `locationBias.circle` cuando hay `radius`.

## Impact

- Código: `src/tools/google_places.rs` (`SearchPlacesTool::execute`; retirada de `search_nearby`;
  `search_text` acepta el sesgo circular). El esquema de `parameters()` no cambia.
- Sin cambios de frontend, base de datos, compose ni despliegue.
- Tests: `src/tools/google_places.rs` (wiremock) — nombre + `radius` → `places:searchText` con
  `locationBias.circle`; sin `radius` → `places:searchText` sin bias; ninguno con `includedTypes`.

### Fuera de alcance

- `locationRestriction` (rectángulo) para restricción dura: no se implementa; el radio es sesgo.
- Migrar `maxResultCount` a `pageSize`: cambio aparte si procede.
