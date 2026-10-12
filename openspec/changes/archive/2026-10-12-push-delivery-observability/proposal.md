# Proposal: Observabilidad del ciclo del ReminderPushWorker (contadores por suscripción y endpoints redactados)

## Why

El change anterior —`webpush-notifications`— entregó las notificaciones WebPush y se verificó en
**producción el 2026-10-12** con dos dispositivos suscritos (un Android y un Brave). El ciclo
registró:

```
reminder_push: ciclo completado delivered=1 skipped_old=0 pruned=0
```

**Esa línea no bastó para verificar la entrega.** Con dos suscripciones, `delivered=1` solo dice
que *al menos una* aceptó el envío: no distingue «las dos aceptaron» de «una aceptó y la otra
respondió un 5xx». Para saber qué había pasado en cada endpoint hubo que leer los `WARN` sueltos
del ciclo. El resumen cuenta recordatorios, no suscripciones, y por eso no responde a la pregunta
que de verdad se hacía.

Además, dos casos quedan hoy **sin resumen**:

- Un ciclo cuyo único suceso sea un `datetime` no parseable: `unparseable` se incrementa, pero el
  `if outcome.delivered > 0 || outcome.skipped_old > 0 || outcome.pruned > 0` lo excluye y no se
  registra nada.
- Un ciclo en el que **todos** los envíos fallan: no hay contador de fallos y la condición es
  falsa; solo quedan `WARN` sueltos. Ese es justo el caso que significa «la feature está rota en
  silencio».

Y los `WARN` por endpoint registran la **URL completa** de la suscripción
(`endpoint = %sub.endpoint`). Un endpoint WebPush es una **URL de capacidad**: quien la posee puede
enviar avisos a ese dispositivo —por eso el proyecto ya trata claves y tokens como material que no
va a los logs—. Volcarla entera es innecesario para diagnosticar y filtra un secreto.

## What Changes

- **`TickOutcome` SHALL contar por suscripción**, además de sus contadores actuales de
  recordatorios: cuántos envíos se intentaron (`attempted`), cuántos aceptó el push service
  (`accepted`) y cuántos fallaron (`failed`). Una suscripción depurada por `Gone` (404/410) SHALL
  contar como **intentada** y como **depurada**, **no** como fallida.
- **La decisión de registrar el resumen SHALL ser un predicado puro** sobre los contadores,
  `should_log_summary`, verdadero cuando **cualquiera** de ellos sea distinto de cero —incluidos
  `unparseable` y `failed`, hoy excluidos—.
- **Los registros del worker SHALL redactar el endpoint**: SHALL incluir solo el **host** del
  endpoint y el identificador de la suscripción, nunca la URL completa. La extracción del host SHALL
  ser un **ayudante puro**.

## Capabilities

### New Capabilities

_(ninguna)_

### Modified Capabilities

- `workers`: se añaden al `ReminderPushWorker` los contadores por suscripción, el predicado
  `should_log_summary` y la redacción del endpoint en los registros. Se especifica mediante
  `## ADDED Requirements` sobre la capability `workers` (no `MODIFIED`), de modo que los headers
  nuevos no colisionan con los ya existentes en `openspec/specs/workers/spec.md`.

## Impact

- **Backend:** `src/workers/reminder_push.rs` — los tres contadores nuevos de `TickOutcome`, el
  predicado `should_log_summary`, el ayudante puro de host y los tres `WARN` por endpoint
  redactados.
- **Spec:** `openspec/specs/workers/spec.md` (deltas `## ADDED Requirements` en este change).
- Sin cambios de esquema, sin endpoints nuevos, sin cambios de frontend.

## Non-goals

- **No se cambia la semántica de entrega.** Sigue siendo *at-least-once* y el recordatorio sigue
  marcándose en cuanto **al menos una** entrega sea aceptada. Un envío parcialmente fallido sigue
  consumiendo el aviso; corregir eso es una decisión aparte, no de este change.
- **No hay endpoints nuevos** ni se toca la API.
- **No se toca el frontend.**
- **No se añade un pipeline de métricas.** No se introduce Prometheus, ni un formato estructurado de
  ingesta, ni nada parecido: el proyecto no lo tiene y este change no lo estrena.
- **No se toca la ventana de gracia** (`push_reminder_grace_hours`) ni su lectura acotada.
- **No se añade un arnés de captura de `tracing` a los tests.** El repo no tiene `tracing-test` ni
  subscriber de test, y montarlo sería otro change. Aquí el contrato de aceptación se redacta sobre
  cosas verificables —los contadores de `TickOutcome`, el predicado y el ayudante—; el contenido
  exacto de la línea de log se documenta pero no se asevera en tests.
- **No se toca `docker-compose.prod.yml`.**
