# Change: Landear la cola de Dependabot de noviembre de 2026 y cerrar `vite` 8.3.7 con el motivo medido

## Why

Dependabot acumula **6 PRs abiertos** sobre `development` (#352–#357), todos del 2026-11-12 y todos `BEHIND`. Ninguno es una migración, pero uno **no se puede mergear**: `vite` 8.3.7 (patch) rompe el presupuesto de bundle inicial. Rebasado sobre `development` y con el CI relanzado (run `43368520339`), sigue rojo, y no por un test:

- `development` (`6cdc1fa`): `index` 3.62 kB + `rolldown-runtime` 0.42 kB + `vendor-react` 65.95 kB = **69.98 kB** / 70.00 kB.
- Con `vite` 8.3.7: `index` 3.64 kB → **70.01 kB** / 70.00 kB → `✖ check-initial-bundle`.

El grafo inicial está en su suelo —65.95 de los 70.00 kB son `react` + `react-dom`, y 3.62 el código de entrada—, así que no hay nada que recortar, y el umbral no se sube (`openspec/specs/frontend/spec.md`: «el control de bundle inicial SHALL pasar sin subir su umbral»). Dejar los seis PRs ahí no resuelve nada: los cinco que sí caben caducan y vuelven a proponerse en el siguiente ciclo.

`jsdom` 31.0.0 es además un *major* con el motor CSS reescrito, el backend de canvas cambiado y un parseo que puede devolver documentos parcialmente parseados, así que se adopta con la suite verificada en local, no por inercia del CI de Dependabot.

## What Changes

- **MODIFIED** dependencia: `uuid` 1.29.0 → **1.29.2** (solo `Cargo.lock`; `Cargo.toml` declara `version = "1"` y no cambia).
- **MODIFIED** dependencias de desarrollo en `frontend/`: `react-router-dom` 7.18.4 → **7.18.5`, `vitest` 5.0.5 → **5.1.0`, `eslint` 10.11.0 → **10.14.0`, `jsdom` 30.1.3 → **31.0.0` (major).
- **NO SE LANDEA** `vite` 8.3.7: se queda en `^8.3.5` y el PR #353 se cierra con el motivo medido.
- **NO SE TOCA** el umbral de 70.00 kB de `frontend/scripts/check-initial-bundle.mjs`.
- **NO SE RELAJA** ninguna aserción de test para que entre un bump.
- Sin cambios de comportamiento observable, de API ni de esquema.

## Capabilities

- **New Capabilities**: (ninguna)
- **Modified Capabilities**: (ninguna) — bumps de dependencia sin cambio de comportamiento observable; se marca `skip_specs: true` en `.openspec.yaml`. No se inventa un requirement solo para satisfacer la validación.

## Impact

- **Dependencias**: `Cargo.lock`; `frontend/package.json` y `frontend/package-lock.json`.
- **Sistemas**: job `frontend` del CI (`tsc --noEmit`, `eslint --max-warnings 0`, `vitest run`, `npm run build`) y el control `check-initial-bundle`.
- **PRs de Dependabot**: #352, #354, #355, #356 y #357 quedan absorbidos por este change; **#353 se cierra** con el motivo del presupuesto.
- **Producción**: sin despliegue; el backend no cambia de comportamiento.
- **Fuera de alcance**: `docker-compose.prod.yml`, cualquier otro bump y la política del umbral de bundle (se registra como follow-up, no se decide aquí).