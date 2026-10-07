# Tasks

## 1. Frontend — RED

- [x] 1.1 `LoginPage.test.tsx`: el control de login es un `<button>` nativo (`tagName === "BUTTON"`) y **no** tiene la clase `ant-btn`; se conservan los tests de logo, tamaño, fondo y navegación a `/api/auth/login`.
- [x] 1.2 Nuevo `frontend/src/test/AppLoader.test.tsx`: `AppLoader` renderiza `data-testid="auth-loading"` con `role="status"` y **sin** clase `ant-spin`.
- [x] 1.3 Comprobación del bundle: habilitar `build.manifest` y añadir `frontend/scripts/check-initial-bundle.mjs`, cableado en `postbuild`. Ejecutar `npm run build` → **falla** (el grafo inicial incluye `vendor-antd` y supera el presupuesto).
- [x] 1.4 `npx vitest run` → los tests nuevos fallan y el resto sigue verde (RED).

## 2. Frontend — GREEN

- [x] 2.1 Nuevo `AuthenticatedApp.tsx` (default export) con `BrowserRouter` + `ConfigProvider(valetTheme)` + `AntdApp` + `ProfileProvider` + `Routes` + `AppLayout`.
- [x] 2.2 Nuevo `AppLoader.tsx` (spinner nativo, `data-testid="auth-loading"`, `role="status"`) y estilos `.login-button`/spinner en `global.css`.
- [x] 2.3 `App.tsx`: sin `antd` ni router; `AuthGate` usa `AppLoader` y `React.lazy` + `Suspense` para `AuthenticatedApp`.
- [x] 2.4 `LoginPage.tsx`: botón nativo con `.login-button`, sin `antd`, conservando fondo `#000000`, logo 120 px y navegación a `/api/auth/login`.
- [x] 2.5 `AppLayout.tsx`: vistas `StatsDashboard`, `CalendarView`, `TaskView` y `SettingsDialog` con `React.lazy` + `Suspense` por modal (patrón `.then(m => ({ default: m.X }))`); `ChatView` estático.
- [x] 2.6 `App.guard.test.tsx`: esperar la resolución del `lazy` (`findBy*`) en el caso autenticado.
- [x] 2.7 `vite.config.ts`: vendors separados con `codeSplitting.groups` (`vendor-react`, `vendor-antd-icons`, `vendor-antd`, `vendor-charts`, `vendor-markdown`, `vendor-leaflet`) y `chunkSizeWarningLimit` documentado. Sin grupo para `react-router`: queda dentro del chunk diferido `AuthenticatedApp`.
- [x] 2.8 `npm run build` → la comprobación del bundle inicial pasa; `npx vitest run` verde; `npx tsc --noEmit` y `npm run lint`.
- [x] 2.9 `ErrorBoundary` en las fronteras diferidas (`App.tsx`): si falla la carga de un chunk, aviso recuperable con recarga en vez de pantalla en blanco.
- [x] 2.10 Dockerfile: no servir `dist/.vite/manifest.json` en producción (se elimina tras el build del frontend).

## 3. Revisión y cierre

- [x] 3.1 Revisión `react-reviewer`.
- [x] 3.2 `just check-all` en verde y comprobación manual del reparto de chunks.
- [ ] 3.3 PR a `development` y `openspec archive lazy-app-bundle`.
