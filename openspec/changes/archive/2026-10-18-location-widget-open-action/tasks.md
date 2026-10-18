# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — resolución de enlaces por plataforma

- [x] 1.1 Tests que fallan: Android resuelve a un destino que empieza por `geo:`, contiene las coordenadas y NO fija paquete; iOS resuelve a `maps.apple.com`; escritorio resuelve a la URL oficial de Google Maps. Verificar en rojo con `npx vitest run`.
- [x] 1.2 Tests que fallan: la etiqueta va codificada (espacios, acentos y paréntesis) y no sustituye a las coordenadas; sin `title` ni `address` el destino no lleva paréntesis.
- [x] 1.3 Test que falla: un `userAgent` de iPadOS (declarado `Macintosh` con `maxTouchPoints > 1`) resuelve como iOS.

## 2. GREEN — resolución de enlaces

- [x] 2.1 Implementar el módulo puro `frontend/src/components/widgets/locationLinks.ts` y verificar 1.1–1.3 en verde.
- [x] 2.2 Verificar `npx tsc --noEmit` sin errores en el módulo nuevo.

## 3. RED — el widget cambia de acciones

- [x] 3.1 Test que falla: el widget NO ofrece el botón «Guardar» y NO invoca `onAction` en ningún caso.
- [x] 3.2 Tests que fallan: «Abrir» apunta al destino que resuelve el módulo para cada plataforma y no abre pestaña nueva; «Cómo llegar» y «Copiar coordenadas» siguen funcionando (estas dos no tenían ningún test hasta ahora).
- [x] 3.3 Quitar el test existente de `save_place` (`frontend/src/test/LocationWidget.test.tsx:55`) y verificar que la suite del widget queda en rojo solo por los casos nuevos.
- [x] 3.4 Cerrar el agujero de cobertura que señaló la revisión: el cableado `label: title ?? address` no lo cubría ningún test del widget —se podía degradar sin que nada se pusiera rojo— y el test de «las coordenadas, no la dirección» corría en la rama que ignora la etiqueta, así que su aserción no podía fallar por el motivo que enunciaba. Tests nuevos para el cableado, el *fallback* a `address` y la prioridad del `title`, más la comprobación por mutación de que muerden: degradar la llamada a `openLocationUrl` deja 3 tests en rojo.

## 4. GREEN — el widget

- [x] 4.1 `LocationWidget.tsx`: quitar «Guardar», `handleSave`, `onAction` y el `save_place`; añadir «Abrir» como ancla al destino resuelto por plataforma; actualizar el docstring del componente, que hoy documenta `onAction("save_place", …)`.
- [x] 4.2 Verificar la suite del widget en verde con `npx vitest run` y `npx tsc --noEmit` limpio.
- [x] 4.3 Verificar por `grep` que no queda ninguna referencia a `save_place` en `frontend/src/`.

## 5. Refactor y build versionada

- [x] 5.1 `npx eslint . --max-warnings 0` limpio en `frontend/` (0 warnings, incluido el prop `onAction` ya sin uso).
- [x] 5.2 Reconstruir el frontend (`npm run build` en `frontend/`) y sincronizar la copia versionada `static/`, **excluyendo `.vite/`** (el `Dockerfile` lo borra a propósito antes de copiar la imagen, porque no debe servirse con `ServeDir`); verificar que `static/assets/LocationWidget-*.js` ya no contiene `save_place` y que no queda ningún chunk con el hash anterior. La sincronía arrastra además una deriva preexistente de siete commits: 22 ficheros de `static/` cambian (12 añadidos, 9 borrados, 1 modificado).
- [x] 5.3 Verificar que `cargo test` sigue en verde (baseline: 1423 passed en 38 suites), incluida `tests/api/frontend_serving.rs`, que sirve `static/`.

## 6. Verificación de integración y cierre

- [x] 6.1 `npx vitest run` completo en verde (baseline: 61 ficheros / 433 tests; ahora 62 / 466) y `npx tsc --noEmit` limpio.
- [x] 6.2 `openspec validate location-widget-open-action --strict` en verde.
- [x] 6.3 Prueba real desde el móvil contra el contenedor (`just dev`): «Abrir» abre el selector de aplicaciones o la app de mapas por defecto sin dejar pestaña en blanco, y «Cómo llegar» sigue abriendo Google Maps.
- [x] 6.4 Revisión con `@react-reviewer`; hallazgos aplicados (I1 resincronía de `static/`, I2 esta propia puesta al día, I3 cobertura del cableado de la etiqueta, M1 aserción real sobre `window.open`). Las observaciones sin acción quedan anotadas en `plans/PLAN-006.md`.
- [x] 6.5 `openspec archive location-widget-open-action` y actualizar `plans/PLAN-006.md` (Tema 1) marcando lo completado.
