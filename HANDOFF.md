# HANDOFF

**Fecha:** 2026-11-07

## Estado

La integración de OxiMeal está en **`main`** y desplegada como **v0.32.0** en **`co1`**, verificada con `GET /api/health` → `{"version":"0.32.0"}`. El change de openspec quedó **archivado** (`2026-11-07-oximeal-integration`) y el plan vive en `plans/PLAN-021-ARCHIVED.md`. **Única tarea pendiente: la E2E real de OxiMeal.**

## Qué se ha hecho

- **Tool `oximeal`** con 6 operaciones: `recent_recipes`, `search_recipes`, `plan_week`, `add_shopping_items`, `clear_purchased`, `create_recipe`.
- **Skill `menus`**: catálogo de 12 → **13 skills** y 33 → **34 tools**.
- **Servicio OxiMeal** (`src/services/oximeal.rs`) con token **M2M single-flight**: descubre `token_endpoint` vía OIDC, cachea el token con margen de 30 s y lo invalida ante `401`/`403`.
- **Endpoints `/api/oximeal/{status,check,secret}`**: `status` nunca expone el secreto; `check` es la sonda real (siempre `200` con `{ok, detail}`); `secret` guarda (400 vacío / 204 ok).
- **`oximeal_client_secret`** añadido a `SENSITIVE_KEYS`: `GET /api/settings` lo omite y `PUT /api/settings` lo ignora.
- **Migración** `20261107000001_oximeal.sql`: siembra `oximeal_base_url`, `oximeal_client_id` y `SKILL_MENUS_PROMPT`.
- **UI** en Ajustes → Integraciones (`useOximeal.ts` + `OximealSection.tsx`): campos de base/issuer/client_id, secreto siempre vacío al abrir y botón «Probar conexión».
- Documentación en `docs/oximeal-integration.md`.
- **Cierre de fase 1**: merge a `development` (PR #325), `openspec archive oximeal-integration` y `plans/PLAN-021-ARCHIVED.md`. Puertas: `cargo test` 1653+, `clippy` limpio, `fmt`, `vitest` 692, `tsc`, `eslint`, bundle 69.99/70.00 kB.
- **Cierre de release**: PR #327 (`release/v0.32.0` → `main`), tag **v0.32.0**, imagen publicada por `image.yml` y desplegada en **`co1`** (Dockge); verificada con `GET /api/health` → `{"version":"0.32.0"}`.

## Tarea pendiente / siguiente paso

- **E2E real de OxiMeal**: requiere **desplegar OxiMeal** y crear el **cliente M2M en PocketID** (scopes `agent:read agent:plan agent:shopping agent:write`). Después: `GET /api/oximeal/check` → `{ok:true}` y las operaciones `search_recipes`, `plan_week`, `add_shopping_items`, `clear_purchased`, `create_recipe`. Documentado en `docs/oximeal-integration.md`.

## Follow-ups conocidos

- El **example `curl` del README de OxiMeal está desactualizado**: no coincide con la API real de la Agent API.
- La **E2E queda pendiente por infraestructura** (OxiMeal sin desplegar en `co1`, sin cliente M2M real).

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development` salvo merges de PR.
- `docker-compose.prod.yml` no se toca.
- No relajar aserciones de tests.
