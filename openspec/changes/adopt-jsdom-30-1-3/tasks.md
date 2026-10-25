# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — reproducir el fallo con jsdom 30.1.3

- [x] 1.1 Subir `jsdom` a `^30.1.3` en `frontend` (instalación) y confirmar que `SettingsDialog.test.tsx` y `CalendarView.test.tsx` fallan con los tests actuales (sin tocarlos).
- [x] 1.2 Registrar el fallo exacto de cada uno (mensaje y línea).

## 2. GREEN — adaptar los dos tests

- [x] 2.1 `SettingsDialog.test.tsx`: activar la sub-pestaña («Archivist», «Collapse») antes de escribir en cada prompt.
- [x] 2.2 `CalendarView.test.tsx`: asertar `background-color` en lugar de `background`.
- [x] 2.3 `npx vitest run` en verde (suite completa).

## 3. REFACTOR

- [x] 3.1 `npx tsc --noEmit` y `npm run lint` limpios.
- [x] 3.2 Confirmar que no queda ningún otro punto de la suite que dependa de los bugs de jsdom.

## 4. Verificación y cierre

- [x] 4.1 `openspec validate adopt-jsdom-30-1-3 --strict`.
- [x] 4.2 `just check-spec`.
- [ ] 4.3 PR a `development` con CI verde.
- [ ] 4.4 Cerrar el PR de Dependabot #248 como *superseded* y archivar el change.
