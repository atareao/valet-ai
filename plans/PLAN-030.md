# PLAN-030 — La guarda de versiones del frontend cubre por fin la etapa B

Issue: [#368](https://github.com/atareao/valet-ai/issues/368)
Change OpenSpec: `dependency-stack-etapa-b`

## 1. Contexto

`frontend/src/test/dependency-stack.test.ts` es la guarda de contrato sobre `frontend/package.json`. Solo cubre la etapa A del change `frontend-react19-antd6` (antd 6, icons ≥ 6, ausencia del parche de React 19), y su cabecera lo reconoce: la etapa B quedaba fuera «porque estaba pendiente». La etapa B se cerró el 2026-11-06 (v0.31.0) y la guarda nunca se amplió, así que hoy se puede volver a React 18, a `react-leaflet` 4 o bajar `chart.js` con el CI en verde. La spec de `frontend` ya declara esas versiones: lo que falta es su test.

## 2. Alcance

- **Afirmar la etapa B** en el test: `react` y `react-dom` (serie 19), `@types/react` y `@types/react-dom` (serie 19), `react-leaflet` (serie 5) y `chart.js` (major 4, minor ≥ 6).
- **Conservar** las tres aserciones de la etapa A y renombrar el `describe`, que deja de describir solo la etapa A.
- **Verificar en rojo** que la guarda detecta una bajada de versión.
- **No tocar** `frontend/package.json`, código de producción, el presupuesto de bundle ni la spec.

## 3. Tareas

- [ ] 3.1 Change `dependency-stack-etapa-b` (proposal, design, tasks) validado y aprobado.
- [ ] 3.2 TDD RED (rojo verificado bajando versiones) → GREEN → REFACTOR.
- [ ] 3.3 `npx vitest run` y `npx tsc --noEmit` en verde; `just check-all` en verde.
- [ ] 3.4 PR a `development` con CI verde.
- [ ] 3.5 `openspec archive dependency-stack-etapa-b`, fila en el índice de `plans/`, `docs/dependency-stack-guard.md` y issue #368 cerrado.

## 4. Fuera de alcance

- Los bumps de la cola de Dependabot (issue #367, change `deps-queue-2026-11`).
- Afirmar en la guarda la cadena de herramientas (`jsdom`, `vitest`, `vite`, `eslint`).
- Que la spec nombre al guardián: sería un change aparte.