# PLAN-031 — Ingesta de notificaciones: aceptar webhooks genéricos (envelope de OpenObserve)

Issue: [#374](https://github.com/atareao/valet-ai/issues/374)
Change OpenSpec: `notification-ingest-envelope`

## 1. Contexto

OpenObserve entrega alertas por webhook HTTP con un envelope fijo (`title`, `body_markdown`, `severity`, …) que **no incluye `message`**, el campo obligatorio de `POST /api/notifications`. Al conectar la alerta `traefik_high_403_429` (picos 403/429 en Traefik) en `co1` la entrega llegó y Valet respondió `400 {"error":"message is required"}`. OpenObserve no puede emitir el esquema de Valet, así que la puerta de ingesta debe aceptar el envelope genérico.

## 2. Alcance

- **Derivar `message` de `title`** cuando falte, sin romper el contrato actual (un `message` presente sigue mandando).
- **Aceptar `severity`** como fuente del nivel cuando falte `level` (`critical`/`warning`/`info`; otro valor cae a `info`).
- **Anexar `body_markdown`** a `metadata["body_markdown"]` sin pisar una clave ya presente.
- **Ignorar** el resto del envelope (`version`, `fields`, `rows`, `links`, `context`, `chart_url`).
- **Actualizar** `docs/api-notifications.md`.

## 3. Tareas

- [ ] 3.1 Change `notification-ingest-envelope` (proposal, design, tasks, delta de `notifications`) validado y aprobado.
- [ ] 3.2 TDD RED → GREEN → REFACTOR en `src/routes/notifications.rs` y `tests/api/notifications.rs` (fases 1-3 de `tasks.md`).
- [ ] 3.3 `just fmt`, `just clippy`, `just test` y `openspec validate --all` en verde.
- [ ] 3.4 PR a `development` con CI verde.
- [ ] 3.5 `openspec archive notification-ingest-envelope`, renombrar este plan a `-ARCHIVED`, y cerrar el issue #374.

## 4. Fuera de alcance

- Autenticación, endpoints y push WebPush (sin cambios).
- Interpretar el resto del envelope de OpenObserve.
- Severidades fuera de la lista blanca (`error`, `debug`, …).
