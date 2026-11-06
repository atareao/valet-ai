# Tasks

## 1. Preparación

- [x] 1.1 Crear la rama `feature/frontend-antd6` desde `development` y comprobar que `just check-all` está verde antes de tocar nada (verificación: `just check-all` termina con código 0 en el árbol limpio).
- [x] 1.2 Fijar la línea base de APIs deprecadas y de tokens con `npx @ant-design/cli lint frontend/src` y `npx @ant-design/cli token <Componente>` para cada componente de `theme.ts` (verificación: la salida queda adjunta al PR como punto de comparación).
- [x] 1.3 Fijar la línea base de bundle con `cd frontend && npm run build` (verificación: se registra el tamaño del chunk inicial y los chunks `vendor-*` generados).

## 2. Etapa A — antd 5 → 6 (sobre React 18)

- [x] 2.1 Subir `antd` a la serie 6 y `@ant-design/icons` a `^6.0.0` en el mismo commit, y regenerar `frontend/package-lock.json` (verificación: `npm ci` en `frontend/` termina sin errores ni avisos de peer dependencies).
- [x] 2.2 Sustituir `direction` por `orientation` en todos los `Space` y `Space.Compact` (verificación: `rg 'direction="' frontend/src` no devuelve ningún uso en `Space` y `vitest run` sigue verde).
- [x] 2.3 Sustituir `tabPosition` por `tabPlacement` en los seis `Tabs` laterales y actualizar el test que fija ese atributo (verificación: `test/SettingsDialog.test.tsx` comprueba la prop vigente y pasa, y las pestañas siguen en vertical).
- [x] 2.4 Sustituir `destroyOnClose` por `destroyOnHidden` en `EventModal.tsx` y `maskClosable` por `mask.closable` en `ChatView.tsx` (verificación: `npx @ant-design/cli lint frontend/src` no reporta ninguna de las dos).
- [x] 2.5 Confirmar que `Descriptions` conserva `bordered`: en antd 6.7.0 `DescriptionsProps` no declara `variant` y `bordered` no está deprecado, así que no se migra (verificación: `npx @ant-design/cli lint frontend/src` no reporta ninguna de las dos y los tests de `EventDetail` y `LastApiCallCard` pasan).
- [x] 2.6 Verificar `theme.ts` token a token contra `antd token` y corregir los que hayan cambiado de nombre (verificación: la lista de tokens vigentes coincide con la declarada y `Layout`, `Menu`, `Table` y `Card` conservan sus colores en revisión visual).
- [x] 2.7 Revisar el efecto de la pérdida del margen final de `Tag` en las listas de tags (verificación: revisión visual de las listas afectadas, con corrección local por contenedor si el hueco se nota).
- [x] 2.8 Pasar `just frontend-lint`, `just frontend-test` y `just frontend-check` sin relajar ninguna aserción ni subir el umbral del bundle inicial (verificación: los tres terminan con código 0 y el diff no elimina ninguna aserción existente).
- [x] 2.9 Verificación manual en local de las vistas oscuras: cabecera, menú lateral, tablas de Stats, ajustes completo, calendario, chat y mapa (verificación: sin avisos de deprecación en consola y sin cambios de color respecto a la línea base).
- [x] 2.10 Auditoría de `@react-reviewer` sobre el diff de la etapa A (verificación: 0 bloqueantes; los hallazgos se corrigen antes de abrir el PR).
- [ ] 2.11 PR `feature/frontend-antd6` a `development` con CI verde (verificación: los jobs `Backend (Rust)` y `Frontend (Node)` en verde).
- [x] 2.12 Migrar los siete usos de `List` a `Listy` (`NotesView`, `JournalView`, `NotificationsView`), con `items`/`itemRender`, `rowKey` estable y `role="list"` en el contenedor que agrupa las filas `role="listitem"` (verificación: `antd lint` no reporta `List` deprecado y las tres vistas comprueban por rol que la lista tiene su contenedor).
- [x] 2.13 Convertir «0 deprecadas» en una puerta de CI: `frontend/scripts/check-antd-deprecations.mjs` (solo falla por `rule === "deprecated"`, ignora `performance`/`a11y`/`usage`), script `lint:antd` en `frontend/package.json`, dependencia `@ant-design/cli@6.7.0` y enganche en `.justfile` (`frontend-lint`) y `.github/workflows/ci.yml` (verificación: la puerta sale 0 con el árbol limpio y 1 al inyectar una deprecación temporal).

## 3. Etapa A — release

