# Change: Skill `rss` — feeds (listar no leídos, leer un artículo, buscar y marcar como leído)

## Why

Hoy **no existe ninguna integración con RSS ni con Miniflux** en el proyecto. Se ha comprobado por
CLI: cero apariciones de «miniflux» en el repositorio, ninguna herramienta `rss_*` entre las
veinticuatro del registro y ninguna clave `MINIFLUX_*`. Esto **no es un cambio sobre algo
existente: es una capability nueva de cero**, no la modificación de un lector que ya estuviera
wireado.

El usuario lee sus noticias en una instancia propia de **Miniflux**
(`https://miniflux.territoriolinux.es`), cuya API HTTP v1 autentica todas sus rutas con la cabecera
`X-Auth-Token: <API key>` y resuelve las operaciones que hacen falta: listar las entradas no leídas
(`GET /v1/entries?status=unread`), obtener una entrada por su id (`GET /v1/entries/{id}`), buscar
por palabra clave sin filtrar por estado (`GET /v1/entries?search=…`, de modo que encuentra también
lo ya leído) y marcar entradas como leídas en una sola petición
(`PUT /v1/entries` con `{"entry_ids":[…],"status":"read"}`, idempotente).

El enrutador de skills ya sabe repartir capacidades por dominio: una skill nueva con sus
herramientas basta para que el modelo pueda consultar las noticias **solo** en los turnos en que
haga falta, sin engordar el prompt de todos los turnos. El molde que se sigue es la skill `email`
sobre apimail, ya archivada.

## What Changes

- **Nueva skill `rss`** en el catálogo cerrado (`Skill::Rss`), la **décima**, con su pregunta, sus
  dos criterios, su umbral, su clave de fragmento `SKILL_RSS_PROMPT` y su encabezado
  `# SKILL ACTIVA: RSS`. Habilitada por defecto.
- **Cuatro herramientas nuevas**, registradas en `build_tool_registry` y cubiertas por la skill
  `rss`: `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`. Las cuatro
  declaran `Permission::NoConfirm`: nada de esto es irreversible ni sale del sistema, así que no hay
  aprobación explícita que pedir.
  1. **`rss_list_unread`** — las entradas no leídas **sin contenido** (`limit` opcional 1..=200, por
     defecto 50; `category` opcional por nombre): `id`, `title`, `feed`, `url`, `published_at`,
     `reading_time` y el `total`.
  2. **`rss_get_entry`** — el contenido de un artículo por su `entry_id`, en texto plano y recortado
     a 20 000 caracteres, **sin** marcarlo como leído.
  3. **`rss_search_entries`** — busca por palabra clave (`query` obligatorio, `limit` opcional) sin
     filtrar por estado, de modo que encuentre también lo ya leído.
  4. **`rss_mark_read`** — marca como leídos los `entry_ids` indicados en una sola petición;
     idempotente.
- **Decisión de diseño: listar y leer están separados.** El listado devuelve solo metadatos y
  **únicamente** `rss_get_entry` trae el cuerpo del artículo. Es la misma separación que en el
  correo, y es deliberada: si el listado arrastrara el contenido, veinte artículos meterían el
  cuerpo entero en el contexto del modelo cuando a menudo solo se quería ver los titulares.
- **Nueva capa de servicio `Miniflux`** (`src/services/miniflux.rs`) que resuelve la configuración y
  traduce los errores de la API a mensajes accionables. La URL base sale de `miniflux_base_url` en
  `settings`, cae a `MINIFLUX_BASE_URL` y, solo para la URL, a un valor por defecto
  `https://miniflux.territoriolinux.es`; el token sale de `miniflux_api_key` o de `MINIFLUX_API_KEY`.
  **Se leen en cada llamada.** Sin token no hay ninguna petición de red y el error dice dónde
  configurarlo; el token **nunca** aparece en un mensaje de error. `miniflux_api_key` es una clave
  **editable por el usuario**, igual que `apimail_api_key`, así que **no** va en `SENSITIVE_KEYS`
  (esa lista es para los tokens OAuth de Strava, que no son ajustes editables).
- **Migración aditiva e idempotente** (`migrations/20261010000007_rss_skill.sql`) que siembra
  `ROUTER_SKILL_RSS_ENABLED='true'`, `SKILL_RSS_PROMPT`, `miniflux_base_url` y `miniflux_api_key`
  vacía, sin pisar valores existentes. **No reconstruye ninguna tabla** — se dice explícitamente
  porque es lo primero que se mira tras el incidente de la migración del timeline.
- **Pestaña «API Keys»** con dos campos nuevos: la URL base de Miniflux y su API key.
- **Variables de entorno** `MINIFLUX_BASE_URL` y `MINIFLUX_API_KEY` documentadas en `.env.example` y
  `.env.j2`; se leen **directamente en el servicio**, como ya se hace con las de apimail, sin tocar
  `src/config.rs`.

## Impact

### Specs modificadas

- `orchestrator/skill-router`: el catálogo cerrado pasa de nueve a **diez** skills y cubre
  **veintiocho** herramientas; la consulta por API devuelve diez.
- `tools/registry`: el registro de producción incluye las cuatro herramientas `rss_*` y pasa de
  veinticuatro a **veintiocho**.
- `frontend`: la pestaña «API Keys» gana los campos de Miniflux.

### Specs nuevas

- `tools/rss`: las cuatro herramientas y su contrato con la API v1 de Miniflux.

### Código

- `src/services/miniflux.rs` (nuevo), `src/tools/rss.rs` (nuevo), `src/tools/mod.rs` y
  `build_tool_registry` en `src/lib.rs`, `src/orchestrator/skills.rs` (variante, catálogo y tests de
  integridad), el comentario de `src/lib.rs` que dice «all 24 built-in tools», la lista `ALL_TOOLS`
  de los tests de `src/orchestrator/skill_router.rs`,
  `frontend/src/components/SettingsDialog.tsx` (dos campos), una migración nueva en `migrations/`
  y su target de test `rss-migration` (`tests/db/rss_migration.rs`). Las variables `MINIFLUX_BASE_URL`
  y `MINIFLUX_API_KEY` se leen directamente en el servicio (mismo precedente que apimail y Strava),
  sin tocar `src/config.rs`.

### Datos

- Migración nueva que siembra cuatro claves. **No se reconstruye ninguna tabla.**

### No-objetivos

- **No se marca como no leído** ni se cambia el estado a ningún valor distinto de `read`.
- **No se destaca** (`starred`) ni se gestionan las estrellas.
- **No se crean ni editan feeds ni categorías**: solo se leen.
- **No hay interfaz de lector de noticias**: son herramientas para el asistente, no una UI de feeds.
- **No se usa `GET /v1/entries/{id}/fetch-content`**: reescribiría el contenido guardado en Miniflux
  como efecto secundario de una lectura.
- **No se toca Miniflux**: se consume su API tal y como está documentada.
- **No se toca `docker-compose.prod.yml`**: la configuración se hace desde la UI, con respaldo por
  variables de entorno a cargo del operador.
