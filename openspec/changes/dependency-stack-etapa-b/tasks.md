# Tasks

## 0. Aprobación

- [ ] 0.1 Presentar este change (proposal + design + tasks) y **esperar el OK explícito** del usuario antes de escribir tests.

## 1. RED — la guarda tiene que fallar cuando debe

- [ ] 1.1 Añadir las aserciones de la etapa B al `describe` del test.
- [ ] 1.2 Verificar **en rojo** que detecta: bajar temporalmente `react` a `^18.3.1` en `frontend/package.json` y confirmar que el test falla; ídem `react-leaflet` a `^4.2.1` y `chart.js` a `^4.5.1`. Revertir cada manipulación.
- [ ] 1.3 Confirmar que las tres aserciones de la etapa A siguen pasando.

## 2. GREEN

- [ ] 2.1 `npx vitest run src/test/dependency-stack.test.ts` en verde con el `package.json` real.
- [ ] 2.2 Suite completa `npx vitest run` en verde.

## 3. REFACTOR

- [ ] 3.1 Renombrar el `describe` para que describa el stack completo (no «etapa A») y actualizar el comentario de cabecera, que hoy declara que la etapa B queda fuera.
- [ ] 3.2 `npx tsc --noEmit` y `npm run lint` limpios.

## 4. Cierre

- [ ] 4.1 `openspec validate dependency-stack-etapa-b --strict` y `just check-spec`.
- [ ] 4.2 PR a `development` con CI verde.
- [ ] 4.3 Archivar el change, cerrar el issue #368 y crear `docs/dependency-stack-guard.md`.