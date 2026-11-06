# PLAN-020 — El panel `Stats → Sistema → Tool Usage` deja de salir siempre «No data yet»

- **Estado:** ✅ Completado, archivado y desplegado como **v0.31.1** el 2026-11-06.
- **Fecha:** 2026-11-06
- **Change de OpenSpec:** `tool-usage-stats`, archivado en
  `openspec/changes/archive/2026-11-06-tool-usage-stats/`.
- **Release:** `v0.31.1` desde `main`.

---

## 1. Motivo

El panel **Stats → Sistema → Tool Usage** del frontend mostraba siempre «No data yet», sin importar
cuántas herramientas hubiese invocado el agente. El gráfico no tenía origen de datos: nunca llegaba una
fila con `tool_calls` poblado.

## 2. Diagnóstico (evidencia)

Medido contra la base de datos real de producción el 2026-11-06, sobre la **v0.31.0**:

- `llm_requests` tenía **1166 filas** con `kind='chat'` y **0** con `tool_calls` poblado.
- `StatsRepo::tools_summary()` lee exactamente `llm_requests.tool_calls` filtrando `kind='chat'`, y es
  el **único** origen del gráfico.
- La ruta de chat (`process_message_stream`) llamaba a
  `StatsRepo::record_request(..., tool_calls: None, ...)` aunque el evento `Done` ya traía los
  `tool_calls` en scope. La columna nunca se escribía.

**Contrato roto:** el lector (`db/repos`) era correcto y cumplía su spec
(`StatsRepo SHALL provide tool call frequency`); el escritor (`orchestrator/agent`) no la alimentaba
(`Orchestrator records stats after each streamed LLM call`). No era un bug del frontend ni de la
consulta: era un desajuste entre escritor y lector.

## 3. Objetivo y alcance

**Objetivo:** que cada llamada de chat del bucle ReAct persista en `llm_requests.tool_calls` los
nombres de las herramientas solicitadas por el modelo, de modo que `tools_summary()` refleje el uso
real y la tarjeta del frontend se pueble sola.

**Alcance:** `src/orchestrator/agent.rs` (ruta de streaming del bucle ReAct).

**Fuera de alcance:** `StatsRepo::tools_summary()` (el lector ya era correcto), el frontend (la tarjeta
se puebla cuando llegan datos), la API y la base de datos (la columna `tool_calls` ya existía, sin
migración).

## 4. Solución

- El orquestador graba en `llm_requests.tool_calls` un JSON con la forma `[{"name":"<tool>"}, …]` con
  los nombres de las herramientas solicitadas en cada respuesta.
- Se aplica la **misma precedencia que ya regía la ejecución**: los `tool_calls` del evento `Done` y,
  **solo** si el `Done` no los trae, los acumulados del stream (`Done` > stream).
- Cuando la respuesta no solicita ninguna herramienta, la fila se graba con `tool_calls = NULL` (no
  ensucia el recuento).

## 5. Ciclo TDD

1. **RED:** tests que fallan — (1.1) tras un turno de streaming cuyo `Done` trae `tool_calls` existe una
   fila `llm_requests` con `kind='chat'` y `tool_calls` con los nombres; (1.2) precedencia `Done` >
   stream; (1.3) una respuesta sin `tool_calls` graba `tool_calls = NULL`; (1.4)
   `StatsRepo::tools_summary()` devuelve el recuento esperado sobre las filas escritas por el
   orquestador.
2. **GREEN:** implementación mínima — serializar los nombres y pasarlos a `StatsRepo::record_request`
   en `process_message_stream`, con la precedencia `Done` > stream.
3. **REFACTOR:** `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` en verde —
   **1604 tests**.

## 6. Auditoría

Auditoría `@rust-reviewer` sobre el diff: **sin bloqueantes** (0 bloqueantes, 0 mayores).

## 7. Release y verificación en producción

- **PR #323** ✅ — mergeado a `development` con CI verde. Hash de merge: `9765256`.
- **Release v0.31.1** — **PR #324** a `main` (merge `3d529ac`), tag `v0.31.1` e imagen publicada por el
  workflow `image.yml`.
- **Despliegue `co1`** ✅ — stack recreado con la nueva imagen; `GET /api/health` →
  `{"version":"0.31.1"}`.

**Verificación E2E en producción (2026-11-06):**

- `SELECT COUNT(*) FROM llm_requests WHERE kind='chat' AND tool_calls IS NOT NULL` → **1**.
- Fila: `created_at=2026-11-06 20:36:43`, `tool_calls=[{"name":"email_list_unread"}]`.
- Agregación equivalente a `tools_summary` → `email_list_unread: 1`.

El panel **Stats → Sistema → Tool Usage** deja de salir vacío.

## 8. Decisiones cerradas

- **Sin backfill del histórico** (decisión del usuario). Los turnos previos al despliegue quedan sin
  datos de herramientas y el gráfico se puebla a partir de la v0.31.1. Se descartó rellenarlos desde
  `messages.tools_used` porque ese campo es un **agregado por turno** en formato `nombre, (2) nombre`,
  **no por llamada al LLM**, de modo que reconstruir `llm_requests.tool_calls` desde él
  **distorsionaría el recuento** (un turno con varias llamadas quedaría contado una sola vez, y las
  llamadas sin herramientas también aparecerían).

## 9. Observación fuera de alcance (pendiente, sin tarea)

La **ruta de reflexión** graba `kind='chat'` con `model="default"`, de modo que ensucia las estadísticas
de chat con llamadas que no son conversación del usuario. Es un asunto independiente, **pendiente y sin
tarea asignada**; no se abordó en este change.

## 10. Estado y siguientes pasos

1. ✅ Change `tool-usage-stats` aprobado, implementado con TDD (RED → GREEN → REFACTOR) y archivado en
   `openspec/changes/archive/2026-11-06-tool-usage-stats/`.
2. ✅ Release **v0.31.1** mergeada a `main` y desplegada en `co1`.
3. ✅ Verificación funcional en producción: `email_list_unread: 1` en `tools_summary`.
4. ⏸️ Pendiente sin tarea: la ruta de reflexión con `model="default"` que ensucia las stats de chat
   (§9).
