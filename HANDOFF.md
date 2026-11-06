# HANDOFF

**Fecha:** 2026-11-06

## Estado

Change `tool-usage-stats` **mergeado y desplegado**. El arreglo del registro de `tool_calls` en `llm_requests` entró por PR #323 a `development` (`9765256`) y se publicó como release **`v0.31.1`** (PR #324 a `main`, merge `3d529ac`). La imagen se construyó y publicó con el workflow `image.yml` (run `42280704435`, success) y el stack de `co1` se recreó con la nueva imagen: `https://valet.territoriolinux.es/api/health` → `200` con `"version":"0.31.1"`. `development` quedó sincronizado con `main` (fast-forward a `3d529ac`).

## Release v0.31.1

- **PR #323** ✅ — mergeado a `development` con CI verde (Backend Rust + Frontend Node). Hash de merge: `9765256f5f8eb06428f54ca36c32358ebfafa566`.
- **Release branch** `release/v0.31.1`: `vampus upgrade --patch` dejó `0.31.1` en `Cargo.toml`, `.vampus.yml` y `Cargo.lock`; commit `🔧 chore(release): v0.31.1` (`936b20b`).
- **PR #324** ✅ — CI verde (Backend Rust + Frontend Node), mergeado a `main`. Hash de merge: `3d529ac049193b2472757243838f2cf019dfe0fe`.
- **Tag `v0.31.1`** ✅ — publicado; workflow `image.yml` (run `42280704435`) **success**.
- **Despliegue `co1`** ✅ — `docker compose pull valet && docker compose up -d valet`; health: `{"db":"connected","status":"ok","version":"0.31.1"}`.
- **Sincronización** ✅ — `development` fast-forward a `3d529ac` y empujado.

## Tareas pendientes

- **Verificación funcional del arreglo** en producción (que `llm_requests.tool_calls` se rellene en turnos reales). **A cargo del usuario.**
- **4.10 / 5.3** — verificación manual/visual de las vistas de la etapa A del frontend (React 19). **A cargo del usuario.**

## Hallazgos / notas (no bloqueantes)

- Bundle inicial del frontend al límite: 69.99 kB / 70.00 kB (headroom ~1.7 %). No se sube el límite.
- `dependency-stack.test.ts` solo cubre la etapa A del frontend (candidato a endurecer, sin tarea asignada).

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development` salvo merges de PR.
- `docker-compose.prod.yml` no se toca.
- No relajar aserciones de tests.
