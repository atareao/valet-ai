# Change: La guarda de versiones del frontend vigila el stack completo, etapa B incluida

## Why

`frontend/src/test/dependency-stack.test.ts` es la guarda de contrato sobre `frontend/package.json`: impide que alguien baje de serie el stack de UI sin que el CI lo note. Hoy **solo cubre la etapa A** del change `frontend-react19-antd6` —`antd` en la serie 6, `@ant-design/icons` ≥ 6.0.0 y la ausencia de `@ant-design/v5-patch-for-react-19`—, y su propia cabecera lo declara: «No cubre aquí lo que solo corresponde a la etapa B (React 19, react-leaflet 5, chart.js 4.6)». Ese «aquí» se escribió cuando la etapa B estaba pendiente. La etapa B se cerró el 2026-11-06 (v0.31.0) y la guarda **nunca se amplió**.

La consecuencia es concreta: **la spec ya exige lo que la guarda no vigila**. `openspec/specs/frontend/spec.md` («El frontend SHALL declarar React 19 y antd 6 como versiones de su stack de UI») enumera `react` y `react-dom` en la serie `19`, `@types/react` y `@types/react-dom` en la serie `19`, `react-leaflet` en la serie `5` y `chart.js` en la serie `4.6`; el test solo comprueba la mitad. Hoy se puede volver a React 18, a `react-leaflet` 4 o bajar `chart.js` con el CI en verde.

## What Changes

- **MODIFIED** `frontend/src/test/dependency-stack.test.ts`: el `describe` deja de anunciar «etapa A» y pasa a vigilar el stack completo.
- Se añaden aserciones para `react` y `react-dom` (serie **19**), `@types/react` y `@types/react-dom` (serie **19**), `react-leaflet` (serie **5**) y `chart.js` (major **4**, minor ≥ **6**), reutilizando los ayudantes `minVersion`, `major` y `compareVersions` que ya existen en el fichero.
- Se conservan **intactas** las tres aserciones de la etapa A.
- **Sin cambios** en `frontend/package.json`, en código de producción, en el presupuesto de bundle ni en ninguna spec.

## Capabilities

- **New Capabilities**: (ninguna)
- **Modified Capabilities**: (ninguna) — el requirement ya existe en `openspec/specs/frontend/spec.md` y **no cambia**: lo que faltaba era su test. Se marca `skip_specs: true` en `.openspec.yaml`. No se inventa un requirement solo para satisfacer la validación.

## Impact

- **Frontend (solo test)**: `frontend/src/test/dependency-stack.test.ts`.
- **Sin cambios** en `frontend/package.json`, `frontend/package-lock.json`, código de producción, API, esquema ni base de datos.
- **Sistemas**: job `frontend` del CI.
- **Fuera de alcance**: los bumps de la cola de Dependabot (issue #367, change `deps-queue-2026-11`) y cualquier dependencia fuera del stack de UI.