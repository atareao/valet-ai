# HANDOFF

**Fecha:** 2026-11-06

## Estado

Change `tool-usage-stats` **cerrado y archivado** (`openspec/changes/archive/2026-11-06-tool-usage-stats/`). El arreglo del registro de `tool_calls` en `llm_requests` entró por PR #323 a `development` (`9765256`) y se publicó como release **`v0.31.1`** (PR #324 a `main`, merge `3d529ac`), desplegada en `co1`. El panel **Stats → Sistema → Tool Usage** ya no sale vacío.

## Release v0.31.1

- **PR #323** ✅ — mergeado a `development` con CI verde. Hash de merge: `9765256f5f8eb06428f54ca36c32358ebfafa566`.
- **Release branch** `release/v0.31.1`: `vampus upgrade --patch` dejó `0.31.1` en `Cargo.toml`, `.vampus.yml` y `Cargo.lock`; commit `🔧 chore(release): v0.31.1` (`936b20b`).
- **PR #324** ✅ — CI verde, mergeado a `main`. Hash de merge: `3d529ac049193b2472757243838f2cf019dfe0fe`.
- **Tag `v0.31.1`** ✅ — publicado; workflow `image.yml` (run `42280704435`) **success**.
- **Despliegue `co1`** ✅ — health: `{"version":"0.31.1"}`.
- **Verificación E2E** ✅ — `email_list_unread: 1` en la agregación equivalente a `tools_summary` (fila `created_at=2026-11-06 20:36:43`).

## Tareas pendientes

- **4.10 / 5.3** — verificación manual/visual de las vistas de la etapa A del frontend (React 19). **A cargo del usuario.**
- **Ruta de reflexión** (fuera de alcance del change, sin tarea): graba `kind='chat'` con `model="default"` y ensucia las stats de chat.

## Hallazgos / notas (no bloqueantes)

- **Sin backfill del histórico** (decisión del usuario): los turnos previos a la v0.31.1 quedan sin `tool_calls`; el gráfico se puebla a partir del despliegue. Descartado rellenarlo desde `messages.tools_used` (agregado por turno, no por llamada al LLM).
- Bundle inicial del frontend al límite: 69.99 kB / 70.00 kB (headroom ~1.7 %). No se sube el límite.
- `dependency-stack.test.ts` solo cubre la etapa A del frontend (candidato a endurecer, sin tarea asignada).

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development` salvo merges de PR.
- `docker-compose.prod.yml` no se toca.
- No relajar aserciones de tests.
