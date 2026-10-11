# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

## 1. Aprobación, migración y ajustes

- [x] 1.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de tocar código. Es el punto de parada obligatorio del protocolo OpenSpec: sin un «aprobado» del usuario, no se avanza a la sección 2.
- [x] 1.2 RED — test que falla de la migración (`tests/db/rss_migration.rs`, siguiendo el molde de `tests/db/email_migration.rs`): tras migrar existen las claves `ROUTER_SKILL_RSS_ENABLED` (con `'true'`), `SKILL_RSS_PROMPT`, `miniflux_base_url` (con `https://miniflux.territoriolinux.es`) y `miniflux_api_key` (sembrada **vacía**); un valor previo escrito por el usuario **no se sobrescribe**; volver a aplicar la migración es **idempotente**. Registrar el target `[[test]]` `name = "rss-migration"` / `path = "tests/db/rss_migration.rs"` en `Cargo.toml`, junto al de `email-migration`. Verificar en rojo con `cargo test --test rss-migration` (el target aún no existe o el test falla).
- [x] 1.3 GREEN — implementar `migrations/20261010000007_rss_skill.sql`: siembra aditiva e idempotente con el idioma de la migración de la skill `email` (`INSERT ... ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now') WHERE settings.value = '' OR settings.value IS NULL`, que también rellena una fila existente con valor vacío), de las cuatro claves y **sin reconstruir ninguna tabla**. Confirmar con `cargo test --test rss-migration` en verde.
- [x] 1.4 Documentar `MINIFLUX_BASE_URL` y `MINIFLUX_API_KEY` en `.env.example` y en `.env.j2`, con `https://miniflux.territoriolinux.es` como ejemplo de URL base.

## 2. Cliente de Miniflux (`src/services/miniflux.rs`)

- [ ] 2.1 RED — tests que fallan contra un servidor de prueba HTTP con `wiremock` (mismo molde que `src/services/apimail.rs`): la configuración se resuelve **en cada llamada** y el ajuste de `settings` (`miniflux_base_url` / `miniflux_api_key`) tiene **precedencia** sobre la variable de entorno; el token viaja en la cabecera `X-Auth-Token`; **sin token no se envía ninguna petición** y el error dice dónde configurarlo; un `401` se traduce a un error accionable de credenciales inválidas y **el token no aparece en el mensaje de error**. Verificar en rojo con `cargo test miniflux`.
- [ ] 2.2 GREEN — implementar `src/services/miniflux.rs` y exponerlo desde `src/services/mod.rs`: base inyectable, cabecera `X-Auth-Token`, resolución de configuración en cada llamada (ajuste primero, entorno después, valor por defecto **solo para la URL**) y traducción de los errores de la API v1 a mensajes accionables, enmascarando siempre el token. Confirmar `cargo test miniflux` y `cargo check` en verde.

## 3. Las cuatro herramientas (`src/tools/rss.rs`)

- [ ] 3.1 RED — test que falla de `rss_list_unread`: devuelve el `total` y los metadatos de cada entrada (`id`, `title`, `feed`, `url`, `published_at`, `reading_time`) **sin contenido**, con `limit` por defecto 50.
- [ ] 3.2 RED — test que falla de validación de argumentos: un `limit` fuera de `1..=200` devuelve `InvalidArguments` **sin salir a la red**; un `category` resuelve el nombre a su id y, si no existe, el error **enumera las categorías disponibles**.
- [ ] 3.3 RED — test que falla de `rss_get_entry`: devuelve el contenido en **texto plano recortado a 20 000 caracteres** y **no** marca la entrada como leída.
- [ ] 3.4 RED — test que falla de `rss_search_entries`: la petición va **sin filtro de estado** (encuentra lo ya leído) y una búsqueda sin resultados devuelve **lista vacía**, no un error.
- [ ] 3.5 RED — test que falla de `rss_mark_read`: marca los `entry_ids` en **una sola petición** (`PUT /v1/entries`), es **idempotente** y rechaza una **lista vacía sin salir a la red**.
- [ ] 3.6 GREEN — implementar las cuatro herramientas en `src/tools/rss.rs` (todas con `Permission::NoConfirm`) y registrarlas en `src/tools/mod.rs` y en `build_tool_registry` (`src/lib.rs`). Confirmar `cargo test rss` y `cargo check` en verde.

## 4. Catálogo y enrutado

- [ ] 4.1 RED — tests que fallan de integridad en `src/orchestrator/skills.rs`: el catálogo declara **diez** skills; sus claves de fragmento son diez e incluyen `SKILL_RSS_PROMPT`; la unión del core y las diez skills cubre las **veintiocho** herramientas sin huérfanas; `rss` cubre exactamente las cuatro `rss_*`.
- [ ] 4.2 RED — test que falla del router: el fallo abierto con todas las skills habilitadas expone las **veintiocho** herramientas; actualizar la lista `ALL_TOOLS` de los tests de `src/orchestrator/skill_router.rs`.
- [ ] 4.3 GREEN — implementar la variante `Skill::Rss` y su entrada en el catálogo (pregunta, dos criterios, umbral, `prompt_key` `SKILL_RSS_PROMPT` y encabezado `# SKILL ACTIVA: RSS`, habilitada por defecto), con sus **dos aserciones de recuento**. Actualizar el comentario de `src/lib.rs` que dice «all 24 built-in tools» a las veintiocho del registro. Confirmar `cargo test skills` y `cargo test skill_router` en verde.

## 5. Interfaz

- [ ] 5.1 RED — test que falla de `SettingsDialog` (siguiendo el patrón de los campos de apimail): la pestaña «API Keys» muestra los dos campos nuevos de Miniflux —URL base (con marcador `https://miniflux.territoriolinux.es`) y API key— y los envía al guardar. Verificar en rojo con `npx vitest run`.
- [ ] 5.2 GREEN — implementar los dos campos en `frontend/src/components/SettingsDialog.tsx`. Confirmar `npx vitest run` y `npx tsc --noEmit` en verde.

## 6. Verificación

- [ ] 6.1 Anotar el recuento de tests **antes y después** y dejar la suite de Rust completa en verde con `cargo test --no-fail-fast` (0 fallos).
- [ ] 6.2 `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios (0 warnings).
- [ ] 6.3 En el frontend, `npx vitest run`, `npx tsc --noEmit` y el lint (`npm run lint`) en verde.
- [ ] 6.4 Ya con el token puesto por el usuario en Ajustes → API Keys, comprobar en producción que el asistente **lista los no leídos**, **lee un artículo entero** (`rss_get_entry`) y **lo marca como leído** cuando se lo piden (`rss_mark_read`), sin marcarlo al leerlo.

## Notas

- El token lo pone el usuario a mano en **Ajustes → API Keys**: la migración siembra `miniflux_api_key` **vacía a propósito**, para que el valor no quede en el repositorio ni en el historial de migraciones. Sin token configurado, el cliente **no envía ninguna petición** y devuelve un error que indica dónde configurarlo.
- La migración `20261010000007_rss_skill.sql` es **aditiva e idempotente** y **no reconstruye ninguna tabla** — se remarca aquí porque es lo primero que se revisa tras el incidente de la migración del timeline.
- En producción hay que comprobar, contra la instancia `https://miniflux.territoriolinux.es`: que el asistente enumera los no leídos sin traer contenido, que `rss_get_entry` devuelve el artículo en texto plano sin marcarlo como leído, que `rss_search_entries` encuentra también lo ya leído y que `rss_mark_read` deja la entrada como leída de forma idempotente.
