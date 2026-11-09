# HANDOFF

**Fecha:** 2026-11-09

## Estado

**COMPLETADO.** El roadmap de tres features (OxiMeal, WordPress, GitHub) está cerrado. La
integración con GitHub (issue #333) quedó implementada, auditada, desplegada como **v0.35.0** en
`co1` y **verificada E2E** con un PAT clásico real: la tool creó el issue #344 por conversación
(skill `Github` 0.99, `success:true`), `check` → `{ok:true}`, `GET /api/settings` sin
`github_token` y 0 filtraciones en logs. No queda ninguna tarea crítica.

## Qué se ha hecho (GitHub)

- **Código** (PR #342 → `development`, CI verde): servicio `Github`, tool `github` (`create_issue`,
  permiso `Notify`), endpoints `status`/`token`/`check`, `github_token` sensible, ajustes
  `github_default_repo`/`github_default_labels`, skill `github` (14 skills / 35 tools), migración
  `20261111000001_github`.
- **Release v0.35.0** (PR #343 → `main`, tag `v0.35.0`, imagen publicada) y back-merge a `development`.
- **Despliegue** en `co1` (Dockge): `/api/health` → `{"version":"0.35.0"}`; migración
  `20261111000001|github|1` aplicada.
- **E2E** (local con copia de BD + PAT real, y turno real de la tool): router selecciona `Github`
  (0.99), issue **#344** creado y luego cerrado.
- **Cierre documental** (PR #345 → `development`, CI verde): change archivado
  (`2026-11-09-github-integration`), `Purpose` real de `github`/`github-api`, `docs/github-integration.md`,
  `plans/PLAN-025-ARCHIVED.md`, AGENTS §V entrada 24 + índice §VI, issue **#333** cerrado.
- Auditorías `@rust-reviewer` y `@react-reviewer` sin bloqueantes; hallazgos aplicados.

## Tarea pendiente / siguiente paso

- Ninguna tarea crítica. Candidatas (sin asignar):
  - **Valet-ai #340**: endurecer `GET /api/blog/check` para detectar la vía de respuesta muerta
    (hoy sondea solo `users/me`).
  - **atareao.es #175**: aprovisionar `atareao_valet_reply_user` desde la UI del plugin.
  - `frontend/src/test/dependency-stack.test.ts` solo cubre la etapa A de React 19 (etapa B sin cubrir).

## Follow-ups conocidos

- **Bundle inicial al límite:** 69.98/70.00 kB. Los métodos por integración **no** van en
  `api/client.ts` (grafo inicial); van en su propio `api/<X>.ts` importado por un hook lazy.
  No subir el límite del control de bundle.
- **Archive deja el `Purpose` en placeholder** para capabilities nuevas: reescribirlo a mano y
  volver a pasar `openspec validate --all --strict`.
- **Pedir el dominio público desde `co1` da `403 Ko`** (bloqueo de IP origen/WAF); desde fuera
  responde 200.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development`.
- No relajar aserciones de tests.
