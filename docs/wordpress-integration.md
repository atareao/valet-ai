# Integración con WordPress (blog)

## Qué es

Integración de Valet con el blog WordPress (`https://atareao.es`): el agente
publica respuestas a comentarios (`blog_comment_reply`) y manda comentarios a la
papelera (`blog_comment_delete`). Las credenciales se gestionan desde **Ajustes**
(ya no por variables de entorno).

## Ficheros implicados

- `src/services/wordpress.rs` — cliente REST `Wordpress`: `resolve_config(pool)`
  (lee de `settings`), `check_connection`, `publish_comment_reply`,
  `fetch_comment`, `trash_comment`.
- `src/tools/blog_comments.rs` — herramientas `blog_comment_reply` y
  `blog_comment_delete`; resuelven `comment_id`/`post_id` desde el `metadata` de
  la notificación.
- `src/handlers/blog.rs` + `src/routes/blog.rs` — `GET /api/blog/status`,
  `GET /api/blog/check`, `PUT /api/blog/password`.
- `src/db/repos/settings.rs` — `SENSITIVE_KEYS` incluye `blog_app_password`.
- `migrations/20261110000001_blog_credentials.sql` — siembra aditiva e
  idempotente de `blog_base_url=https://atareao.es`, `blog_service_user=''`,
  `blog_app_password=''`.
- Frontend: `frontend/src/components/BlogSection.tsx`,
  `frontend/src/components/IntegrationsTab.tsx`, `frontend/src/hooks/useBlog.ts`.

## Cómo funciona

- Cada llamada de las herramientas resuelve las credenciales **desde `settings`**
  (`resolve_config`, async). **No** hay respaldo por entorno: sin usuario o
  contraseña, la herramienta falla sin salir a la red.
- Base por defecto: `https://atareao.es`.
- `blog_app_password` es **sensible**: `GET /api/settings` la omite y
  `PUT /api/settings` la ignora; la única puerta es `PUT /api/blog/password`
  (`{"password"}` → `400` si vacío, `204` si se guarda). El campo del frontend
  arranca **siempre vacío**.
- `GET /api/blog/status` → `{configured, base_url, user}` (nunca la contraseña).
- `GET /api/blog/check` → sonda real: `GET <base>/wp-json/wp/v2/users/me` con la
  credencial; responde siempre `200` con `{ok, detail}`.
- Publicar: `POST /wp-json/wp/v2/comments` con
  `{post, parent, content, status:"approved"}` (sin `author_name`/`author_email`).
  Borrar: `DELETE /wp-json/wp/v2/comments/<id>?force=false` (papelera, reversible
  30 días).

## Configuración

| Ajuste | Por defecto | Sensible |
|---|---|---|
| `blog_base_url` | `https://atareao.es` | no |
| `blog_service_user` | `''` | no |
| `blog_app_password` | `''` | **sí** |

Las variables `BLOG_BASE_URL`, `BLOG_SERVICE_USER` y `BLOG_APP_PASSWORD` **ya no
se leen** (retiradas de `.env.example`, `.env.j2` y `.env.prod.sample`).

## Cómo habilitar / desplegar

1. **Ajustes → Integraciones → Blog**: URL base + usuario + Application Password
   → «Guardar». «Probar conexión» usa `GET /api/blog/check`.
2. **Aprovisionar el usuario de servicio en el sitio WordPress** (ver abajo). Sin
   esto las respuestas fallan con `404 rest_no_route`.
3. Publicar la imagen y recrear el stack en `co1` (Dockge).

## Aprovisionamiento en el sitio WordPress (imprescindible)

El plugin del sitio (`atareao-functionality`) elimina la ruta de **colección**
`POST /wp-json/wp/v2/comments` salvo que la petición provenga del «usuario de
servicio», identidad que lee de la opción **`atareao_valet_reply_user`**
(`ValetConfig::isServiceRequest()`). Esa opción es el **ID** de la cuenta cuya
Application Password usa Valet.

```bash
wp option update atareao_valet_reply_user <ID>
wp option get atareao_valet_reply_user
```

- Con la opción a `0` o ausente, la vía de respuesta queda **inerte** y Valet
  recibe `404 rest_no_route`.
- La opción guarda solo un **identificador**, nunca una credencial. Revocar la
  Application Password desactiva la vía al instante.
- El sitio conserva únicamente el `POST` (publicar) y descarta el `GET`
  (no-enumeración): `GET /wp-json/wp/v2/comments` sigue devolviendo `404`.

## Decisiones

- **Las credenciales viven en `settings`**, no en el entorno. La contraseña es
  sensible y solo se escribe por `PUT /api/blog/password`.
- **Mismas operaciones** que antes del change: solo cambia el origen de las
  credenciales.
- **Fallar a cerrado** en el sitio: sin usuario de servicio, la colección se
  elimina entera.

## Follow-ups

- **Sitio** — aprovisionar el usuario de servicio desde la UI del plugin (hoy es
  un paso manual de WP-CLI): https://github.com/atareao/atareao.es/issues/175
- **Consumidor** — endurecer `GET /api/blog/check` para que detecte la vía de
  respuesta muerta (hoy `users/me` da `200` aunque la escritura esté cerrada):
  https://github.com/atareao/valet-ai/issues/340
- **Seguridad** — la cuenta de servicio es `yyyxfsgj` con rol `administrator`;
  el plugin recomienda un usuario dedicado con rol mínimo (`read` +
  `moderate_comments`). Sin tarea asignada.
