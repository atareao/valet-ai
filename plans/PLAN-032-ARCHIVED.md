# PLAN-032 — Integración con Mastodon: leer menciones y contestarlas

- **Issue:** #389 (`atareao/valet-ai`)
- **Change OpenSpec:** `mastodon-mentions`
- **Estado:** PROPUESTO. Pendiente de aprobación del spec. No se ha escrito código.

## 1. Objetivo

Conectar una cuenta de Mastodon para que el agente pueda leer sus menciones y contestarlas desde el chat, y para que las menciones nuevas lleguen también a la bandeja de avisos de Valet. La respuesta es una escritura pública y queda siempre detrás de aprobación explícita.

## 2. Decisiones cerradas con el usuario

1. **Autenticación:** token de app OAuth (Bearer personal) con scopes `read:notifications` y `write:statuses`. Sin flujo OAuth en navegador: el token se pega en Ajustes.
2. **Lectura:** herramienta bajo demanda `list_mentions` **y** avisos en la bandeja mediante `NotificationsRepo::insert`. No hay polling ni webhook propio en la v1.
3. **Respuesta:** `reply` con `Permission::ExplicitApproval`.
4. `mastodon_base_url` no es sensible y **no tiene valor por defecto**: se exige URL explícita para no enviar el token a una instancia equivocada.
5. `mastodon_token` es sensible: `GET /api/settings` lo omite y `PUT /api/settings` lo ignora; única puerta `PUT /api/mastodon/token`.
6. El contenido HTML de los statuses se convierte a texto antes de entrar en el prompt, sin añadir dependencias al proyecto.

## 3. Mapa arquitectónico

| Capa | Fichero | Responsabilidad |
|:---|:---|:---|
| Servicio | `src/services/mastodon.rs` | `Mastodon`, `resolve_config`, `check_connection`, `list_mentions`, `reply`, conversión HTML→texto, mapeo de errores |
| Handler | `src/handlers/mastodon.rs` | `status`, `check`, `update_token`, `CheckResponse` |
| Rutas | `src/routes/mastodon.rs` | `GET /api/mastodon/status`, `GET /api/mastodon/check`, `PUT /api/mastodon/token` |
| Tool | `src/tools/mastodon.rs` | `MastodonTool` multioperación; `list_mentions` (`NoConfirm`) y `reply` (`ExplicitApproval`); fail-closed en operación desconocida |
| Registro | `src/lib.rs` | registrar tool y merger de rutas; `SENSITIVE_KEYS` en `src/db/repos/settings.rs` |
| Skill | `src/orchestrator/skills.rs` | `Skill::Mastodon`, `prompt_key`, `prompt_heading`, `threshold`; catálogo 15 skills / 36 tools |
| Persistencia | `migrations/20261128000001_mastodon.sql` | ajustes, prompt y tabla `mastodon_seen` (idempotente, guarda byte a byte del prompt) |
| Avisos | `src/db/repos/notifications.rs` | `NotificationsRepo::insert` con `origin="mastodon"`; dedupe por `mastodon_seen` |
| Frontend | `src/api/mastodon.ts`, `src/hooks/useMastodon.ts`, `src/components/settings/MastodonSection.tsx` | sub-pestaña de configuración y sonda; **fuera** de `api/client.ts` para no engordar el bundle inicial |

## 4. Contrato técnico

### Endpoints

- `GET /api/mastodon/status` → `{configured, base_url}`; nunca devuelve el token.
- `GET /api/mastodon/check` → siempre `200` con `{ok, detail}`. Sondea `GET {base_url}/api/v1/notifications?limit=1&types[]=mention` con Bearer; sin token o sin URL devuelve `{ok:false, ...}`.
- `PUT /api/mastodon/token` → `{token}`, `400` si vacío, `204` si se guarda.

### Herramienta `mastodon`

