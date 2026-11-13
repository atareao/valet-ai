# PLAN-029 — La cola de Dependabot entra, salvo el patch de `vite` que no cabe en el presupuesto

Issue: [#367](https://github.com/atareao/valet-ai/issues/367)
Change OpenSpec: `deps-queue-2026-11`

## 1. Contexto

Dependabot acumuló 6 PRs sobre `development` (#352–#357) el 2026-11-12, todos `BEHIND`. Cinco son bumps que no interactúan entre sí; el sexto, `vite` 8.3.5 → 8.3.7, **falla el CI** y no por un test: rebasado sobre `development` (run `43368520339`) deja el grafo inicial en **70.01 kB** contra un umbral de **70.00 kB**. `development` está en 69.98 kB, así que el margen real es de 0.02 kB y el grafo inicial no tiene nada que recortar (65.95 de los 70.00 kB son `react` + `react-dom`). El umbral no se sube: está prohibido por la spec.

## 2. Alcance

- **Landear 5 bumps** en una sola rama `chore/deps-queue-2026-11`: `uuid` 1.29.2 (`Cargo.lock`), `react-router-dom` 7.18.5, `vitest` 5.1.0, `eslint` 10.14.0 y `jsdom` **31.0.0** (major).
- **No landear `vite` 8.3.7** y cerrar #353 con el motivo medido.
- **Verificar `jsdom` 31** con la suite en local, adaptando los tests que dependieran de comportamiento viejo de jsdom (precedente `adopt-jsdom-30-1-3`), sin relajar aserciones.
- **No tocar** el umbral de bundle ni `docker-compose.prod.yml`.

## 3. Tareas

- [ ] 3.1 Change `deps-queue-2026-11` (proposal, design, tasks) validado y aprobado.
- [ ] 3.2 TDD RED → GREEN → REFACTOR con los cinco bumps aplicados.
- [ ] 3.3 `just check-all` verde y `total gzip inicial` ≤ 70.00 kB sin tocar el umbral.
- [ ] 3.4 PR a `development` con CI verde.
- [ ] 3.5 Cerrar #352, #354, #355, #356 y #357 como absorbidos; **#353** con el motivo medido.
- [ ] 3.6 `openspec archive deps-queue-2026-11`, fila en el índice de `plans/`, `docs/deps-queue-2026-11.md` y issue #367 cerrado.

## 4. Fuera de alcance

- Subir el umbral del control de bundle (`frontend/scripts/check-initial-bundle.mjs`).
- `docker-compose.prod.yml`.
- La política del umbral de bundle: queda como follow-up registrado en el `design.md`, no se decide en este plan.