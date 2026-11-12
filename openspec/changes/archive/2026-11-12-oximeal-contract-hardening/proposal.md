# Change: oximeal-contract-hardening

## Why

La integración OxiMeal (v0.32.0) no funciona en producción como se espera. Revisando `docker logs valet`
en `co1` (7 días) aparecen tres defectos distintos, todos del lado de Valet:

1. **`search_recipes` falla siempre** (8/8 búsquedas): `lista de recetas ilegible: invalid type: map,
   expected a string`. OxiMeal cambió `RecipeSummary.tags` a `Vec<RecipeTag>` (objetos
   `{id, name, slug, source}`) en su feature *recipe-tags* (v0.10.0), pero `RecipeSummary` de Valet lo
   declara como `Vec<String>`, así que la decodificación revienta en cuanto una receta trae etiquetas.
2. **`plan_week` recibe ids inventados** (2 rechazos): al no poder buscar, el modelo rellena
   `recipe_id` con identificadores inventados (`CREMA_CALABAZA`) y OxiMeal responde `400`.
3. **El detalle del `400` se pierde**: `server_message` lee `["error","message","detail","title"]` en
   ese orden, así que recoge el código máquina `"validation"` y descarta la explicación real
   (`"datos inválidos: receta inexistente: …"`).

Issue [#358](https://github.com/atareao/valet-ai/issues/358).

## What Changes

- **`RecipeSummary` tolera las etiquetas objeto.** `tags` pasa a decodificarse como objetos de etiqueta
  y la herramienta sigue exponiendo **nombres** (`name`) en su salida, sin cambiar la forma del JSON.
- **`server_message` prefiere `message` sobre `error`.** El detalle de un `4xx` de OxiMeal conserva la
  explicación humana; el código máquina solo se usa como último recurso.
- **El esquema de `plan_week` documenta la procedencia del `recipe_id`.** La descripción del parámetro
  `entries` deja claro que `recipe_id` sale de `search_recipes` o de `create_recipe`, nunca inventado.
- **Sin cambios** en OxiMeal, en las rutas, en los ajustes, en las migraciones ni en el frontend.

## Capabilities

### New Capabilities

_(ninguna)_

### Modified Capabilities

- `oximeal-service`: la decodificación de la lista de recetas tolera la forma real de `tags` y la
  traducción de errores prefiere el campo `message` del servidor.
- `tools/registry`: la definición de la herramienta `oximeal` documenta que el `recipe_id` de
  `plan_week` proviene de `search_recipes`/`create_recipe`.

## Impact

- Código: `src/services/oximeal.rs` (`RecipeSummary`, `decode_list`, `server_message`) y sus tests;
  `src/tools/oximeal.rs` (descripción del esquema de `plan_week`) y sus tests.
- Documentación: `docs/oximeal-integration.md`.
- Sin migraciones, sin ajustes nuevos, sin cambios de frontend, sin cambios de despliegue.
- Fuera de alcance: pre-validar `recipe_id` con una llamada extra a `search_recipes` y cualquier cambio
  en OxiMeal.
