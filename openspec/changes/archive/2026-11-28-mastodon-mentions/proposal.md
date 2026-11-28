# Change: mastodon-mentions

## Why

Valet ya responde comentarios en el blog y crea issues en GitHub, pero no puede interactuar con Mastodon, donde el usuario recibe menciones. Hoy para contestar hay que salir del chat, leer la notificación y responder en la web. El usuario quiere que Valet lea sus menciones, las lleve a la bandeja de avisos y pueda contestarlas desde la conversación, con aprobación explícita por tratarse de una publicación pública. Issue [#389](https://github.com/atareao/valet-ai/issues/389).

## What Changes

- **Servicio `Mastodon`** (`src/services/mastodon.rs`): resuelve la configuración en cada llamada desde `settings`, usa `reqwest` con `Authorization: Bearer`, exige `mastodon_base_url` explícita y llama a `GET /api/v1/notifications?types[]=mention` y `POST /api/v1/statuses`. Convierte el HTML de los statuses a texto sin dependencias nuevas.
- **Herramienta `mastodon`** (`src/tools/mastodon.rs`): multioperación con `list_mentions` y `reply`. `list_mentions` es `NoConfirm`, `reply` es `ExplicitApproval` y cualquier operación desconocida falla cerrado sin tocar la red.
- **Avisos**: `list_mentions` inserta cada mención nueva en la bandeja con `NotificationsRepo::insert`, deduplicada por `mastodon_seen` para que repetir la consulta no duplique avisos.
- **API de integración**: `GET /api/mastodon/status`, `GET /api/mastodon/check` (siempre `200` con `{ok, detail}`) y `PUT /api/mastodon/token`. `mastodon_token` entra en `SENSITIVE_KEYS`.
- **Ajustes**: `mastodon_base_url` (no sensible, sin valor por defecto) y `mastodon_token` (sensible).
- **Skill `mastodon`** en el catálogo cerrado, con `SKILL_MASTODON_PROMPT`. El catálogo pasa de catorce a quince skills y el registry de treinta y cinco a treinta y seis herramientas.
- **Migración** aditiva e idempotente que siembra los ajustes y el prompt (con guarda byte a byte) y crea `mastodon_seen`.
- **Frontend**: sub-pestaña «Mastodon» en Ajustes → Integraciones, con cliente en `src/api/mastodon.ts` y hook lazy, sin tocar `src/api/client.ts`.

## Capabilities

### Nueva: `mastodon-api`
Contrato de ajustes sensibles, endpoints de configuración y sonda real.

### Nueva: `tools/mastodon`
Contrato de la herramienta multioperación, dedupe de avisos, conversión de contenido y permisos.

### Modificada: `frontend`
Añade la sub-pestaña de configuración de Mastodon.

## Impact

- `src/lib.rs` registra la tool y las rutas nuevas.
- `src/db/repos/settings.rs` añade `mastodon_token` a `SENSITIVE_KEYS`.
- `src/orchestrator/skills.rs` añade la skill; el prompt vive en la migración.
- `tests/api/mastodon.rs`, `tests/db/mastodon_migration.rs` y tests unitarios nuevos.
- Sin cambios en el presupuesto de bundle: las llamadas al frontend van en un módulo lazy.

## Fuera de alcance

Publicación de estados nuevos, boosts, favourites, follows, bloqueos, borrado o edición de estados, multimedia, encuestas, polling/webhook, flujo OAuth en navegador y migración del histórico de menciones.
