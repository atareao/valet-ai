# Cola de Dependabot de noviembre de 2026

## Qué es

El barrido de la cola de Dependabot que se acumuló el 2026-11-12: seis PRs sobre `development`
(#352–#357). **Cinco bumps entran; el sexto no**, y no por un test: `vite` 8.3.5 → 8.3.7 deja el
grafo inicial del frontend en 70.01 kB sobre un presupuesto de 70.00 kB.

- Change: `deps-queue-2026-11` (archivado).
- Issue: [#367](https://github.com/atareao/valet-ai/issues/367). PR: [#369](https://github.com/atareao/valet-ai/pull/369).
- Plan: `plans/PLAN-029-ARCHIVED.md`.

## Ficheros implicados

- `frontend/package.json` y `frontend/package-lock.json` — los cuatro bumps del frontend.
- `Cargo.lock` — `uuid` 1.29.2. `Cargo.toml` **no** cambia: declara `version = "1"`.
- `frontend/scripts/check-initial-bundle.mjs` — **no se toca**: el umbral sigue en 70.00 kB.
- **Ningún** fichero de código de producción ni de test.

## Cómo funciona

### Los cinco bumps que entraron

| Dependencia | Salto | Tipo |
|---|---|---|
| `uuid` (Rust) | 1.29.0 → 1.29.2 | patch |
| `react-router-dom` | 7.18.4 → 7.18.5 | patch |
| `vitest` | 5.0.5 → 5.1.0 | minor |
| `eslint` | 10.11.0 → 10.14.0 | minor |
| `jsdom` | 30.1.3 → **31.0.0** | **major** |

Ninguno de los cuatro bumps del frontend aporta **un solo byte** al grafo inicial: medido con
Node 22, el reparto queda idéntico al baseline (`index` 3.62 kB / `rolldown-runtime` 0.42 kB /
`vendor-react` 65.95 kB = 69.98 kB).

### Por qué `vite` 8.3.7 no entra

| | `index` | `rolldown-runtime` | `vendor-react` | total |
|---|---|---|---|---|
| `development` (`6cdc1fa`) | 3.62 kB | 0.42 kB | 65.95 kB | **69.98 kB** |
| con `vite` 8.3.7 | 3.64 kB | 0.42 kB | 65.95 kB | **70.01 kB** |

El presupuesto es de 70.00 kB y **no se sube**: `openspec/specs/frontend/spec.md` exige que el
control de bundle pase sin tocar su umbral. El grafo inicial está en su suelo —65.95 de los
70.00 kB son `react` + `react-dom`—, así que no hay nada que recortar para hacerle sitio a un
patch del bundler. `vite` se queda en `^8.3.5` y el PR #353 se cerró con este motivo.

### Qué trajo `jsdom` 31

Es un *major*: reescribe el motor CSS (serialización y valores computados pueden diferir), cambia
el backend de canvas a `@napi-rs/canvas` y puede devolver documentos parcialmente parseados si hay
scripts bloqueantes.

- **No rompió nada.** 738 tests verdes **sin adaptar ningún test**, a diferencia del precedente
  `adopt-jsdom-30-1-3`, donde dos tests dependían de dos bugs de jsdom. No se borró, saltó ni
  relajó ninguna aserción.
- **El backend de canvas es inerte aquí:** `@napi-rs/canvas` es un peer **opcional** y no se instala.
- **Efecto colateral en el lockfile:** desaparece el árbol CSS de jsdom 30 (`@csstools/*`,
  `css-tree`, `@asamuzakjp/*`, `mdn-data`, `bidi-js`, `require-from-string`) y entran `bcp-47`,
  `bcp-47-match` y `parse5-parser-stream`. De ahí las ~300 líneas que adelgaza el lockfile.

## Variables/config

Ninguna. Este change no introduce ajustes, endpoints, migraciones ni variables de entorno.

## Cómo habilitar/desplegar

**No hay despliegue.** Son dependencias de desarrollo del frontend (`jsdom`, `vitest`, `eslint`)
más un patch de `uuid`; el backend no cambia de comportamiento, así que no requiere release ni
recrear el stack en `co1`. Entra en el siguiente release que se publique.

## Decisiones

- **Los cinco bumps en una sola rama y un solo PR.** Cinco PRs independientes serían cinco rondas
  de CI y cinco merges para cambios que no interactúan entre sí.
- **`vite` 8.3.7 se queda fuera y el umbral no se sube.** Alternativas descartadas: subir el umbral
  (lo prohíbe la spec) y recortar el grafo inicial (65.95 de 70.00 kB son React).
- **`jsdom` 31 entra como *major* con la suite verificada en local**, no con el resultado del PR
  automático, que corría sobre una base `BEHIND`.
- **`skip_specs: true`:** no hay cambio de comportamiento observable, así que no se toca ninguna
  spec (regla de OpenSpec: «specs describe behavior… Do not invent a requirement just to satisfy
  validation»).

## Follow-ups

- **Política del umbral de bundle (abierto).** El margen real es de 0.02 kB. Toca decidir si se
  sube el umbral de forma deliberada —una vez, con criterio— o si se asume que los patches de
  `vite` quedan congelados mientras el margen sea este. No se decidió en este change.
- **El gate local depende del Node del runtime.** Con **Node 24** (el `node` por defecto en esta
  máquina) el mismo árbol mide **70.02 kB** y el check sale rojo; con **Node 22** (el que fija
  `.github/workflows/ci.yml`) mide **69.98 kB** y pasa. Es el `zlib` del runtime, no el código.
  Candidato: fijar la versión esperada con `.nvmrc`/`engines` para que un rojo local no se
  confunda con una regresión.
- **Dependabot volverá a proponer `vite`.** Riesgo aceptado y documentado en el `design.md` del
  change: mientras el margen siga en 0.02 kB, ese PR volverá a estar rojo.