- `list_mentions` — parámetros `limit` (1..40, def. 20) y `max_id` opcional. Llama a `GET /api/v1/notifications?types[]=mention`. Por cada notificación nueva (INSERT OR IGNORE en `mastodon_seen`) crea un aviso `origin="mastodon"`, `level="info"`, `author=acct`, `message` en texto plano y `metadata` con `notification_id`, `status_id` y `url`. Devuelve las menciones con `notification_id`, `status_id`, `author`, `text`, `visibility` y `url`.
- `reply` — parámetros `status_id`, `text`, `visibility` (`public|unlisted|private|direct`, def. `unlisted`) y `spoiler_text` opcional. Publica `POST /api/v1/statuses` con `in_reply_to_id` y `Idempotency-Key` UUID v4 fresco por petición. Si el estado original se registró con visibilidad `direct`, se fuerza `direct` aunque se pida otra. Operación desconocida → error, nunca ejecuta red.
- Errores upstream: `401/403` → `Unauthorized`, `404` → `NotFound`, `422` → `InvalidInput`, `429` → `RateLimited`, `5xx`/red → `Upstream`.

### Conversión HTML→texto

Reemplazar `<br>` y cierres de bloque por saltos de línea, eliminar etiquetas conservando el texto de los enlaces, decodificar entidades habituales (`&quot;`, `&amp;`, `&lt;`, `&gt;`, `&#39;`, `&nbsp;`), colapsar espacios y recortar a un máximo razonable. Sin crates nuevas.

## 5. Tareas (TDD)

- [ ] 5.1 RED: tests de conversión HTML→texto, mapeo de errores y fail-closed de operación. Verificar que fallan.
- [ ] 5.2 GREEN: implementar `src/services/mastodon.rs` mínimo; `cargo test` y `cargo check` en verde.
- [ ] 5.3 RED: tests de dedupe de avisos y de visibilidad `direct` forzada.
- [ ] 5.4 GREEN: migración `20261128000001_mastodon.sql`, `mastodon_seen` y tool completo; registrar en `lib.rs`.
- [ ] 5.5 RED: tests de handler/API (`status`, `check`, `token`) y de que `GET /api/settings` omite el token.
- [ ] 5.6 GREEN: handler, rutas y `SENSITIVE_KEYS`.
- [ ] 5.7 RED: test de skill (registro, umbral, prompt) y de catálogo 15/36.
- [ ] 5.8 GREEN: skill y prompts de migración con guarda byte a byte.
- [ ] 5.9 RED: test de frontend de la sub-pestaña (token siempre vacío, sonda, estado).
- [ ] 5.10 GREEN: `api/mastodon.ts`, hook lazy y `MastodonSection.tsx`.
- [ ] 5.11 REFACTOR: `cargo fmt --check`, `cargo clippy -- -D warnings`, `npx tsc --noEmit`, lint; revisar presupuesto de bundle (70.00 kB).
- [ ] 5.12 Arquivar el change, crear `docs/mastodon.md`, actualizar `AGENTS.md` y el índice de planes; renombrar este plan a `-ARCHIVED`.

## 6. Criterios de aceptación

- Sin token, la herramienta falla con mensaje accionable y no hace peticiones de red.
- `mastodon_token` nunca aparece en `GET /api/settings`, logs ni mensajes de error.
- Repetir `list_mentions` no crea avisos duplicados.
- `reply` exige aprobación explícita y envía `Idempotency-Key` único.
- `just check-all` verde y bundle inicial ≤ 70.00 kB.

## 7. Fuera de alcance

Publicación de estados nuevos, boosts, favourites, follows, bloqueos, borrado/edición de estados, multimedia, encuestas, polling/webhook de menciones y flujo OAuth en navegador.

## 8. Riesgos y follow-ups

- La fidelidad de la conversión HTML→texto es el punto más frágil: cubrir con casos reales de menciones con enlaces y menciones.
- La tabla `mastodon_seen` crece sin límite: valorar limpieza periódica en un change futuro; no en la v1.
- El catálogo pasa a 15 skills / 36 tools; verificar que el router no degrada la selección y que el bundle no se acerca al umbral.