- [ ] 3.1 `release/v0.30.0` desde `development`, PR a `main` y tag (verificación: el tag existe y el PR está mergeado en `main`).
- [ ] 3.2 Despliegue en `co1` y verificación de la versión (verificación: `curl -s https://valet.territoriolinux.es/api/health` devuelve `0.30.0`).
- [ ] 3.3 Verificación visual en producción: tema oscuro, pestañas laterales de ajustes, tablas de Stats y mapa del widget de ubicación (verificación: sin avisos de deprecación en consola y sin cambios visuales respecto a v0.29.0).

## 4. Etapa B — React 18 → 19

- [ ] 4.1 Crear la rama `feature/frontend-react19` desde `development` (ya con antd 6) (verificación: `just check-all` verde en el punto de partida).
- [ ] 4.2 Subir `react` y `react-dom` a la serie 19 y `@types/react` y `@types/react-dom` a la serie 19 en el mismo commit, y regenerar el lock (verificación: `npm ci` limpio, sin avisos de peer dependencies).
- [ ] 4.3 Sustituir el `JSX.Element` global de `test/ErrorBoundary.test.tsx` por `React.JSX.Element` (verificación: `npx tsc --noEmit` no reporta el namespace `JSX`).
- [ ] 4.4 Subir `react-leaflet` a 5 y revisar `LocationWidget` y sus dos mocks (verificación: `test/LocationWidget.test.tsx` y `test/WidgetRenderer.test.tsx` pasan).
- [ ] 4.5 Subir `chart.js` a 4.6.1 y revisar los cuatro ficheros de test que mockean `react-chartjs-2` o el canvas (verificación: los tests de `stats/` y de `StatsDashboard` pasan).
- [ ] 4.6 Confirmar que no queda ninguna API retirada en React 19 (`render`, `hydrate`, `findDOMNode`, `unmountComponentAtNode`, `createFactory`, `propTypes`, `defaultProps` de componente de función) (verificación: `rg` sobre `frontend/src` no devuelve ninguna y la aplicación arranca sin avisos de React).
- [ ] 4.7 Pasar `just frontend-lint`, `just frontend-test` y `just frontend-check` sin relajar aserciones (verificación: los tres terminan con código 0 y el code-splitting conserva todos los grupos `vendor-*`).
- [ ] 4.8 Actualizar `README.md`, que hoy anuncia «React 18 + Ant Design 5» (verificación: `rg 'React 18' README.md` no devuelve nada y las versiones citadas coinciden con `frontend/package.json`).
- [ ] 4.9 Actualizar el índice de planes de `AGENTS.md` con la entrada de `PLAN-019` (verificación: la entrada aparece con el estado correcto y el ordinal siguiente sigue libre).
- [ ] 4.10 Verificación manual en local de las mismas vistas que en la etapa A (verificación: sin cambios visuales respecto a la etapa A y sin avisos en consola).
- [ ] 4.11 Auditoría de `@react-reviewer` sobre el diff de la etapa B (verificación: 0 bloqueantes; los hallazgos se corrigen antes de abrir el PR).
- [ ] 4.12 PR `feature/frontend-react19` a `development` con CI verde (verificación: los dos jobs del CI en verde).

## 5. Etapa B — release

- [ ] 5.1 `release/v0.31.0` desde `development`, PR a `main` y tag (verificación: el tag existe y el PR está mergeado en `main`).
- [ ] 5.2 Despliegue en `co1` y verificación de la versión (verificación: `curl -s https://valet.territoriolinux.es/api/health` devuelve `0.31.0`).
- [ ] 5.3 Verificación visual en producción de las mismas vistas (verificación: idénticas a la etapa A; un cambio visual aquí es señal de un problema de tipos mal resuelto).

## 6. Verificación de integración

- [ ] 6.1 `just check-all` sobre `development` con ambas etapas dentro (verificación: código 0, incluidos los chequeos de Rust, que no se han tocado).
- [ ] 6.2 `openspec validate --all --strict` (verificación: 0 fallos).
- [ ] 6.3 Cerrar como obsoletos los PRs de Dependabot que la migración absorbe (`#280`, `#281`, `#284`, `#311`) y confirmar que `#310` quedó mergeado o absorbido (verificación: no queda ningún PR de Dependabot de `frontend` abierto que la migración deje atrás).

## 7. Cierre

- [ ] 7.1 `openspec archive frontend-react19-antd6` (verificación: el delta se fusiona en `openspec/specs/frontend/spec.md` y el change pasa a `changes/archive/`).
- [ ] 7.2 Renombrar este plan a `plans/PLAN-019-COMPLETED.md` y actualizar el índice de `AGENTS.md` (verificación: el índice apunta al fichero renombrado con estado completado).
- [ ] 7.3 Añadir a `AGENTS.md` la decisión cerrada con las versiones finales del stack y lo que no se reabre (verificación: la entrada existe y cita las versiones de `frontend/package.json`).
