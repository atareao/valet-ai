# HANDOFF

**Fecha:** 2026-11-28

## Estado

**COMPLETADO.** Integración de Mastodon implantada, liberada como **v0.39.0** y desplegada en `co1`. PR **#390** (feature → `development`) y PR **#391** (release → `main`) mergeadas; tag **v0.39.0** publicado; issue **#389** cerrado. `development` y `main` apuntan a `4efb38a`.

## Verificación

- CI de #390 y #391 en verde (Backend y Frontend).
- `just check-all`: Rust 2327 passed / 0 failed / 1 ignored; Vitest 753 passed; lint y `tsc` limpios; bundle inicial 68.23/70.00 kB.
- Imagen `ghcr.io/atareao/valet-ai:v0.39.0` publicada por `image.yml`; stack de Dockge recreado en `co1`.
- `https://valet.territoriolinux.es/api/health` → `{"db":"connected","status":"ok","version":"0.39.0"}`; `/api/events` y `/api/mastodon/status` → 401 sin sesión.
- Auditorías `@rust-reviewer` y `@react-reviewer`: sin bloqueantes.

## Pendiente

1. E2E real: configurar el token OAuth de Mastodon en Ajustes → Integraciones → Mastodon y probar `list_mentions` + `reply` en el chat.
2. Follow-up aparte: filtrar `SENSITIVE_KEYS` en `GET /api/chat/init` (expone tokens sensibles al frontend).
3. Follow-ups menores documentados en `docs/mastodon.md`: crecimiento de `mastodon_seen` y test dedicado del wrapper `LazyApp`.
