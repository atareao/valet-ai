# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

Comandos de verificación de este repo: `cargo test`, `cargo test --lib timeline`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `openspec validate --all`.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + delta de `workers` + design + tasks) y **esperar aprobación explícita** antes de tocar código.
- [x] 0.2 Resolver la **decisión abierta** del `design.md`: tolerar un `[]` suelto como cero hechos (statu quo) o tratarlo como **fallo de forma** y reintentar. Recomendada: la segunda.

## 1. RED — forma de la petición (defecto A)

- [x] 1.1 RED — test unitario en `src/workers/episodic_memory.rs` (`#[cfg(test)]`, junto a los de consolidación): la petición de consolidación contiene un mensaje de rol `user` **no vacío** además del de rol `system`; hoy solo lleva el `system`. **Pegar la salida en rojo en el informe.**
- [x] 1.2 RED — test unitario: el mensaje de rol `system` sigue llevando **literalmente** el `consolidator_prompt` configurable (el turno de usuario no altera el prompt editable de la UI).

## 2. RED — prompt sembrado (defecto B)

- [x] 2.1 RED — test de migración: tras aplicar las migraciones, un `settings.timeline_prompt` **sin editar** contiene el envoltorio `{"events": []}` para el lote sin hechos y **no** la frase de la lista vacía.
- [x] 2.2 RED — test de migración: un `settings.timeline_prompt` **editado por el usuario** (valor distinto del sembrado) **no** se pisa.
- [x] 2.3 RED — test unitario: el respaldo del código `DEFAULT_TIMELINE_PROMPT_FALLBACK` pide el envoltorio `{"events": []}` y no una lista suelta (ya coherente; se añade como red de seguridad del contrato).

## 3. RED — stats del consolidador (defecto C)

- [x] 3.1 RED — test unitario: una consolidación cuyo contenido no valida (vacío, sin objeto JSON o con esquema inválido) deja la fila de `llm_requests` en `status = 'error'` con el motivo en `error_message` (hoy: `status = 'success'`).
- [x] 3.2 RED — test unitario: una consolidación cuyo contenido **sí** valida deja la fila en `status = 'success'`.

## 4. GREEN — implementación mínima

- [x] 4.1 GREEN — `src/workers/episodic_memory.rs`: añadir a `call_semantic_chat` un turno de rol `user` fijo y no vacío, conservando el prompt como `system` (Forma 1 del `design.md`). `cargo test` y `cargo test --lib timeline` en verde.
- [x] 4.2 GREEN — escribir la fila de `llm_requests` del consolidador después de validar, con el `status` y el `error_message` finales.
- [x] 4.3 GREEN — migración nueva (p. ej. `migrations/20261012000001_timeline_empty_events_envelope.sql`) que sustituye la frase de la lista vacía por el envoltorio `{"events": []}`, **solo si** `value` sigue siendo el texto anterior exacto, siguiendo el idioma de `migrations/20261011000001_timeline_fact_integrity.sql` (bloque 4a), e insertándola si la clave falta (`INSERT ... WHERE NOT EXISTS`).
- [x] 4.4 GREEN — **decisión del 0.2 = tratar `[]` como fallo de forma** (Alternativa 2): retirar la tolerancia del array vacío en el parser del extractor y dejar que el reintento acotado (ya existente) la recupere; el `[]` suelto pasa a agotar los reintentos y, agotados, la pasada sigue sin hechos con `status = 'error'` (`kind = 'timeline'`), mientras que el envoltorio `{"events": []}` sigue siendo cero hechos con `status = 'success'`. Esto **incluye actualizar los tests preexistentes** que hoy afirman que `[]` es cero hechos, añadidos por el change archivado `timeline-extractor-resilience` —en particular `test_parse_timeline_events_bare_empty_array_is_zero_events` en `src/workers/episodic_memory.rs`—, que dejan de ser válidos: deben reescribirse para fijar el nuevo contrato (array suelto = fallo de forma; envoltorio vacío = cero hechos).

## 5. REFACTOR

- [x] 5.1 REFACTOR — limpiar artefactos en cascada (variantes de error, constantes o tests que queden sin uso al cambiar el registro de stats y la tolerancia de `[]`).
- [x] 5.2 REFACTOR — `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings nuevos.
- [x] 5.3 REFACTOR — revisar y actualizar los tests del extractor preexistentes que codifican el statu quo del `[]` (p. ej. `test_parse_timeline_events_bare_empty_array_is_zero_events` en `src/workers/episodic_memory.rs`, del change archivado `timeline-extractor-resilience`) para que afirmen el contrato nuevo: `[]` suelto = fallo de forma que agota reintentos; `{"events": []}` = cero hechos con `status = 'success'`. Ningún test debe seguir afirmando que `[]` es cero hechos.

## 6. Verificación y cierre

- [x] 6.1 `cargo test` (suite completa) en verde.
- [x] 6.2 `cargo test --lib timeline` en verde.
- [x] 6.3 `cargo clippy --all-targets -- -D warnings` en verde.
- [x] 6.4 `cargo fmt --check` en verde.
- [x] 6.5 `openspec validate episodic-call-integrity` y `openspec validate --all` en verde.
- [ ] 6.6 Medir en producción tras desplegar: `SELECT status, error_message, completion_tokens FROM llm_requests WHERE kind='consolidator'` (esperado: fuera de los `completion_tokens = 1` con `status='success'`) y `SELECT status, error_message FROM llm_requests WHERE kind='timeline'` (esperado: menos `NotJson`). Dejar constancia del antes y el después.
- [x] 6.7 Parar y esperar aprobación del usuario. Aprobar el archive (`openspec archive episodic-call-integrity`) en un PR aparte tras el merge.
