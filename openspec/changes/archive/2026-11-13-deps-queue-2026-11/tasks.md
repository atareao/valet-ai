# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — registrar el estado antes de tocar nada

- [x] 1.1 Registrar el baseline en `development` (`6cdc1fa`): `just check-all` verde y `total gzip inicial: 69.98 kB`.
- [x] 1.2 Crear la rama `chore/deps-queue-2026-11` desde `development`.
- [x] 1.3 Aplicar los cinco bumps (`npm install` en `frontend/`, `cargo update -p uuid --precise 1.29.2`) **sin tocar ningún test** y registrar qué falla, si algo falla, con `jsdom` 31. — **Nada falló**: 738 tests / 83 ficheros en verde con los tests intactos.

## 2. GREEN — dejar la suite verde

- [x] 2.1 `npx vitest run` en verde (suite completa del frontend): 738 tests / 83 ficheros.
- [x] 2.2 Si un test falla por el motor CSS nuevo de jsdom 31 o por su parseo, adaptar el test al comportamiento correcto; **prohibido** borrar, saltar o relajar aserciones. — **No aplicó**: ningún test dependía de un comportamiento viejo de jsdom, así que no se adaptó, borró, saltó ni relajó ninguno.
- [x] 2.3 `cargo test` en verde tras el bump de `uuid`: 2277 tests, 0 fallos.

## 3. REFACTOR y verificación

- [x] 3.1 `npx tsc --noEmit`, `npm run lint` y `cargo clippy --all-targets -- -D warnings` limpios (clippy, 0 warnings); `cargo fmt --check` limpio.
- [x] 3.2 `npm run build` y confirmar `total gzip inicial` ≤ 70.00 kB **sin tocar el umbral**: 69.98 kB (presupuesto 70.00 kB), byte-idéntico al baseline — los cuatro bumps del frontend aportan 0 bytes al grafo inicial. Medido con Node 22, el del CI.
- [x] 3.3 Verificación de punta a punta: `cargo test` 2277/0, `vitest` 738/0, `tsc`, `lint`, `clippy`, `fmt` y `check-initial-bundle` en verde.

## 4. Cierre

- [x] 4.1 `openspec validate deps-queue-2026-11 --strict` (válido) y `just check-spec` (OK).
- [x] 4.2 PR a `development` con CI verde: **#369**, ambos jobs en verde.
- [x] 4.3 Cerrar #352, #354, #355, #356 y #357 como absorbidos, referenciando este change.
- [x] 4.4 Cerrar **#353** con el motivo medido (70.01 kB > 70.00 kB; el umbral no se sube).
- [x] 4.5 Archivar el change, cerrar el issue #367 y crear `docs/deps-queue-2026-11.md`