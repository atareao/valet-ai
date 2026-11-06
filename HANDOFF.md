# HANDOFF

**Fecha:** 2026-11-06

## Estado

Change `frontend-react19-antd6` **cerrado y archivado**. Etapa B del frontend (React 18 → 19) mergeada (PR #321), release **`v0.31.0` publicada y desplegada en `co1`** (`https://valet.territoriolinux.es/api/health` → `200` con `"version":"0.31.0"`). El archive fusionó los deltas en `openspec/specs/frontend/spec.md` y movió el change a `openspec/changes/archive/2026-11-06-frontend-react19-antd6/`; **no queda ningún change activo**. Plan renombrado a `plans/PLAN-019-COMPLETED.md` y decisión cerrada registrada como entrada **19** de `AGENTS.md`. La etapa A (antd 6, v0.30.0) sigue desplegada y verde.

## Tareas completadas (etapa B)

- **4.1–4.9** ✅ — rama + baseline verde, subida de versiones, `React.JSX.Element`, react-leaflet 5, chart.js 4.6.1, sin APIs retiradas, `frontend-lint`/`test`/`check` en verde (678 tests), README e índice de `AGENTS.md`.
- **4.11** ✅ — auditoría `@react-reviewer`: **0 bloqueantes**.
- **4.12** ✅ — PR #321 mergeado a `development` (`84f38aa`) con CI verde.

## Tareas completadas (release etapa B)

- **5.1** ✅ — release `v0.31.0`: PR #322 mergeado a `main` (`803d52d`), tag `v0.31.0` publicado; workflow `image.yml` completado.
- **5.2** ✅ — desplegada en `co1` y verificada: `curl -s https://valet.territoriolinux.es/api/health` → `200` con `"version":"0.31.0"`.

## Verificación de integración (6.1–6.2)

- **6.1** ✅ — `just check-all` (agrupa `fmt clippy test frontend-lint frontend-test frontend-check`) sobre `development`: **exit 0**. Rust `2083` tests pasan / `0` fallan; frontend Vitest `678 passed (77 files)`; `check-antd-deprecations` `161` ficheros / `0` deprecaciones; bundle inicial `69.99 kB / 70.00 kB` OK.
- **6.2** ✅ — `openspec validate --all --strict`: **exit 0**, `Totals: 63 passed, 0 failed (63 items)`.

## Tareas de cierre (7.1–7.3)

- **6.3** ✅ — PRs de Dependabot `#280`, `#281`, `#284`, `#310` y `#311` **cerrados**; sus versiones absorbidas en `frontend/package.json` (react 19.3.1, react-dom 19.3.1, react-leaflet 5.0.0, chart.js 4.6.1, antd 6.7.0).
- **7.1** ✅ — `openspec archive frontend-react19-antd6 --yes`: éxito, `+3 added` en `openspec/specs/frontend/spec.md`; change archivado como `2026-11-06-frontend-react19-antd6`.
- **7.2** ✅ — `plans/PLAN-019.md` → `plans/PLAN-019-COMPLETED.md` (el índice de `AGENTS.md` ya apuntaba al nombre completado).
- **7.3** ✅ — entrada **19** añadida a la sección V de `AGENTS.md`.

## Tareas pendientes

Solo las del usuario:

- **4.10** — verificación manual en local de las vistas de la etapa A (sin cambios visuales, sin avisos en consola). **A cargo del usuario.**
- **5.3** — verificación visual en producción de las mismas vistas (idénticas a la etapa A). **A cargo del usuario.**

## Hallazgos / notas (no bloqueantes)

- **`react-leaflet` 5 tuvo que subir junto con React 19** en la tarea 4.2: su peer `react ^18` rompía `npm ci`.
- **Bundle inicial al límite:** 69.99 kB / 70.00 kB (headroom ~1.7 %). `vendor-react` creció con React 19.
- **`AGENTS.md` apunta a `plans/PLAN-019-COMPLETED.md`**, ya existente tras el rename de la tarea 7.2.
- **`dependency-stack.test.ts` solo cubre la etapa A** (su comentario anticipa las asserts de la etapa B). Candidato a endurecer, **sin tarea asignada**; no se hizo para no ampliar alcance.
- Fix de timing `Suspense`/`lazy` en React 19 en 4 ficheros de test (aserciones síncronas → asíncronas positivas, sin relajar nada) + un flaky preexistente en `useEvents.test.ts`.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development` y commits con gitmoji.
- `docker-compose.prod.yml` no se toca.
- No relajar aserciones de tests.
