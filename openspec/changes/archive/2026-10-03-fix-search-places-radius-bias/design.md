# Design

## Context

`SearchPlacesTool::execute` (`src/tools/google_places.rs`) despacha por la presencia de `radius`:
con `radius` llama a `search_nearby(lat, lon, radius, &[query])`, que envía `includedTypes: [query]`.
Google rechaza cualquier valor de `includedTypes` que no sea un tipo de Place válido. Ver
`proposal.md` — Why.

## Goals / Non-Goals

- **Goal:** que cualquier consulta (nombre de negocio o tipo) funcione con o sin `radius`.
- **Non-Goal:** restricción geográfica dura. El radio es un sesgo (`locationBias`), no un filtro.

## Decisions

- **Un solo endpoint, `places:searchText`.** `textQuery` es el parámetro correcto para texto libre
  (nombres y tipos). Alternativa descartada: mantener `searchNearby` con lista blanca de tipos; la
  lista hay que mantenerla y el comportamiento sigue fallando para nombres.
- **`radius` → `locationBias.circle`.** La doc oficial de Text Search (New) admite círculo en
  `locationBias` (centro + radio 0–50000 m); `locationRestriction` solo admite rectángulo.
  Alternativa descartada: `locationRestriction` con un rectángulo equivalente — cambia la forma del
  área y la semántica (restricción dura).
- **Retirar `search_nearby`.** Sin la ruta con radio, el método queda sin uso; mantenerlo sería código
  muerto con un contrato engañoso.

## Risks / Trade-offs

- [El radio deja de ser una restricción dura: pueden aparecer resultados algo fuera del área] →
  Aceptado y coherente con la semántica de `locationBias`; se documenta en la descripción del
  parámetro. Si se quisiera dureza, sería `locationRestriction` (rectángulo), fuera de alcance.
- [`locationBias` se ignora si el `textQuery` menciona una localidad explícita] → Comportamiento de
  la propia API; el sesgo no se pierde en las consultas típicas (nombre de negocio).

## Migration Plan

Sin migración de datos. El cambio es de comportamiento en una tool. Despliegue normal: merge a
`development` y reconstrucción/despliegue de la imagen. Rollback: revertir el commit.
