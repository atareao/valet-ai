# Proposal

## Why

Cualquier visita sin sesión —login, ventana de incógnito, primera carga— descarga el SPA completo: `App.tsx` importa de forma estática `AppLayout` y, con él, todas las vistas (`ChatView`, `CalendarView`, `TaskView`, `StatsDashboard`, `SettingsDialog`), `antd`, `@ant-design/icons`, `chart.js` y `react-markdown`. Con `base:''` y servido por el backend, la pantalla de login —que solo necesita un logo y un botón— arrastra ~1,9 MB de JS (≈565 kB gzip) y el build avisa de chunks >500 kB. El aviso no es accionable mientras todo comparta un único bundle de entrada, y el coste real lo paga el usuario en cada carga de login.

## What Changes

- El **bundle inicial** deja de incluir `antd`, `@ant-design/icons`, `react-router-dom`, las vistas autenticadas y las librerías de gráficas/markdown/leaflet.
- La **aplicación autenticada** (`BrowserRouter` + `ConfigProvider`/`AntdApp` + `ProfileProvider` + `Routes` + `AppLayout`) pasa a un módulo cargado con `React.lazy` + `Suspense` cuando la sesión se resuelve.
- La **pantalla de login** usa un botón nativo (sin `antd`) y el **estado de carga** un spinner nativo, ambos definidos en `global.css`; se conserva el `data-testid="auth-loading"`.
- Dentro de `AppLayout`, las vistas que se abren bajo demanda (**Agenda, Tareas, Stats, Ajustes**) se cargan con `React.lazy`; el chat (vista por defecto) sigue estático.
- `vite.config.ts`: `build.manifest` habilitado, vendors separados y `chunkSizeWarningLimit` ajustado al vendor de `antd` (intencionadamente grande y cacheable por separado).
- Nueva comprobación determinista del bundle inicial (`frontend/scripts/check-initial-bundle.mjs`) que corre en `postbuild` y, por tanto, en `just check-all` y en CI: el grafo inicial no contiene los vendors prohibidos, supera el presupuesto de gzip y existe realmente un chunk diferido de la app autenticada.
- Sin cambios de comportamiento: flujo OIDC, logout y contrato de la API intactos.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `frontend`: se añade el requisito "El frontend SHALL cargar la aplicación autenticada de forma diferida", que fija el contenido máximo del bundle inicial y la carga bajo demanda de la aplicación y de las vistas del `AppLayout`.

## Impact

- `frontend/src/App.tsx`, nuevo `frontend/src/AuthenticatedApp.tsx`, nuevo `frontend/src/components/AppLoader.tsx`, `frontend/src/components/LoginPage.tsx`, `frontend/src/components/AppLayout.tsx`, `frontend/src/global.css`.
- `frontend/vite.config.ts`, `frontend/package.json`, nuevo `frontend/scripts/check-initial-bundle.mjs`.
- Nuevo `frontend/src/components/ErrorBoundary.tsx` (aviso recuperable si falla un chunk diferido) y `Dockerfile` (no servir `dist/.vite/manifest.json`).
- Tests: `frontend/src/test/App.guard.test.tsx`, `frontend/src/test/LoginPage.test.tsx` y nuevo `frontend/src/test/AppLoader.test.tsx`.
- Sin cambios en backend, `docker-compose*.yml` ni en el flujo OIDC.
