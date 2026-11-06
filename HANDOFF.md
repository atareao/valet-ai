# HANDOFF

**Fecha:** 2026-11-06

## Estado

La etapa A del change `frontend-react19-antd6` (antd 5 → 6 sobre React 18) está **cerrada, desplegada y verificada** como **v0.30.0**. PR #318 en `development` y PR #319 a `main` mergeados, tag `v0.30.0` e imagen `ghcr.io/atareao/valet-ai:v0.30.0` publicados; el stack `valet` de `co1` recreado desde Dockge y `/api/health` devuelve `{"version":"0.30.0"}`.

## Tarea pendiente

Etapa B del mismo change: **React 18 → 19**, `react-leaflet` 5 y `chart.js` 4.6.1.

Tareas en `openspec/changes/frontend-react19-antd6/tasks.md`:

- **4.1 a 4.12** — rama, subida de versiones, `React.JSX.Element`, migración de leaflet y chart.js, limpieza de APIs retiradas, lint/test/check, docs, auditoría y PR.
- **5.1 a 5.3** — release `v0.31.0`, despliegue en `co1` y verificación visual en producción.
- **6.1 a 6.3** — `just check-all` y `openspec validate --all --strict`, y cierre de los PRs de Dependabot absorbidos.
- **7.1 a 7.3** — archive del change, renombrado a `plans/PLAN-019-COMPLETED.md` y entrada de decisión cerrada en `AGENTS.md`.

## Punto de partida verificado

- Rama a crear: `feature/frontend-react19` **desde `development`** (que ya trae antd 6 y el release v0.30.0).
- `just check-all` verde en el punto de partida.
- Versiones actuales: antd 6.7.0, `@ant-design/icons` ^6.0.0, React 18, `react-leaflet` 4.2.1, `chart.js` 4.5.1.

## Trampas conocidas

- (a) `List`/`Listy` ya está migrado: no rehacerlo.
- (b) `Descriptions` conserva `bordered` — no existe `variant`.
- (c) La puerta `npm run lint:antd` (`frontend/scripts/check-antd-deprecations.mjs`) ya está en `just frontend-lint` y en el job `Frontend (Node)` del CI, y solo falla por `deprecated`.
- (d) El doble no-op de `ResizeObserver` vive en `frontend/src/test/setup.ts`.
- (e) `frontend/src/theme.ts` declara tokens que antd **no** valida en tiempo de tipos: si algo cambia de color, es un token renombrado, no un error de compilación.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development` y commits con gitmoji.
- `docker-compose.prod.yml` no se toca.
- No relajar aserciones de tests.
