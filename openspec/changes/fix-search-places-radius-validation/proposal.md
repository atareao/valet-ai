# Proposal

## Why

La tool `search_places` acepta `radius` sin validarlo. Google admite 0–50000 m: un radio mayor
vuelve a provocar un 400 (la misma clase de fallo recién arreglado). Además, un `radius` decimal o no
numérico cae en silencio a 1000 m (`as_u64().unwrap_or(1000) as u32`), aplicando un sesgo de 1 km que
el usuario no pidió. Por último, `search_text` conserva un parámetro `included_type` que nadie usa y
que la spec prohíbe (`includedType`), un pie para reintroducir el bug.

## What Changes

- **Parseo robusto de `radius`**: si es numérico (entero o decimal) y > 0 → se redondea al entero y
  se acota a `[1, 50000]`; si es 0, negativo o no numérico → se ignora (sin `locationBias`).
- **Eliminar `included_type`** de `search_text` (y su rama `includedType`), que solo se invocaba con
  `None`.
- **Tests**: radio por encima del máximo, decimal, no numérico y 0/negativo.

## Capabilities

### Modified Capabilities

- `tools/geo`: se añade el requisito de validación del `radius`.

## Impact

- Backend: `src/tools/google_places.rs` (parseo en `execute`, firma/branch de `search_text`, tests).
- Sin cambios de frontend ni de otros tools.
- Fuera de alcance: exponer un parámetro de tipo (`includedType`) o `locationRestriction`.
