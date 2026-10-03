# Tasks

## 1. Baseline

- [x] 1.1 Registrar baseline del frontend: `npm run typecheck`, `npm test` (vitest run), `npm run lint` (passed/warnings).
- [x] 1.2 Localizar los tests que fijan el número de pestañas (`SettingsDialog.test.tsx`: «exactly six top-level tabs») y el ancho del modal (≥860).

## 2. RED

- [x] 2.1 `useTools.test.ts` (mockeando `../api/client`, patrón de `usePersistentMemory.test.ts`): al montar llama a `api.getTools` y expone `tools`/`loading`; `toggle(id)` llama a `api.toggleTool(id)` y actualiza la tool en el estado; si `toggle` falla, expone `error` y conserva el estado previo. Hoy no existe el hook → rojo.
- [x] 2.2 `SettingsDialog.test.tsx` (mockeando `../hooks/useTools`): existe la pestaña superior "Herramientas" y, al abrirla, se listan las tools con nombre, descripción y un `Switch` según `enabled`; con `loading: true` se muestra un indicador y no la lista. Hoy no existe la pestaña → rojo.
- [x] 2.3 `SettingsDialog.test.tsx`: apagar el `Switch` de una tool llama a `toggle(id)`; si `toggle` lanza, se muestra un aviso de error. Rojo.
- [x] 2.4 Actualizar el test «shows exactly six top-level tabs» a **siete**, incluyendo "Herramientas".
- [x] 2.5 Verificar por CLI: 2.1–2.3 en rojo; el resto en verde salvo 2.4 (pasa a exigir siete y falla hasta el GREEN).

## 3. GREEN

- [x] 3.1 `types/index.ts`: `export interface Tool { id: string; name: string; description: string; enabled: boolean }`.
- [x] 3.2 `api/client.ts`: `getTools()` → `GET /api/tools`; `toggleTool(id)` → `PUT /api/tools/{id}/toggle` (tipos `Tool`).
- [x] 3.3 `hooks/useTools.ts`: estado `{ tools, loading, error }` + `toggle(id)` (actualiza la fila con la `Tool` devuelta; en error, conserva el estado y fija `error`).
- [x] 3.4 `SettingsDialog.tsx`: pestaña superior "Herramientas" (antd `List`, `Switch` por tool, indicador de carga, aviso de error con `App.useApp()`) y `width={1000}`.
- [x] 3.5 Tests GREEN: 2.1–2.4 pasan. Verificar por CLI: `npm test` (0 failed) y `npm run typecheck`.

## 4. REFACTOR y cierre

- [x] 4.1 `npm run lint` sin errores ni warnings nuevos; `npm run build` limpio.
- [x] 4.2 Revisión con `react-reviewer`.
- [ ] 4.3 PR a `development`; tras el merge, PR de archivado (`openspec archive settings-tools-tab`).

## 5. Verificación de integración

- [ ] 5.1 En despliegue: abrir Settings → Herramientas, desactivar una tool y comprobar que desaparece del prompt (el registry la oculta) y que al reactivarla vuelve.
