# Tasks

## 1. RED (test que falla)

- [x] 1.1 Añadir en `frontend/src/test/SettingsDialog.test.tsx` un test que, con `useTools` mockeado devolviendo varias tools, verifique que las filas están dentro de un contenedor con `overflow-y: auto` y `max-height` no vacía.
- [x] 1.2 Ejecutar `npm test` (en `frontend/`) y confirmar que el test nuevo falla y el resto sigue verde.

## 2. GREEN (implementación mínima)

- [x] 2.1 Envolver la `<List>` de `ToolsTab` en un `<div style={{ maxHeight: "60vh", overflowY: "auto", overflowX: "hidden", paddingRight: 8 }}>`.
- [x] 2.2 Ejecutar `npm test` → todo verde.
- [x] 2.3 Ejecutar `npm run typecheck` → sin errores.

## 3. REFACTOR

- [x] 3.1 `npm run lint` (0 warnings) y, si aplica, extraer el estilo a una constante con nombre.
- [x] 3.2 Re-ejecutar `npm test` para descartar regresiones.

## 4. Cierre

- [x] 4.1 Revisión (`react-reviewer`).
- [x] 4.2 PR a `development`; tras el merge, PR de archivado (`openspec archive frontend-tools-scroll`).
