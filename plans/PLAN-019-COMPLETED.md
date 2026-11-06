# PLAN-019 — El frontend salta a React 19 y antd 6: dos etapas para no mezclar los fallos

## 1. Motivo

Dependabot mantiene cinco PRs abiertos sobre `frontend/` (`#280`, `#281`, `#284`, `#310`, `#311`). Cuatro de ellos no son actualizaciones independientes sino **una sola migración**: `antd 6` y `react-leaflet 5` exigen React 19, así que `react`, `react-dom` y sus tipos tienen que subir a la vez. Mergearlos por separado, como sugiere Dependabot, deja el árbol en combinaciones de peer dependencies que ningún release puede construir.

El proyecto encadena cuatro releases de widgets seguidos (v0.27.x a v0.29.0) sin haber tocado el frontend de fondo. Esta es la primera migración mayor del stack de UI desde que existe la aplicación, y afecta a 59 importaciones de `antd`, 42 ficheros `*.test.tsx` y al fichero de tema completo.

## 2. Diagnóstico (evidencia medida sobre el estado actual)

**Stack declarado** (`frontend/package.json`):

| Paquete | Hoy | Destino |
|:---|:---|:---|
| `react` / `react-dom` | `^18.3.1` | `^19.x` |
| `@types/react` / `@types/react-dom` | `^18.3.0` | `^19.x` |
| `antd` | `^5.25.0` (lock 5.29.3) | `^6.x` |
| `@ant-design/icons` | `^6.3.4` | `>= 6.0.0` (ya cumple) |
| `react-leaflet` | `^4.2.1` | `^5.x` |
| `chart.js` | `^4.5.1` (lock 4.5.1) | `^4.6.1` |
| `@testing-library/react` | `^16.3.3` | sin cambio (soporta React 19) |
| `vite` / `vitest` / `typescript` | `^8.3.5` / `^5.0.5` / `^5.6.0` | sin cambio |

**Inconsistencia latente que la migración resuelve:** el repo ya declara `@ant-design/icons@^6.3.4` junto a `antd@5`. La documentación de antd 6 dice literalmente que `@ant-design/icons@6` **no es compatible con antd@5** y que ambas deben subir juntas. Hoy conviven sin síntoma visible, pero es una combinación no soportada.

**Alcance real del cambio, medido con `rg` sobre `frontend/src`:**

- 59 importaciones de `antd` / `@ant-design/icons`; ninguna por rutas internas (`antd/es/*`, `antd/lib/*`) ni de `@ant-design/cssinjs`.
- **Sin métodos estáticos de riesgo**: `eslint.config.js:34-64` ya prohíbe importar `message`/`notification` y usar `Modal.confirm|info|success|error|warning|destroy` estáticos, y los 14 componentes que muestran mensajes usan `App.useApp()` bajo el `<AntdApp>` de `AuthenticatedApp.tsx:14`. El patrón que antd 6 exige ya está adoptado.
- **APIs deprecadas en v6 que el repo sí usa** (todas siguen funcionando en v6, con aviso en consola, y desaparecen en v7):
  - `Space` / `Space.Compact` con `direction="vertical"`: `NotificationsView.tsx:63`, `IngestKeysSection.tsx:167`, `CalendarView.tsx:177`, `OpentodoSection.tsx:172`, `JournalView.tsx:288`, `StravaIntegration.tsx:268`, `PersistentMemoryPanel.tsx:109`, `NotificationsTab.tsx:149`… → `orientation`.
  - `Tabs` con `tabPosition="left"`: `SkillsTab.tsx:239`, `IntegrationsTab.tsx:137`, `SettingsDialog.tsx:478`, `:587`, `:702`, `:771` → `tabPlacement`. Hay un test que fija ese atributo literalmente (`test/SettingsDialog.test.tsx:2724-2725`).
  - `Modal` con `destroyOnClose`: `EventModal.tsx:127` → `destroyOnHidden`. Es la única deuda de este tipo: el resto del repo ya usa `destroyOnHidden` (`SettingsDialog.tsx:471`).
  - `Descriptions` con `bordered`: `EventDetail.tsx:60`, `stats/LastApiCallCard.tsx:52` → `variant`.
  - `maskClosable={false}`: `ChatView.tsx:253` → `mask.closable` (6.3.0).
