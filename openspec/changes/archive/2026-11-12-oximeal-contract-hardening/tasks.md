# Tasks: oximeal-contract-hardening

Issue: [#358](https://github.com/atareao/valet-ai/issues/358)

## 1. RED (tests que fallan)

### 1.1 Servicio OxiMeal (`src/services/oximeal.rs`)
- [x] 1.1.1 Test (`wiremock`): `search_recipes` con `tags` como objetos `{id,name,slug,source}` → `Ok` con el nombre de la etiqueta.
- [x] 1.1.2 Test: `recent_recipes` con `tags` objeto en una receta → `Ok` (sin regresión con la forma antigua).
- [x] 1.1.3 Test: receta sin `tags` (o `tags: []`) → `Ok`.
- [x] 1.1.4 Test: `400` con `{"error":"validation","message":"datos inválidos: receta inexistente: X"}` → `InvalidInput` con el mensaje humano (no `validation`).
- [x] 1.1.5 Test: `400` con solo `error` humano (sin `message`) → usa `error`.
- [x] 1.1.6 Test: `400` con cuerpo no JSON → usa el cuerpo recortado.
- [x] 1.1.7 `cargo test` → RED.

### 1.2 Herramienta OxiMeal (`src/tools/oximeal.rs`)
- [x] 1.2.1 Test: el esquema de `entries` (`plan_week`) nombra `search_recipes` y `create_recipe` como fuentes del `recipe_id`.
- [x] 1.2.2 Test: el esquema de `entries` indica que el identificador no se inventa.
- [x] 1.2.3 Test: un `400` de OxiMeal en `plan_week` surfacea el mensaje humano.
- [x] 1.2.4 `cargo test` → RED.

## 2. GREEN (implementación mínima)
- [x] 2.1 Añadir `TagRef { name: String }` (o equivalente) y cambiar `RecipeSummary.tags` a `Vec<TagRef>`.
- [x] 2.2 Ajustar `recipe_summary_json` para seguir emitiendo nombres.
- [x] 2.3 Reordenar los campos de `server_message` a `message → error → detail → title`.
- [x] 2.4 Ampliar la descripción de `entries` en el esquema de `plan_week`.
- [x] 2.5 `cargo test` verde y `cargo check` sin errores.

## 3. REFACTOR
- [x] 3.1 `cargo fmt --check` y `cargo clippy -- -D warnings` limpios.
- [x] 3.2 Comentarios de módulo y `Purpose` coherentes.
- [x] 3.3 `npx vitest run` en `frontend/` (sin cambios esperados).
- [x] 3.4 `openspec validate --all --strict` limpio.

## 4. CIERRE
- [x] 4.1 Actualizar `docs/oximeal-integration.md`.
- [x] 4.2 Auditoría `@rust-reviewer` sin bloqueantes.
- [x] 4.3 PR a `development` (gitmoji) y release a `main`.
- [x] 4.4 Despliegue en `co1` y E2E: `search_recipes` con etiquetas → `Ok`; `plan_week` con id inventado → mensaje humano. (despliegue v0.36.1 en co1 hecho; E2E pendiente del usuario)
- [x] 4.5 `openspec archive oximeal-contract-hardening`, índice de `plans/`, issue #358 cerrado. (pendiente hasta cerrar la E2E)
