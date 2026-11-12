-- OxiMeal search semantics (oximeal-search-semantics, task 2.4).
--
-- Same idiom as the previous prompt migrations
-- (`20261109000001_oximeal_recipe_instructions.sql`,
-- `20261104000001_widget_options_contract.sql`): additive and idempotent, with a
-- **byte-for-byte guard**. Only the exact texts a previous release seeded are
-- replaced; any other value is user content and is never touched. No other
-- setting is modified.
--
-- Why: OxiMeal's recipe search matches a **contiguous substring** of the name or
-- the instructions (FTS5 `trigram`, quoted phrase), so the agent's word lists
-- («tomate aguacate mozzarella») matched nothing and it refused to plan the
-- week. Valet now splits the query into terms and combines the results, but the
-- skill fragment must still tell the model to search with **short, distinctive**
-- terms and where the `recipe_id` for `plan_week` comes from. This migration
-- updates the `menus` skill fragment accordingly, keeping the heading
-- `# SKILL ACTIVA: MENUS` as its first line.
--
-- Known previous texts (one guard each):
--
--   1) the fragment seeded by `20261109000001_oximeal_recipe_instructions.sql`
--      (the current default, which already asks for the preparation).
--   2) the fragment seeded by `20261107000001_oximeal.sql` (the original, which
--      still names only `name`, `category` and `ingredients`).
--
-- An absent key stays absent (the seeding migration creates it), and a
-- present-but-blank key is deliberate user content.

-- 1) Fragment seeded by `20261109000001_oximeal_recipe_instructions.sql`.
UPDATE settings
SET value = '# SKILL ACTIVA: MENUS
- Gestiona el recetario y los menús de la casa con la herramienta `oximeal` y no mantengas una lista paralela.
- Lee recetas con `recent_recipes` (las planificadas hace poco) o `search_recipes` (por texto y/o `category`: main, salad, side, dessert, breakfast). La búsqueda de OxiMeal coincide con una **subcadena contigua** del nombre o de las instrucciones, no con palabras sueltas: usa términos **cortos y distintivos** (por ejemplo «mozzarella» o «calabaza») y, si buscas con varias palabras, Valet combina los resultados de cada término.
- Planifica la semana con `plan_week`: cada entrada lleva `date` (YYYY-MM-DD), `meal_type` (breakfast, lunch, dinner o snack) y `recipe_id`; `role` (main, salad, side) y `servings` son opcionales. Devuelve la lista de la compra reconstruida.
- Gestiona la compra con `add_shopping_items` (suma ingredientes: `name`, `amount`, `unit` y, si procede, `category`) y con `clear_purchased` (purga lo ya comprado).
- Guarda recetas nuevas con `create_recipe`, cuyo `recipe` ha de traer `name`, `category` e `ingredients` no vacíos y **siempre** la preparación: el texto de elaboración en `instructions` y el tiempo de preparación en minutos en `prep_time_minutes`. Una receta guardada sin instrucciones es una ficha de la compra a medias: no la dejes así.
- `plan_week` y `clear_purchased` exigen aprobación explícita porque reemplazan el plan o destruyen lo comprado; `add_shopping_items` y `create_recipe` avisan.
- El `recipe_id` de `plan_week` procede de `search_recipes` o del `id` que devuelve `create_recipe`: no los inventes.',
    updated_at = datetime('now')
WHERE key = 'SKILL_MENUS_PROMPT'
  AND value = '# SKILL ACTIVA: MENUS
