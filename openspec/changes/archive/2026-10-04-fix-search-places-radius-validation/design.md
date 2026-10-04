# Design

## Context

`SearchPlacesTool::execute` (`src/tools/google_places.rs:347-349`) construye el sesgo así:

```rust
let location_bias = args
    .get("radius")
    .map(|radius_value| (lat, lon, radius_value.as_u64().unwrap_or(1000) as u32));
```

`serde_json::Value::as_u64()` devuelve `None` para decimales, cadenas, negativos o `null`; en esos
casos se aplica 1000. No hay cota superior, así que `radius > 50000` llega tal cual a Google y
devuelve 400. `search_text` (`:140`) acepta `included_type`, pero `execute` siempre pasa `None`.

## Goals / Non-Goals

- **Goal:** que ningún `radius` razonable provoque un 400 y que un valor inválido no imponga un sesgo
  arbitrario.
- **Non-Goal:** cambiar el comportamiento sin `radius` (ya es correcto) ni añadir `includedType`.

## Decisions

- **Acotar (clamp) en vez de rechazar.** El sesgo por radio es best-effort: acotar a 50000 mantiene
  viva una consulta válida, mientras que devolver `ToolError::InvalidArguments` la abortaría por un
  parámetro secundario. El radio es sesgo, no restricción dura.
- **Ignorar el radio inválido en vez de asumir 1000.** Si el valor no es utilizable, es preferible
  buscar sin sesgo (comportamiento ya especificado para «sin `radius`») que imponer 1 km inventado.
- **Redondear al entero más cercano.** `as_f64().round()`: 2500.4 → 2500, 2500.6 → 2501. Google espera
  un radio entero; el redondeo no sesga hacia abajo.
- **Eliminar `included_type`.** La tool no expone tipos de lugar y la spec prohíbe `includedType`;
  mantener el parámetro solo invita a reintroducir el 400. Si algún día se necesita, se especifica
  aparte con validación de tipos.

## Risks / Trade-offs

- [Cambio observable: un `radius` decimal pasa de 1000 al valor redondeado] → Es la corrección; queda
  cubierto por un escenario y un test.
- [Un `radius` basura ya no aplica 1000] → Deliberado: sin sesgo es el comportamiento por defecto.

## Migration Plan

Sin migración ni estado persistente.
