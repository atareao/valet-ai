# Design: Integración ZenNotes

## Arquitectura

Nueva capa de servicio + una herramienta de dominio, siguiendo los moldes del repositorio:

| Pieza | Fichero | Molde |
|---|---|---|
| Cliente HTTP + resolución de config + errores | `src/services/zennotes.rs` | `src/services/miniflux.rs` |
| Herramienta de dominio con `operation` | `src/tools/zennotes.rs` | `src/tools/calendar.rs` |
| Skill del catálogo cerrado | `src/orchestrator/skills.rs` | skill `email`/`rss` |
| Siembra de ajustes y prompt | `migrations/*.sql` | `20261010000006_email_skill.sql` |

## Configuración resuelta

Orden: `settings` → entorno → valor por defecto (solo URL).

| Clave `settings` | Entorno | Default | Sensible |
|---|---|---|---|
| `zennotes_base_url` | `ZENNOTES_BASE_URL` | `https://zennotes.territoriolinux.es` | no |
| `zennotes_token` | `ZENNOTES_TOKEN` | (vacío) | **sí** |
| `zennotes_folder` | `ZENNOTES_FOLDER` | `Valet` | no |

El token viaja en `Authorization: Bearer <token>` y **nunca** debe aparecer en un `Debug`, un log ni
el mensaje de un error (igual que `MinifluxConfig`).

## Taxonomía de errores

`ZennotesError` con `MissingCredentials`, `Unauthorized` (401/403), `NotFound` (404), `Unavailable`
(red/5xx) y `Http(String)`. Mensajes en español; ninguno incluye el token. `MissingCredentials` (no
hay token) es distinto de `Unauthorized` (el token no vale).

## Operaciones de la herramienta `zennotes`

| `operation` | Argumentos | Efecto | Permiso |
|---|---|---|---|
| `list` | `folder?`, `subpath?`, `tag?`, `limit?` (def 20, máx 200) | resumen de notas | `NoConfirm` |
| `search` | `query` (obs), `mode?` = `text`\|`title`\|`tag` (def `text`), `limit?` (def 20) | coincidencias | `NoConfirm` |
| `read` | `path` (obs) | nota completa (body ≤ 20.000 car.) | `NoConfirm` |
| `create` | `title` (obs), `body?`, `folder?` (def `inbox`; enum `inbox`\|`quick`\|`archive`), `subpath?` (def el ajuste `zennotes_folder` = `Valet`) | crea en `inbox/Valet/`; con `body` son dos viajes (create + write) | `Notify` |
| `append` | `path` (obs), `text` (obs) | añade al final (read + write) | `Notify` |
| `replace` | `path`, `find`, `replace`, `occurrence?` = `first`\|`all` | reemplaza literal | `Notify` |
| `write` | `path` (obs), `body` (obs) | sobrescribe el cuerpo | `ExplicitApproval` |
| `move` | `path`, `target_folder` (`inbox`\|`quick`\|`archive`), `target_subpath?` | mueve | `Notify` |
| `archive` / `unarchive` | `path` | archiva / desarchiva | `Notify` |
| `rename` | `path`, `new_title` | renombra | `Notify` |
| `trash` | `path` | a la papelera | `ExplicitApproval` |
| `restore` | `path` | restaura de la papelera | `Notify` |
| `delete` | `path` | borra definitivamente | `ExplicitApproval` |

`empty-trash` **no se expone**.

## Contrato del listado (recorte)

Para no volcar 6.273 notas al prompt, `list` SHALL devolver un objeto con `count` (total tras filtro) y
`notes` (hasta `limit`), y cada nota SHALL incluir **solo** `path`, `title`, `folder`, `tags` y
`updatedAt`. `search` en modo `text` SHALL devolver `path`, `title`, `lineNumber` y `lineText`; en modo
`title`/`tag` SHALL filtrar localmente el listado de metadatos. `read` SHALL devolver el `body`
recortado a 20.000 caracteres e indicar si hubo recorte.

## Endpoints de ajuste del token

`PUT /api/settings` ignora las claves de `SENSITIVE_KEYS`, así que un token oculto no se puede
guardar por esa vía. Se añaden (molde Strava):

- `GET /api/zennotes/status` → `200 {"configured":bool,"base_url":"…","folder":"…"}` (sin token).
- `PUT /api/zennotes/token` → cuerpo `{"token":"…"}`; token vacío → `400`; válido guarda y devuelve `204`.

## Fuera de alcance
Vista de notas, MCP, assets, escritura de tasks de ZenNotes.