- Gestiona el recetario y los menús de la casa con la herramienta `oximeal` y no mantengas una lista paralela.
- Lee recetas con `recent_recipes` (las planificadas hace poco) o `search_recipes` (por texto y/o `category`: main, salad, side, dessert, breakfast).
- Planifica la semana con `plan_week`: cada entrada lleva `date` (YYYY-MM-DD), `meal_type` (breakfast, lunch, dinner o snack) y `recipe_id`; `role` (main, salad, side) y `servings` son opcionales. Devuelve la lista de la compra reconstruida.
- Gestiona la compra con `add_shopping_items` (suma ingredientes: `name`, `amount`, `unit` y, si procede, `category`) y con `clear_purchased` (purga lo ya comprado).
- Guarda recetas nuevas con `create_recipe`, cuyo `recipe` ha de traer `name`, `category` e `ingredients` no vacíos y **siempre** la preparación: el texto de elaboración en `instructions` y el tiempo de preparación en minutos en `prep_time_minutes`. Una receta guardada sin instrucciones es una ficha de la compra a medias: no la dejes así.
- `plan_week` y `clear_purchased` exigen aprobación explícita porque reemplazan el plan o destruyen lo comprado; `add_shopping_items` y `create_recipe` avisan.
- Los ids de receta se obtienen antes con `recent_recipes` o `search_recipes`: no los inventes.';

-- 2) Fragment seeded by `20261107000001_oximeal.sql` (original).
UPDATE settings
SET value = '# SKILL ACTIVA: MENUS
- Gestiona el recetario y los menús de la casa con la herramienta `oximeal` y no mantengas una lista paralela.
- Lee recetas con `recent_recipes` (las planificadas hace poco) o `search_recipes` (por texto y/o `category`: main, salad, side, dessert, breakfast). La búsqueda de OxiMeal coincide con una **subcadena contigua** del nombre o de las instrucciones, no con palabras sueltas: usa términos **cortos y distintivos** (por ejemplo «mozzarella» o «calabaza») y, si buscas con varias palabras, Valet combina los resultados de cada término.
- Planifica la semana con `plan_week`: cada entrada lleva `date` (YYYY-MM-DD), `meal_type` (breakfast, lunch, dinner o snack) y `recipe_id`; `role` (main, salad, side) y `servings` son opcionales. Devuelve la lista de la compra reconstruida.
- Gestiona la compra con `add_shopping_items` (suma ingredientes: `name`, `amount`, `unit` y, si procede, `category`) y con `clear_purchased` (purga lo ya comprado).
- Guarda recetas nuevas con `create_recipe`, cuyo `recipe` ha de traer `name`, `category` e `ingredients` no vacíos y **siempre** la preparación: el texto de elaboración en `instructions` y el tiempo de preparación en minutos en `prep_time_minutes`. Una receta guardada sin instrucciones es una ficha de la compra a medias: no la dejes así.
- `plan_week` y `clear_purchased` exigen aprobación explícita porque reemplazan el plan o destruyen lo comprado; `add_shopping_items` y `create_recipe` avisan.
- El `recipe_id` de `plan_week` procede de `search_recipes` o del `id` que devuelve `create_recipe`: no los inventes.',
    updated_at = datetime('now')
WHERE key = 'SKILL_MENUS_PROMPT'
  AND value = '# SKILL ACTIVA: MENUS
- Gestiona el recetario y los menús de la casa con la herramienta `oximeal` y no mantengas una lista paralela.
- Lee recetas con `recent_recipes` (las planificadas hace poco) o `search_recipes` (por texto y/o `category`: main, salad, side, dessert, breakfast).
- Planifica la semana con `plan_week`: cada entrada lleva `date` (YYYY-MM-DD), `meal_type` (breakfast, lunch, dinner o snack) y `recipe_id`; `role` (main, salad, side) y `servings` son opcionales. Devuelve la lista de la compra reconstruida.
- Gestiona la compra con `add_shopping_items` (suma ingredientes: `name`, `amount`, `unit` y, si procede, `category`) y con `clear_purchased` (purga lo ya comprado).
- Guarda recetas nuevas con `create_recipe`, cuyo `recipe` ha de traer `name`, `category` e `ingredients` no vacíos.
- `plan_week` y `clear_purchased` exigen aprobación explícita porque reemplazan el plan o destruyen lo comprado; `add_shopping_items` y `create_recipe` avisan.
- Los ids de receta se obtienen antes con `recent_recipes` o `search_recipes`: no los inventes.';