- **Sin** `dropdownRender`, `bodyStyle`, `headStyle`, `overlayClassName`, `TabPane`, `onVisibleChange` ni `filterDropDownVisible`. El `visible={settingsVisible}` de `AppLayout.tsx:273` es la prop propia de `SettingsDialog`, no de antd.

**`theme.ts` — el fichero de mayor riesgo y sin cobertura de tipos:** declara `algorithm: darkAlgorithm`, 8 tokens globales y overrides de `Layout` (`siderBg`, `headerBg`, `bodyBg`), `Menu` (`darkItemBg`, `darkItemSelectedBg`), `Table` (11 claves, incluidas `headerSortActiveBg`, `bodySortBg`, `rowSelectedHoverBg`, `cellFontSize`) y `Card` (`colorBgContainer`). antd 6 reescribe el DOM interno de muchos componentes y activa **variables CSS por defecto**, de modo que un token renombrado no da error: da un cambio de color silencioso.

**Patrones de React que cambian de tipo con `@types/react@19`:**

- `ReactDOM` solo se usa bien: `main.tsx:2,9` con `createRoot`. Sin `render`, `hydrate`, `findDOMNode`, `createFactory`, `propTypes` ni `defaultProps` de componente de función.
- `forwardRef` en un solo sitio (`MessageInput.tsx:1,22`), sigue funcionando: migrarlo es opcional.
- **El namespace global `JSX` desaparece** en los tipos de React 19: único punto afectado, `test/ErrorBoundary.test.tsx:50` (`function Boom(): JSX.Element`).
- 30+ usos de `React.FC`, ninguno con `children` implícito.

**`react-leaflet 5`** exige React 19 y elimina `LeafletProvider` (que el repo no usa). Hay un único consumidor: `widgets/LocationWidget.tsx:3-9` (`MapContainer`, `TileLayer`, `CircleMarker`, `Popup`), con dos mocks (`test/WidgetRenderer.test.tsx:10-22`, `test/LocationWidget.test.tsx:8-21`).

**Suite y CI:** 42 ficheros `*.test.tsx` + 33 `*.test.ts`, `vitest` con `jsdom` y `setupFiles: ['./src/test/setup.ts']` (que solo importa `@testing-library/jest-dom/vitest`; **no hay mock global de `matchMedia`** — se mockea fichero a fichero en ~25 sitios). El CI (`ci.yml:39-69`) y el `Dockerfile:42-56` corren `tsc --noEmit`, `lint:ci`, `vitest run` y `build` sobre Node 22.

**Herramienta disponible:** antd publica `@ant-design/cli` (Node >= 20), con `antd lint <dir>` para enumerar APIs deprecadas, `antd doctor` para diagnosticar el entorno, `antd token <Componente>` para consultar los tokens vigentes de v6 y `antd migrate 5 6` para la lista de migración. Es la fuente de verdad para el barrido, por delante de una lista escrita a mano.

## 3. Objetivo y alcance

**Objetivo:** dejar `frontend/` sobre React 19 + antd 6 + react-leaflet 5 + chart.js 4.6.1, con la suite de tests pasando sin relajar aserciones, el tema oscuro intacto y cero avisos de API deprecada de antd.

**Dentro del alcance:** dependencias del frontend, barrido de APIs deprecadas de antd, adaptación de tipos a React 19, actualización de los tests que fijan atributos que cambian de nombre, `README.md` (dice «React 18 + Ant Design 5») y el índice de planes de `AGENTS.md`.

