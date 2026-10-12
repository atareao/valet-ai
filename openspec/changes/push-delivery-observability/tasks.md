# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + specs + design + tasks) y **esperar aprobación explícita** antes de tocar código. Es el punto de parada obligatorio del protocolo OpenSpec: sin un «aprobado» del usuario, no se avanza a la tarea 1.1.

## 1. Contadores por suscripción en `TickOutcome`

- [x] 1.1 RED — test que falla de los contadores por suscripción, añadido al módulo de tests de `src/workers/reminder_push.rs`: con un recordatorio vencido y 3 suscripciones que devuelven `Delivered`, `Gone` y `Failed`, `outcome.attempted == 3`, `outcome.accepted == 1`, `outcome.pruned == 1`, `outcome.failed == 1` y `outcome.delivered == 1`; con `Failed` en las dos suscripciones, `failed == 2`, `accepted == 0` y `delivered == 0`, sin marca; sin suscripciones, `attempted == 0`. Verificar en rojo con `cargo test --lib reminder_push`.
- [x] 1.2 GREEN — añadir los campos `attempted`, `accepted` y `failed` a `TickOutcome` (que ya deriva `Debug, Clone, Default, PartialEq, Eq`) e incrementarlos en el bucle de envío: `attempted` en cada envío intentado (incluido `Gone`), `accepted` en `Delivered`, `failed` en `Failed`; `Gone` suma a `attempted` y a `pruned`, y nunca a `failed`. Confirmar `cargo test --lib reminder_push` y `cargo check` en verde.

## 2. Predicado `should_log_summary` y su uso en el worker

- [x] 2.1 RED — test que falla del predicado: un `TickOutcome` con todos los contadores a cero devuelve `false`; con `unparseable == 1` devuelve `true`; con `failed == 1` devuelve `true`; con `delivered == 1` devuelve `true`. Verificar en rojo con `cargo test --lib reminder_push`.
- [x] 2.2 GREEN — implementar `TickOutcome::should_log_summary(&self) -> bool` como predicado puro (verdadero cuando cualquiera de los contadores difiere de cero) y sustituir en `tick()` el `if outcome.delivered > 0 || outcome.skipped_old > 0 || outcome.pruned > 0` por `if outcome.should_log_summary()`. Añadir también los contadores nuevos a la línea `reminder_push: ciclo completado`. Confirmar `cargo test --lib reminder_push` y `cargo check` en verde.

## 3. Ayudante de host y redacción de los `WARN` por endpoint

- [x] 3.1 RED — test que falla del ayudante puro de host: para `https://fcm.googleapis.com/fcm/send/abc` devuelve `fcm.googleapis.com`; para una cadena vacía o un valor sin host reconocible devuelve el marcador `(endpoint no parseable)` y **no** la cadena original. Verificar en rojo con `cargo test --lib reminder_push`.
- [x] 3.2 GREEN — implementar el ayudante puro de host y aplicarlo a los tres `WARN` por endpoint del bucle (`suscripción caducada`, `fallo de envío` y `error al enviar`), de modo que registren `endpoint_host = %endpoint_host(&sub.endpoint)` más el identificador de la suscripción, y **no** `endpoint = %sub.endpoint`. Confirmar `cargo test --lib reminder_push` y `cargo check` en verde.

## 4. Verificación

- [x] 4.1 `cargo test --lib reminder_push` en verde, con el recuento de tests anotado antes y después.
- [ ] 4.2 Suite de Rust completa en verde con `cargo test --no-fail-fast` (0 fallos).
- [x] 4.3 `cargo clippy --all-targets -- -D warnings` limpio (0 warnings).
- [x] 4.4 `cargo fmt --check` limpio.

## Notas

- Este change **no** cambia la semántica de entrega: sigue siendo *at-least-once* y el recordatorio se sigue marcando en cuanto al menos una entrega sea aceptada. Un fallo parcial sigue consumiendo el aviso; arreglarlo es otro change.
- El contenido exacto de la línea de log **no** se asevera en tests: el repo no tiene arnés de captura de `tracing`. El contrato de aceptación se apoya en los contadores de `TickOutcome`, el predicado `should_log_summary` y el ayudante de host, que sí son verificables sin capturar logs.
- Sin cambios de esquema, sin endpoints nuevos, sin cambios de frontend, sin tocar `docker-compose.prod.yml`.
