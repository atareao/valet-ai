# Proposal

## Why

Las herramientas del blog (`blog_comment_reply`, `blog_comment_delete`) resuelven sus credenciales de WordPress —`BLOG_BASE_URL`, `BLOG_SERVICE_USER`, `BLOG_APP_PASSWORD`— **solo desde el entorno del servidor**, y en producción (`co1`) esas variables no están definidas. Resultado: el agente no puede responder a comentarios y el usuario no tiene forma de configurarlo desde la aplicación, a diferencia de ZenNotes, OpenTodo y OxiMeal. Issue [#332](https://github.com/atareao/valet-ai/issues/332).

## What Changes

- Las credenciales del blog pasan a resolverse desde `settings` en cada llamada. **BREAKING**: se retira el respaldo por variables de entorno `BLOG_*`.
- Nuevos ajustes: `blog_base_url` (por defecto `https://atareao.es`), `blog_service_user` y `blog_app_password` (esta última **sensible**).
- `blog_app_password` entra en `SENSITIVE_KEYS`: se omite en `GET /api/settings` y se ignora en `PUT /api/settings`. Se añaden la puerta dedicada `PUT /api/blog/password`, el estado `GET /api/blog/status` y la sonda `GET /api/blog/check`.
- Migración aditiva e idempotente que siembra los tres ajustes.
- Nueva sub-pestaña «Blog» en Ajustes → Integraciones, con su campo de contraseña que arranca siempre vacío.
- **No** cambian las operaciones de las herramientas: mismo comportamiento funcional, solo cambia el origen de las credenciales.

## Capabilities

### New Capabilities
- `blog-api`: superficie HTTP de la integración Blog —sensibilidad del secreto, endpoints dedicados (`GET /api/blog/status`, `PUT /api/blog/password`), sonda real (`GET /api/blog/check`) y siembra de ajustes por migración.

### Modified Capabilities
- `blog-comments`: la resolución de credenciales pasa del entorno a `settings`, sin respaldo de entorno.
- `frontend`: nueva sub-pestaña «Blog» en «Integraciones» y actualización del orden y del recuento de sub-pestañas.

## Impact

- **Backend:** `src/services/wordpress.rs` (resolución async desde `settings`), `src/tools/blog_comments.rs` (config async), `src/handlers/blog.rs` y `src/routes/blog.rs` (nuevos), `src/routes/mod.rs` y `src/lib.rs` (registro), `src/db/repos/settings.rs` (`SENSITIVE_KEYS`).
- **Datos:** migración nueva `migrations/<timestamp>_blog_credentials.sql`.
- **Frontend:** `BlogSection.tsx` (nueva), `IntegrationsTab.tsx`, hooks/API/tipos y sus tests.
- **Configuración:** retirada de `BLOG_BASE_URL`/`BLOG_SERVICE_USER`/`BLOG_APP_PASSWORD` de `.env.example`, `.env.j2` y `.env.prod.sample`.
- **Docs:** nueva `docs/wordpress-integration.md`.