**Fuera del alcance:** cualquier cambio de comportamiento o de diseño visual; el backend Rust; `docker-compose.prod.yml` (aplazado por decisión del usuario); la poda del `AGENTS.md` del repo (PR #288 cerrado, se rehará por separado).

**Estrategia en dos etapas, y por qué:**

antd 6 exige React **>= 18**, no 19. Eso permite separar la migración en dos PRs independientes y reversibles:

- **Etapa A — antd 6 con React 18.** Aísla los fallos propios de antd (tokens renombrados, cambio de DOM, orden de carga de CSS) sin que React 19 pueda ser el culpable.
- **Etapa B — React 19.** Con antd ya asentado, solo queda el salto de React, sus tipos y las dos librerías que lo exigen (`react-leaflet 5`, y `chart.js 4.6.1` como cola).

Cada etapa tiene su propio PR a `development` y su propio release, de modo que un fallo visual en producción se pueda atribuir a una sola causa. La alternativa de un único PR de las cinco dependencias se descarta: mezcla dos causas raíz distintas en el mismo diff y hace el rollback ambiguo.

## 4. Diseño

1. **Enumerar antes de tocar.** `npx @ant-design/cli lint frontend/src` y `npx @ant-design/cli token <Componente>` para cada token de `theme.ts`, antes y después de subir antd, para comparar la lista vigente de nombres. La comparación es lo que demuestra que ningún token quedó huérfano.
2. **Etapa A (antd 5 → 6):** subir `antd` y `@ant-design/icons` juntos en el mismo commit; barrer las APIs deprecadas que v6 introduce (`orientation`, `tabPlacement`, `destroyOnHidden`, `variant`, `mask.closable`, enumerado de `size`); verificar `theme.ts` token a token; ajustar los tests que fijan `tabPosition` y el margen implícito de `Tag` (v6 lo elimina).
3. **Etapa B (React 18 → 19):** subir `react`, `react-dom` y sus tipos a la vez (los tipos de v19 van acoplados al runtime); adaptar el único `JSX.Element` global a `React.JSX.Element`; subir `react-leaflet` a 5 y `chart.js` a 4.6.1; revisar los dos mocks de leaflet.
4. **Verificación por etapa:** `just frontend-lint`, `just frontend-test`, `just frontend-check` y `just check-all` en verde, más verificación visual manual de las vistas oscuras antes de mergear.
5. **Auditoría por etapa:** `@react-reviewer` sobre el diff de cada PR.
6. **Release y despliegue por etapa:** `release/vX.Y.Z` desde `development`, PR a `main`, tag, despliegue en `co1` y verificación de la versión en `/api/health`.
7. **Cierre:** `openspec archive frontend-react19-antd6`, renombrar este plan a `PLAN-019-COMPLETED.md`, actualizar el índice de `AGENTS.md` y cerrar los PRs de Dependabot que la migración deja obsoletos (`#280`, `#281`, `#284`, `#311`), dejando vivo solo `#310` si no se ha absorbido antes.

## 5. Plan de ejecución

1. **SDD:** change `frontend-react19-antd6`, su delta de spec y este plan (aprobación antes de escribir código).
2. **Etapa A — TDD:** PR a `development` con antd 6 sobre React 18, barrido de APIs deprecadas y tests ajustados.
3. **Etapa A — verificación y auditoría:** `just check-all` y `@react-reviewer`; verificación visual en local.
4. **Etapa A — release:** `release/v0.30.0`, PR a `main`, tag, despliegue en `co1` y comprobación visual en producción.
5. **Etapa B — TDD:** PR a `development` con React 19, tipos, `react-leaflet 5` y `chart.js 4.6.1`.
6. **Etapa B — verificación y auditoría:** `just check-all` y `@react-reviewer`; verificación visual en local.
7. **Etapa B — release:** `release/v0.31.0`, PR a `main`, tag, despliegue en `co1` y comprobación visual en producción.
8. **Cierre:** archivar el change, renombrar el plan, actualizar `AGENTS.md` y cerrar los PRs de Dependabot absorbidos.

## 6. Criterios de aceptación

- [ ] `npm ci` en `frontend/` termina sin errores ni avisos de peer dependencies.
- [ ] `frontend/package.json` declara React y sus tipos en la serie 19, `antd` en la 6, `react-leaflet` en la 5 y `chart.js` en 4.6.x, sin `@ant-design/v5-patch-for-react-19`.
- [ ] `npx @ant-design/cli lint frontend/src` no reporta APIs deprecadas.
- [ ] La aplicación arrancada no emite avisos de deprecación de antd en consola al recorrer las vistas que montan `Modal`, `Drawer`, `Tabs`, `Descriptions`, `Space` y `Table`.
- [ ] `npx vitest run` pasa sin haber eliminado ni relajado ninguna aserción existente.
- [ ] El tema oscuro conserva los colores declarados en `theme.ts`, con cada token verificado contra `antd token`.
- [ ] `just check-all` en verde (`tsc --noEmit`, `eslint --max-warnings 0`, `vitest run`, `build` y control de bundle inicial).
- [ ] El code-splitting de `vite.config.ts` conserva los grupos `vendor-react`, `vendor-antd`, `vendor-antd-icons`, `vendor-charts`, `vendor-leaflet` y `vendor-markdown`.
- [ ] Producción sirve la versión nueva en `/api/health` tras cada etapa, con el tema y las vistas revisadas a mano.
- [ ] `README.md` ya no dice «React 18 + Ant Design 5».

## 7. Riesgos

- **Token renombrado en `theme.ts` sin error de compilación.** Es el riesgo principal: antd no valida los nombres de los tokens en tiempo de tipos, así que un renombrado produce un cambio de color silencioso. Mitigado comparando la lista de `antd token` antes y después, y con revisión visual de `Layout`, `Menu`, `Table` y `Card`.
- **`@ant-design/icons@6` junto a `antd@5` (estado actual).** Combinación no soportada según la documentación; la migración la corrige, pero conviene no dejar el árbol en ese estado intermedio más de lo necesario. Mitigado subiendo ambos paquetes en el mismo commit de la etapa A.
- **`Tag` pierde su margen final en v6.** Cambio de layout aceptado por antd. Se revisan las listas de tags y, si el hueco importa, se reintroduce por contenedor y no con un override global.
- **`matchMedia` mockeado fichero a fichero en ~25 tests.** Cualquier cambio del entorno que antd 6 necesite puede hacer fallar varios a la vez; sin mock central, el diagnóstico es ruidoso. Mitigado no tocando el setup salvo necesidad demostrada, para que cada fallo siga apuntando a su fichero.
- **Namespace global `JSX` eliminado en los tipos de React 19.** Afecta a `test/ErrorBoundary.test.tsx:50`. Riesgo bajo y localizado.
- **`forwardRef` deprecado a futuro.** No bloquea: React 19 lo mantiene. Se deja como está para no mezclar refactor con migración.
- **Bundle inicial.** `npm run postbuild` controla regresiones: si antd 6 reorganiza subpaquetes, los regex de `vendor-antd` pueden dejar de casar y romper el build. Mitigado tratándolo como fallo del PR, no como umbral a subir.
- **Dependabot seguirá abriendo PRs de major hacia `development`** mientras esté la cola abierta. Mitigado cerrando como obsoletos los que la migración absorbe.

## 8. Verificación contra producción

Al cerrar cada etapa, sobre el despliegue de `co1`:

| Qué | Cómo |
|:---|:---|
| Versión desplegada | `curl -s https://valet.territoriolinux.es/api/health` |
| Tema oscuro | Revisión visual de cabecera, menú lateral, tablas de Stats y tarjetas de ajustes |
| Diálogos y pestañas | Ajustes completo (pestañas laterales), cualquier `Modal` de calendario y el `Drawer`/`Modal` del chat |
| Widgets | `LocationWidget` (mapa) y los diagramas de Stats |
| Consola del navegador | Sin avisos de API deprecada de antd |

La primera etapa es la que puede cambiar píxeles; la segunda no debería cambiar nada visible. Si la etapa B cambia algo, es señal de un problema de tipos mal resuelto.
