# Tasks: settings-dialog-layout

## Bloque 1 — RED (frontend)
- [x] 1.1 Test: la pestaña «Generación» muestra las cuatro sub-pestañas Chat, Colapso, Fichas y
  Consolidación (falla hoy: no existen).
- [x] 1.2 Test: al cambiar de sub-pestaña se ven los campos del rol correspondiente y se ocultan los
  del anterior.
- [x] 1.3 Test: el ancho del diálogo es ≥ 860 px (falla hoy: `width={600}`).
- [x] 1.4 Verificar por CLI: `npx vitest run` → los nuevos en rojo y los 172 legacy en verde.

## Bloque 2 — GREEN (frontend)
- [x] 2.1 Sustituir los cuatro bloques verticales de «Generación» por un `Tabs` anidado, una
  sub-pestaña por rol, con `forceRender` y los mismos tres campos por rol.
- [x] 2.2 Subir el ancho del `Modal` de ajustes a 900 px.
- [x] 2.3 Verificar por CLI: `npx vitest run`, `npx tsc --noEmit`, `npm run lint:ci`.

## Bloque 3 — REFACTOR y revisión
- [x] 3.1 Revisión con `react-reviewer` (sin blockers).
- [ ] 3.2 Verificación final por CLI y PR a `development`.
