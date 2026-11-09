# Design

## Context

Ver `proposal.md` — Why. Estado actual relevante:

- `src/services/wordpress.rs`: `DEFAULT_BASE_URL = "https://atareao.es"` (:17); `WordpressConfig` (:134); `Wordpress::resolve_config()` (:236) es **estático, síncrono y lee el entorno** en cada llamada; el cliente `Wordpress` guarda `base_url` y recibe `config` solo para el `basic_auth`.
- `src/tools/blog_comments.rs`: `BlogCommentReplyTool`/`BlogCommentDeleteTool` tienen `db: SqlitePool` y un `config: Option<WordpressConfig>` para tests; `resolve_config(&self)` (:208/:534) es **síncrono** y cae a `Wordpress::resolve_config()`.
- Ajustes: `SettingsRepo::get/set`; `SENSITIVE_KEYS` en `src/db/repos/settings.rs`; moldes de integración con endpoint dedicado en `src/handlers/zennotes.rs`, `opentodo.rs`, `oximeal.rs` + sonda `/check` siempre `200`.
- Frontend: `IntegrationsTab.tsx` con sub-pestañas; molde `OximealSection.tsx`/`OpentodoSection.tsx`.

## Goals / Non-Goals

**Goals**
- Una **única fuente de verdad** para las credenciales del blog: `settings`.
- Secreto (`blog_app_password`) invisible en `GET /api/settings`, en logs y en `Debug`.
- Paridad de UX con ZenNotes/OpenTodo/OxiMeal (estado + «Probar conexión»).

**Non-Goals**
- No se añaden operaciones nuevas a las herramientas del blog (mismo `blog_comment_reply`/`blog_comment_delete`).
- No se toca WordPress ni su API.
- No se migran credenciales desde variables de entorno al primer arranque (el usuario las pega en Ajustes).

## Decisions

- **D1 — Resolución desde `settings`, sin fallback.** `Wordpress::resolve_config` pasa a recibir el `SqlitePool` y leer `blog_base_url`/`blog_service_user`/`blog_app_password` con `SettingsRepo::get` (no `setting_or_env`). Alternativa descartada: `setting_or_env` (mantendría dos fuentes de verdad, contra la decisión del usuario).
- **D2 — `resolve_config` async.** Los `resolve_config(&self)` de las herramientas pasan a `async` y usan `self.db`; se conserva el `config: Option<WordpressConfig>` inyectado para los tests sin BD/env.
- **D3 — Secreto sensible con puerta dedicada.** `blog_app_password` en `SENSITIVE_KEYS`; única puerta `PUT /api/blog/password` (`400` vacío / `204` ok). Molde `zennotes_token`.
- **D4 — Sonda real `GET /api/blog/check`.** Consulta `GET <base_url>/wp-json/wp/v2/users/me` con autenticación básica; siempre `200` con `{ok, detail}`; nunca la contraseña. Molde `/api/opentodo/check`.
- **D5 — `blog_base_url` y `blog_service_user` no sensibles.** Viajan en `GET`/`PUT /api/settings`; `GET /api/blog/status` devuelve `{configured, base_url, user}` sin la contraseña.
- **D6 — Migración aditiva e idempotente** (patrón `INSERT … ON CONFLICT DO UPDATE … WHERE value='' OR value IS NULL`) que siembra `blog_base_url=https://atareao.es`, `blog_service_user=''` y `blog_app_password=''`. Sin valor por defecto para el secreto.
- **D7 — Corrección de deriva de spec.** El requirement «organize the Integraciones tab in sub-tabs» listaba 8 sub-pestañas y omitía OxiMeal (ya presente en el código); se corrige a 10 incluyendo OxiMeal y Blog.

## Risks / Trade-offs

- [Ruptura para quien dependía de `BLOG_*`] → Se retiran de `.env.example`/`.env.j2`/`.env.prod.sample` y se documenta en `docs/wordpress-integration.md`; el error es accionable y remite a Ajustes.
- [`resolve_config` síncrono usado en tests sin BD] → Se conserva el `config` inyectado; los tests existentes que fijan el entorno se adaptan a la inyección de config o a una BD en memoria.
- [Fuga del secreto en la sonda o en logs] → Se redacta en `Debug`, no se registra la cabecera `Authorization` y la sonda solo devuelve `{ok, detail}`.
- [Migración pisando ajustes del usuario] → `WHERE value='' OR value IS NULL`; test de «respeta un valor ya fijado».

## Migration Plan

1. Migración nueva `migrations/<timestamp>_blog_credentials.sql` (sembrado idempotente).
2. Merge → release → imagen → recrear el stack en `co1` (Dockge) para que corra la migración.
3. Rollback: la migración es aditiva (no destruye datos); revertir la imagen basta.

## Open Questions

- Ninguna bloqueante. El nombre final del fichero de migración se fija al crear la tarea 2.4.
