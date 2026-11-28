# Mastodon

## Qué es

Integración de Mastodon en Valet: el agente puede leer las menciones de la cuenta configurada y contestarlas desde el chat. Las menciones nuevas también llegan a la bandeja de avisos. La respuesta es una publicación pública y exige aprobación explícita.

## Ficheros implicados

- `src/services/mastodon.rs` — cliente HTTP, configuración por llamada, conversión HTML→texto, errores tipados.
- `src/tools/mastodon.rs` — herramienta multioperación `mastodon`.
- `src/handlers/mastodon.rs`, `src/routes/mastodon.rs` — endpoints de configuración y sonda.
- `src/orchestrator/skills.rs` — skill `mastodon`; el prompt `SKILL_MASTODON_PROMPT` vive en la migración.
- `migrations/20261128000001_mastodon.sql` — ajustes, prompt y tabla `mastodon_seen`.
- `src/db/repos/settings.rs` — `mastodon_token` en `SENSITIVE_KEYS`.
- `frontend/src/api/mastodon.ts`, `frontend/src/hooks/useMastodon.ts`, `frontend/src/components/settings/MastodonSection.tsx` — configuración en Ajustes.
- `frontend/src/components/LazyApp.tsx` — traslada el guard de sesión al límite `lazy` para mantener el grafo inicial por debajo del presupuesto de bundle.

## Cómo funciona

- `list_mentions(limit, max_id)` consulta `GET /api/v1/notifications?types[]=mention` y devuelve `notification_id`, `status_id`, `author`, `text`, `visibility` y `url`. El HTML del status se convierte a texto conservando enlaces, saltos de línea y entidades.
- Cada notificación nueva se registra en `mastodon_seen` (`INSERT OR IGNORE`) y solo entonces genera un aviso con `origin="mastodon"`, `level="info"` y metadata con los ids. Repetir la consulta no duplica avisos.
- `reply(status_id, text, visibility, spoiler_text)` publica `POST /api/v1/statuses` con `in_reply_to_id` y un `Idempotency-Key` UUID v4 nuevo por petición.
- Si el status original se registró con visibilidad `direct`, la respuesta se fuerza a `direct`.
- Errores: `401/403 → Unauthorized`, `404 → NotFound`, `422 → InvalidInput`, `429 → RateLimited`, `5xx`/red → `Upstream`. Los detalles se sanitizan para no devolver el token.

## Configuración

- `mastodon_base_url`: no sensible, **sin valor por defecto**; se exige URL explícita para no enviar el token a una instancia equivocada.
- `mastodon_token`: sensible; `GET /api/settings` lo omite y `PUT /api/settings` lo ignora. Única puerta: `PUT /api/mastodon/token` (`400` vacío, `204` correcto).
- `GET /api/mastodon/status` → `{configured, base_url}`.
- `GET /api/mastodon/check` → siempre `200` con `{ok, detail}`; sondea `GET /api/v1/notifications?limit=1&types[]=mention`.

## Cómo habilitarlo

1. Crear una aplicación OAuth en la instancia de Mastodon con scopes `read:notifications` y `write:statuses` y copiar el token de acceso de la cuenta.
2. En Valet: Ajustes → Integraciones → Mastodon, pegar la URL base y el token y guardar cada campo.
3. Pulsar «Probar conexión».

No hay flujo OAuth en navegador, polling ni webhook: las menciones se consultan al pedirlo en el chat.

## Decisiones y follow-ups

- La respuesta exige aprobación explícita (`Permission::ExplicitApproval`); `list_mentions` no la exige.
- Sin token, la herramienta falla con un mensaje accionable y no hace peticiones de red.
- El token del frontend arranca siempre vacío.
- **Pendiente:** E2E con un token real contra la instancia del usuario.
- **Pendiente (hallazgo heredado, fuera de alcance):** `GET /api/chat/init` devuelve `settings` sin filtrar `SENSITIVE_KEYS`, por lo que los tokens sensibles —incluido `mastodon_token`— llegan al frontend. Corregir en un change aparte.
- **Pendiente:** `mastodon_seen` crece sin limpieza automática; valorar retención en el futuro.
