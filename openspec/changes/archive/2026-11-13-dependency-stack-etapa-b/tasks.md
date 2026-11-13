# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + tasks) y **esperar el OK explícito** del usuario antes de escribir tests. — OK explícito recibido («adelante»).

## 1. RED — la guarda tiene que fallar cuando debe

- [x] 1.1 Añadir las aserciones de la etapa B al `describe` del test. — Seis aserciones nuevas: `react`/`react-dom` (serie 19), `@types/react`/`@types/react-dom` (serie 19), `react-leaflet` (serie 5) y `chart.js` (major 4, minor ≥ 6).
- [x] 1.2 Verificar **en rojo** que detecta: bajar temporalmente `react` a `^18.3.1` en `frontend/package.json` y confirmar que el test falla; ídem `react-leaflet` a `^4.2.1` y `chart.js` a `^4.5.1`. Revertir cada manipulación. — **8/8 detecciones**, cada caso con **un solo fallo** y en el test que le corresponde: `react`, `react-dom`, `@types/react` y `@types/react-dom` a `^18`, `react-leaflet` a `^4.2.1`, `chart.js` a `^4.5.1` y a `^5.0.0`, más `antd` a `^5.22.0` como control de la etapa A. `package.json` restaurado byte a byte (`diff -q`).
- [x] 1.3 Confirmar que las tres aserciones de la etapa A siguen pasando. — Sí: el fichero queda en 9/9 (3 de etapa A + 6 de etapa B).

## 2. GREEN

- [x] 2.1 `npx vitest run src/test/dependency-stack.test.ts` en verde con el `package.json` real. — 9/9.
- [x] 2.2 Suite completa `npx vitest run` en verde. — 744/744 en 83 ficheros.

## 3. REFACTOR

- [x] 3.1 Renombrar el `describe` para que describa el stack completo (no «etapa A») y actualizar el comentario de cabecera, que hoy declara que la etapa B queda fuera. — `describe` → «stack de dependencias de UI del frontend»; cabecera reescrita para describir lo que vigila y declarar que **no** vigila la cadena de herramientas (`jsdom`, `vitest`, `vite`, `eslint`); eliminado el comentario final que anunciaba la etapa B.
- [x] 3.2 `npx tsc --noEmit` y `npm run lint` limpios. — Ambos limpios.

## 4. Cierre

- [x] 4.1 `openspec validate dependency-stack-etapa-b --strict` y `just check-spec`. — Válido (`skip_specs: true`); `openspec validate --all --strict`: 68 passed, 0 failed.
- [x] 4.2 PR a `development` con CI verde. — **#371**, ambos jobs en verde.
- [x] 4.3 Archivar el change, cerrar el issue #368 y crear `docs/dependency-stack-guard.md`