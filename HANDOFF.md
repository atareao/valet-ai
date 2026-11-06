# HANDOFF

**Fecha:** 2026-11-06

## Estado

Etapa B del change `frontend-react19-antd6` (React 18 → 19) **mergeada y publicada**: PR #321 a `development` (`84f38aa`, CI verde) y release `v0.31.0` ya en `main` (PR #322, merge `803d52d`, tag `v0.31.0`). El workflow `image.yml` está **in_progress** publicando `ghcr.io/atareao/valet-ai:v0.31.0` (run [42271432937](https://github.com/atareao/valet-ai/actions/runs/42271432937)). La rama `feature/frontend-react19` lleva React `^19.3.1`, `react-leaflet` 5, `chart.js` 4.6.1 y los ajustes derivados. Queda desplegar en `co1`. La etapa A (antd 6, v0.30.0) sigue desplegada y verde.

## Tareas completadas (etapa B)

- **4.1–4.9** ✅ — rama + baseline verde, subida de versiones, `React.JSX.Element`, react-leaflet 5, chart.js 4.6.1, sin APIs retiradas, `frontend-lint`/`test`/`check` en verde (678 tests), README e índice de `AGENTS.md`.
- **4.11** ✅ — auditoría `@react-reviewer`: **0 bloqueantes**.
- **4.12** ✅ — PR #321 mergeado a `development` (`84f38aa`) con CI verde.

## Tareas completadas (release etapa B)

- **5.1** ✅ — release `v0.31.0`: PR #322 mergeado a `main` (`803d52d`), tag `v0.31.0` publicado; workflow `image.yml` en marcha.

## Tareas pendientes

- **4.10** — verificación manual en local de las vistas de la etapa A (sin cambios visuales, sin avisos en consola). **A cargo del usuario.**
- **5.2** — desplegar `v0.31.0` en `co1` y verificar la versión (`curl -s https://valet.territoriolinux.es/api/health` → `0.31.0`).
- **5.3** — verificación visual en producción de las mismas vistas (idénticas a la etapa A).
- **6.1–6.3** — `just check-all` + `openspec validate --all --strict`; cerrar los PRs de Dependabot absorbidos.
- **7.1–7.3** — archive del change, renombrado a `plans/PLAN-019-COMPLETED.md` y entrada de decisión cerrada en `AGENTS.md`.

## Hallazgos / notas (no bloqueantes)

- **`react-leaflet` 5 tuvo que subir junto con React 19** en la tarea 4.2: su peer `react ^18` rompía `npm ci`.
- **Bundle inicial al límite:** 69.99 kB / 70.00 kB (headroom ~1.7 %). `vendor-react` creció con React 19.
- **`AGENTS.md` ya apunta a `plans/PLAN-019-COMPLETED.md`**, que aún no existe: se crea en el rename de la tarea 7.2.
- **`dependency-stack.test.ts` solo cubre la etapa A** (su comentario anticipa las asserts de la etapa B). Candidato a endurecer, **sin tarea asignada**; no se hizo para no ampliar alcance.
- Fix de timing `Suspense`/`lazy` en React 19 en 4 ficheros de test (aserciones síncronas → asíncronas positivas, sin relajar nada) + un flaky preexistente en `useEvents.test.ts`.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development` y commits con gitmoji.
- `docker-compose.prod.yml` no se toca.
- No relajar aserciones de tests.
