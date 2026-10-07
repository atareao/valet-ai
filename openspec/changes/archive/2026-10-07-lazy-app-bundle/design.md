# Design

## Contexto

- Entrada actual (`src/main.tsx` → `src/App.tsx`): `App` importa estáticamente `AppLayout`, `LoginPage`, `ProfileProvider`, `AuthProvider`, `antd` (`ConfigProvider`, `App`, `Spin`) y `theme` (que importa `antd`). `AppLayout` importa estáticamente `ChatView`, `SettingsDialog`, `CalendarView`, `TaskView` y `StatsDashboard`.
- Único `lazy()` existente: `LocationWidget` (`src/components/widgets/registry.ts`).
- Build actual: `vendor-antd` 1,2 MB (372 kB gzip) e `index` 461 kB (147 kB gzip); total inicial ≈565 kB gzip.
- Convención del proyecto para `lazy`: `lazy(() => import("./X").then((m) => ({ default: m.X })))` (patrón ya usado en `registry.ts`).

## Enfoque

### Grafo de entrada

Antes: `main → App → { AppLayout → todas las vistas, LoginPage, antd, router, theme }`.

Después:

```
main → App (AuthProvider + AuthGate)
                ├─ loading  → AppLoader            (nativo, sin antd)
                ├─ sin sesión → LoginPage          (botón nativo, sin antd)
                └─ con sesión → Suspense → lazy AuthenticatedApp
                                             └─ BrowserRouter + ConfigProvider(valetTheme)
                                                + AntdApp + ProfileProvider + Routes → AppLayout
```

- `AuthenticatedApp` concentra `react-router-dom`, `antd`, `theme` y `ProfileProvider`, y se exporta como `default` para `React.lazy`.
- `LoginPage` sustituye el `Button` de antd por un `<button>` con la clase `.login-button` (`global.css`), manteniendo `window.location.assign("/api/auth/login")` y el layout ya especificado (fondo `#000000`, logo 120 px).
- `AppLoader` (nuevo) es un spinner CSS nativo con `role="status"`, `aria-label="Cargando"` y `data-testid="auth-loading"` (conserva el contrato del requisito "Estado de carga mientras se resuelve la sesión"). Se usa tanto para la resolución de sesión como para el fallback del `Suspense`.
- Dentro de `AppLayout`, `StatsDashboard`, `CalendarView`, `TaskView` y `SettingsDialog` se cargan con `React.lazy` envueltos en un `Suspense` por modal (antd ya está disponible aquí, así que el fallback puede ser un `Spin` de antd). `ChatView` permanece estático por ser la vista por defecto. Como los `Modal` no montan su contenido si están cerrados, las vistas ni se solicitan hasta que el usuario las abre.

### Chunking

- `build.manifest = true` (Vite 8 / rolldown-vite) para poder inspeccionar el grafo real en la comprobación.
- Vendors separados con nombres estables: `vendor-react`, `vendor-antd`, `vendor-antd-icons`, `vendor-charts` (`chart.js`, `react-chartjs-2`), `vendor-markdown` (`react-markdown`, `remark-gfm`), `vendor-leaflet` (`leaflet`, `react-leaflet`) y `vendor-router` (`react-router-dom`). Se verificará que `manualChunks` se aplica como se espera en rolldown-vite 8; si no, se migrará a `output.advancedChunks`.
- `chunkSizeWarningLimit` se sube por encima del vendor de `antd` —es un vendor deliberado, compartido y cacheable— y se documenta en el propio `vite.config.ts`. El control real de regresión es la comprobación del bundle inicial, no el umbral de aviso.

### Comprobación del bundle inicial (`frontend/scripts/check-initial-bundle.mjs`)

Se ejecuta en `postbuild` (por tanto en `npm run build`, `just frontend-check` y el job de CI), de modo que no hace falta tocar el `.justfile`.

1. Lee `dist/.vite/manifest.json` y parte de la entrada HTML (`isEntry`).
2. Recorre **importaciones estáticas** (`imports`) sumando tamaños gzip reales de los ficheros en `dist/`.
3. Guardas:
   - el grafo inicial **no** contiene ningún chunk cuyo nombre sea `vendor-antd*`, `vendor-charts*`, `vendor-markdown*`, `vendor-leaflet*` ni `AuthenticatedApp*`;
   - el gzip total del grafo inicial **no supera el presupuesto** (≤150 kB);
   - **no vacuidad**: existe un chunk diferido `AuthenticatedApp-*.js` fuera del grafo inicial (si la app no se hubiera separado, la comprobación falla en vez de pasar por vacío).

## Alternativas descartadas

- **Solo subir `chunkSizeWarningLimit`**: silencia el aviso sin reducir lo que descarga el usuario. Se sube el umbral, sí, pero acompañado de la separación real y de la comprobación del bundle inicial.
- **Lazy por ruta con `react-router` `lazy` de loaders**: no aporta aquí; la app tiene una sola ruta (`*`).
- **Lazy de `MessageBubble` a nivel de mensaje**: fuera de alcance y con coste en interacción; el markdown sale del bundle inicial de todos modos al diferir `AuthenticatedApp`.

## Riesgos

- Los tests del guard (`App.guard.test.tsx`) deben esperar a la resolución del `lazy` (`findBy*`), y los mocks de `AppLayout`/vistas deben ofrecer el export que consume el `lazy`.
- `base:''`: el reparto de chunks diferidos ya funciona hoy (`LocationWidget`), por lo que no se introduce riesgo nuevo de rutas relativas.
- `StrictMode`: el doble montaje no altera el comportamiento de `React.lazy`.

## Tests (TDD)

- RED: en `LoginPage.test.tsx`, el control de login es un `<button>` nativo **sin** clase `ant-btn`; existe `AppLoader` con `data-testid="auth-loading"` y **sin** `.ant-spin`; y la comprobación del bundle inicial **falla** contra el build actual (la entrada incluye `vendor-antd` y supera el presupuesto).
- GREEN: refactor de entrada, `AuthenticatedApp`, `AppLoader`, `LoginPage` nativo y `AppLayout` con vistas diferidas; la comprobación del bundle pasa.
- REFACTOR: limpieza de comentarios obsoletos de tests y revisión `react-reviewer`.
