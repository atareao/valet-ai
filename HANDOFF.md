# HANDOFF

**Fecha:** 2026-11-07

## Estado

**COMPLETADO.** La E2E real de OxiMeal (que quedaba pendiente) está hecha: `GET /api/oximeal/check` → `{ok:true}` verificado en `co1` con la **v0.32.1** desplegada (Dockge). El change quedó archivado (`2026-11-07-oximeal-m2m-resource`) y el plan vive en `plans/PLAN-022-ARCHIVED.md`. No queda ninguna tarea crítica.

## Qué se ha hecho

- **PR #329** (`feature/oximeal-m2m-resource` → `development`), **release v0.32.1** (PR #330 → `main`, tag `v0.32.1`, imagen `ghcr.io/atareao/valet-ai:{v0.32.1,0.32,latest}`) y **back-merge a `development`**.
- **Cambios:** `resource` en la petición `client_credentials` (resuelto ajuste `oximeal_resource` → env `OXIMEAL_RESOURCE` → derivado `<base_url>/api`), `status.resource`, `check` con detalle redactado del IdP y campo UI «OxiMeal · Resource».
- Auditorías `@rust-reviewer` y `@react-reviewer` sin bloqueantes.
- Change archivado y plan en `plans/PLAN-022-ARCHIVED.md`.

## Tarea pendiente / siguiente paso

- Ninguna tarea crítica. Candidatas:
  - Los **5 follow-ups menores** del change (en `docs/oximeal-m2m-resource.md`): doble lectura de `base_url` en `resolve_config`; normalización del `resource` explícito; brazo `_` de `check_detail` para `NotFound`/`InvalidInput`; `redact` del token M2M; test del reset de `oximeal_resource`.
  - La **E2E de las operaciones concretas del agente** (`search_recipes`, `plan_week`, `add_shopping_items`, `clear_purchased`, `create_recipe`) desde el chat.

## Follow-ups conocidos

- Causa raíz a recordar: **Pocket ID exige `resource` (RFC 8707)** para scopes personalizados; sin él responde `invalid_scope` y el `aud` del token no coincide con `OIDC_API_AUDIENCE` de OxiMeal.
- Los 5 follow-ups menores del change listados arriba.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development`.
- No relajar aserciones de tests.
