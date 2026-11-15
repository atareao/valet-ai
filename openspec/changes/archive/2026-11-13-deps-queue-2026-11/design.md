# Design

## Decisión 1 — los cinco bumps van en una sola rama y un solo PR

Cinco PRs independientes de Dependabot son cinco rondas de CI, cinco merges y cinco entradas en el historial para cambios que no interactúan entre sí. Agrupados en `chore/deps-queue-2026-11`, la verificación es una sola y el `package-lock.json` se resuelve de una vez, sin conflictos entre ramas que tocan el mismo lockfile. Precedente: `frontend-react19-antd6` absorbió cuatro PRs de una sola vez.

## Decisión 2 — `vite` 8.3.7 no se landea y el umbral no se sube

Medición sobre la rama rebasada sobre `development` (run `43368520339`): **70.01 kB > 70.00 kB**. Alternativas evaluadas:

| Alternativa | Veredicto |
|---|---|
| Subir el umbral (p. ej. a 72 kB) | **Descartada.** La spec lo prohíbe expresamente y el umbral existe para que el code-splitting no se degrade sin que nadie lo note. |
| Recortar el grafo inicial | **Descartada.** 65.95 de los 70.00 kB son `react` + `react-dom` y 3.62 el código de entrada: el margen real es de 0.02 kB, por debajo del ruido de un patch del bundler. No hay nada que recortar sin sacar React del grafo inicial, que es justo lo que no se puede hacer. |
| Cerrar #353 y congelar `vite` en `^8.3.5` | **Elegida.** |

Riesgo aceptado y explícito: Dependabot volverá a proponer `vite` en el siguiente ciclo y el PR volverá a estar rojo. Se acepta a cambio de no abrir una excepción al presupuesto por un patch de una herramienta de build.

**Follow-up registrado (fuera de alcance):** decidir la política del umbral de forma explícita —o asumir que los patches de `vite` quedan congelados— cuando el margen de 0.02 kB empiece a bloquear algo que sí importe. No se decide aquí.

## Decisión 3 — `jsdom` 31 entra como *major* y se verifica con la suite en local

jsdom 31 reescribe el motor CSS (serialización y valores computados pueden diferir), cambia el backend de canvas a `@napi-rs/canvas` y puede devolver documentos parcialmente parseados si hay scripts bloqueantes. El CI de #356 está verde, pero sobre una base `BEHIND`, así que la verificación válida es `npx vitest run` sobre la rama rebasada, no el resultado del PR automático.

Precedente idéntico: `adopt-jsdom-30-1-3`, donde dos tests del frontend dependían de dos bugs de jsdom y hubo que alinearlos. Si aquí aparece un test que dependía de un comportamiento viejo, **se adapta el test al comportamiento correcto; no se borra, no se salta y no se relaja la aserción**.