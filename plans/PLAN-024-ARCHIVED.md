# PLAN-024 — WordPress: credenciales del blog desde Ajustes

- **Issue de entrada:** [#332](https://github.com/atareao/valet-ai/issues/332) (asignado a `atareao`)
- **Change de OpenSpec:** `openspec/changes/wordpress-credentials-settings/`
- **Estado:** spec redactada y validada; **pendiente de aprobación** antes de TDD

## Objetivo

Que el usuario pueda configurar desde **Ajustes → Integraciones → Blog** el usuario de servicio, la Application Password y la URL base de WordPress, y usar las herramientas de comentarios (`blog_comment_reply`, `blog_comment_delete`) sin tocar el servidor.

## Diagnóstico (verificado)

- `src/services/wordpress.rs` resuelve las credenciales **solo del entorno** (`BLOG_BASE_URL`, `BLOG_SERVICE_USER`, `BLOG_APP_PASSWORD`) en cada llamada.
- En producción (`co1`) esas variables **no están definidas**: el agente no puede responder a comentarios.
- La tabla `comment_replies` existe y está **vacía**: nunca se publicó una respuesta desde Valet.

## Alcance (decisiones del usuario)

- **Solo mover credenciales a Ajustes**; mismas operaciones, sin operaciones nuevas.
- **Sin fallback de entorno**: la única fuente es `settings`.
- `blog_app_password` **sensible** (`SENSITIVE_KEYS`); `blog_base_url` y `blog_service_user` no sensibles.

## Cambios

1. Nuevos ajustes: `blog_base_url` (def. `https://atareao.es`), `blog_service_user`, `blog_app_password` (sensible).
2. `blog_app_password` en `SENSITIVE_KEYS`; `GET /api/blog/status` → `{configured, base_url, user}` sin la contraseña; `PUT /api/blog/password` (400 vacío / 204 ok).
3. Sonda `GET /api/blog/check` (siempre 200 `{ok, detail}`), molde `/api/opentodo/check`.
4. `src/services/wordpress.rs`: `resolve_config(pool)` async desde `settings`, sin fallback.
5. `src/tools/blog_comments.rs`: `resolve_config` async usando `self.db`.
6. Migración aditiva e idempotente `migrations/<timestamp>_blog_credentials.sql`.
7. Frontend: sub-pestaña «Blog» (`BlogSection.tsx`), contraseña siempre vacía, «Probar conexión».
8. Retirada de `BLOG_*` de `.env.example`, `.env.j2`, `.env.prod.sample`.

## Verificación

- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- `vitest run`, `npm run lint`, `npx tsc --noEmit` en `frontend/`.
- Auditoría `rust-reviewer` + `react-reviewer`.
- **E2E real:** configurar credenciales en Ajustes y publicar una respuesta a un comentario real; `check` → `{ok:true}`.

## Follow-ups

- No se migran credenciales desde variables de entorno: el usuario las pega en Ajustes.
- `docs/wordpress-integration.md` se crea al cerrar.