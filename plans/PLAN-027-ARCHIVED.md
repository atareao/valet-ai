# PLAN-027 — Endurecer el contrato de Valet con la Agent API de OxiMeal

Issue: [#358](https://github.com/atareao/valet-ai/issues/358)
Change OpenSpec: `oximeal-contract-hardening`
PRs: [#359](https://github.com/atareao/valet-ai/pull/359) (feature → `development`), [#360](https://github.com/atareao/valet-ai/pull/360) (release → `main`)
Release: **v0.36.1**

## 1. Contexto

La integración OxiMeal (v0.32.0) no funcionaba en producción. Los logs de `co1` (7 días)
mostraban tres defectos del lado de Valet:

1. `search_recipes` fallaba siempre (8/8): `lista de recetas ilegible: invalid type: map,
   expected a string`. OxiMeal cambió `RecipeSummary.tags` a `Vec<RecipeTag>` (objetos) en su
   feature *recipe-tags* (v0.10.0), pero Valet lo declaraba `Vec<String>`.
2. `plan_week` rechazado (2 veces): sin búsqueda, el modelo inventaba `recipe_id`.
3. El detalle del `400` se perdía: `server_message` recogía el código máquina `error:
   "validation"` y descartaba el `message` humano.

## 2. Alcance

- **A)** `RecipeSummary.tags` tolera objetos `{id,name,slug,source}`; salida por **nombres**.
- **B)** `server_message` prefiere `message` sobre `error`.
- **C)** El esquema de `plan_week` documenta la procedencia del `recipe_id`.
- OxiMeal **no se toca**. Sin migraciones, ajustes, frontend ni despliegue nuevo.

## 3. Tareas

- [x] 3.1 Issue #358.
- [x] 3.2 Change `oximeal-contract-hardening` (proposal, deltas, design, tasks) validado en verde.
- [x] 3.3 TDD RED → GREEN → REFACTOR.
- [x] 3.4 Auditoría `@rust-reviewer` sin bloqueantes; hallazgos aplicados.
- [x] 3.5 PR #359 a `development` (CI verde) y PR #360 de release a `main`.
- [x] 3.6 Despliegue en `co1` y E2E.
- [x] 3.7 `openspec archive oximeal-contract-hardening`, índice de `plans/`, issue cerrado.

## 4. Verificación

- `cargo test` → 2261 passed / 0 failed.
- `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios.
- `openspec validate --all --strict` → 68 passed / 0 failed.

## 5. Pendiente / fuera de alcance

- Pre-validar `recipe_id` con una llamada extra a `search_recipes`: descartado (latencia y
  carrera). La mitigación del modelo que inventa ids es el arreglo A + el error legible (B).
