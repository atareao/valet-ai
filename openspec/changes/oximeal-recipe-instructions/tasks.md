# Tareas — change oximeal-recipe-instructions

Issue de entrada: [#334](https://github.com/atareao/valet-ai/issues/334)

## 1. RED — Tests que fallan primero

- [x] 1.1 Test de servicio: `create_recipe` con `instructions` en el `RecipeInput` envía el campo en el cuerpo a `POST /api/agent/recipes` (wiremock).
- [x] 1.2 Test de servicio: `create_recipe` sin `instructions` no incluye el campo y completa la petición.
- [x] 1.3 Test de herramienta: el esquema del parámetro `recipe` de `create_recipe` declara `instructions` entre las propiedades opcionales.
- [x] 1.4 Test de herramienta: `parse_recipe` acepta `instructions` no vacío sin salir a la red y sigue rechazando `ingredients` vacío.
- [x] 1.5 Test de prompt: el fragmento sembrado para `SKILL_MENUS_PROMPT` (en la migración) menciona `instructions` y `prep_time_minutes`, y mantiene la exigencia de `name`/`category`/`ingredients`.
- [x] 1.6 Ejecutar `cargo test` y confirmar que los tests nuevos fallan por ausencia de implementación.

## 2. GREEN — Implementación mínima

- [x] 2.1 Documentar `instructions` y `prep_time_minutes` (y `servings`, `diet_type`, `source_url`, `tags`) en el JSON Schema de `recipe` en `src/tools/oximeal.rs`.
- [x] 2.2 Confirmar/ajustar `parse_recipe` para que `instructions` viaje sin alterar (passthrough) y se documente la regla de validación.
- [x] 2.3 Actualizar el fragmento `SKILL_MENUS_PROMPT` en la migración nueva (`migrations/20261109000001_oximeal_recipe_instructions.sql`) para que pida `instructions` y `prep_time_minutes` (el texto del prompt vive solo en migraciones, no compilado en `skills.rs`).
- [x] 2.4 Añadir la migración aditiva e idempotente `migrations/<timestamp>_oximeal_recipe_instructions.sql` con guarda byte a byte del fragmento anterior.
- [x] 2.5 Ejecutar `cargo test` y `cargo check` y confirmar todo en verde.

## 3. REFACTOR — Limpieza

- [x] 3.1 `cargo fmt` y `cargo clippy -- -D warnings` sin warnings.
- [x] 3.2 Revisar que no queda texto antiguo del prompt en migraciones/tests.
- [x] 3.3 Re-ejecutar `cargo test`.

## 4. Verificación E2E y cierre

- [ ] 4.1 Verificar contra la instancia real de OxiMeal (`co1`) que una receta creada con instrucciones las guarda.
- [ ] 4.2 Crear `docs/oximeal-recipe-instructions.md` y actualizar `docs/oximeal-integration.md` si procede.
- [ ] 4.3 Marcar las tareas y archivar el change con `openspec archive oximeal-recipe-instructions`.
