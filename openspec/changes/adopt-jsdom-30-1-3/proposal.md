# Change: Adoptar jsdom 30.1.3 y alinear los tests con su comportamiento correcto

## Why

Dependabot propone `jsdom` 30.1.1 → 30.1.3 (PR #248). El bump no es neutral: los changelogs oficiales de 30.1.2 y 30.1.3 **corrigen dos bugs de jsdom** de los que dependían dos tests del frontend.

- **30.1.2** — «Fixed `element.focus()` incorrectly focusing elements hidden by 'display'»: `frontend/src/test/SettingsDialog.test.tsx` tecleaba en el panel **oculto** «Archivist Prompt» (sub-pestañas de «Prompts») y solo «pasaba» porque jsdom enfocaba por error elementos ocultos por `display`.
- **30.1.3** — «Fixed `getComputedStyle()` returning unresolved or inconsistent shorthand values, including for 'background', 'border', …»: `frontend/src/test/CalendarView.test.tsx` asertaba el shorthand `background`, que jsdom ahora resuelve de forma consistente (longhand `background-color`).

Ambos fallos están **reproducidos en local**: con jsdom 30.1.1 los dos ficheros pasan siempre (3/3 ejecuciones); con jsdom 30.1.3 fallan de forma determinista (3/3). Como lo que cambia es jsdom volviéndose **más correcto**, lo procedente es adoptarlo y arreglar los tests, no quedarse en la versión antigua.

## What Changes

- **MODIFIED** dependencia de desarrollo: `jsdom` pasa a `^30.1.3` en `frontend/package.json` y su `frontend/package-lock.json`.
- **MODIFIED** `frontend/src/test/SettingsDialog.test.tsx`: el test `saves the three prompts together via updateSettings` activa la sub-pestaña correspondiente antes de escribir en «Archivist Prompt» y «Collapse Prompt», en lugar de operar sobre paneles ocultos.
- **MODIFIED** `frontend/src/test/CalendarView.test.tsx`: la aserción de color del dot de categoría usa el longhand `background-color`, no el shorthand `background`.
- **Sin cambios** de código de producción ni de comportamiento observable.

## Capabilities

- **New Capabilities**: (ninguna)
- **Modified Capabilities**: (ninguna) — el change no altera comportamiento observable; es un bump de dependencia de desarrollo con adaptación de tests. Se marca `skip_specs: true` en `.openspec.yaml`. No se inventa un requirement solo para satisfacer la validación.

## Impact

- **Frontend (solo test y entorno)**: `frontend/package.json`, `frontend/package-lock.json`, `frontend/src/test/SettingsDialog.test.tsx`, `frontend/src/test/CalendarView.test.tsx`.
- **Sin cambios de backend, de esquema ni de base de datos.**
- **Fuera de alcance**: cualquier otra dependencia, `docker-compose.prod.yml` y producción.
