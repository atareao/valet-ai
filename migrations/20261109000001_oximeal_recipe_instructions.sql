-- OxiMeal recipe instructions (oximeal-recipe-instructions, task 2.4).
--
-- Same idiom as the previous prompt migrations (`20261104000001_widget_options_contract.sql`,
-- `20261012000003_pendientes_notifications_prompt.sql`): additive and idempotent,
-- with a **byte-for-byte guard**. Only the exact texts a previous release seeded
-- are replaced; any other value is user content and is never touched. No other
-- setting is modified.
--
-- Why: Valet created recipes with `name`, `category` and `ingredients` only, so
-- they landed in OxiMeal as shopping lists without any preparation. OxiMeal's
-- Agent API already accepts `instructions` and `prep_time_minutes` in its
-- `RecipeInput`; the gap was Valet's contract (tool schema + this fragment), not
-- OxiMeal's. This migration updates the `menus` skill fragment to ask for both,
-- keeping the heading `# SKILL ACTIVA: MENUS` as its first line.
--
-- Known previous texts (one guard each):
--
--   1) the fragment seeded by `20261107000001_oximeal.sql`, which still names
--      only `name`, `category` and `ingredients`.
--
-- An absent key stays absent (the seeding migration creates it), and a
-- present-but-blank key is deliberate user content.

UPDATE settings
SET value = '# SKILL ACTIVA: MENUS
- Gestiona el recetario y los menús de la casa con la herramienta `oximeal` y no mantengas una lista paralela.
- Lee recetas con `recent_recipes` (las planificadas hace poco) o `search_recipes` (por texto y/o `category`: main, salad, side, dessert, breakfast).
- Planifica la semana con `plan_week`: cada entrada lleva `date` (YYYY-MM-DD), `meal_type` (breakfast, lunch, dinner o snack) y `recipe_id`; `role` (main, salad, side) y `servings` son opcionales. Devuelve la lista de la compra reconstruida.
- Gestiona la compra con `add_shopping_items` (suma ingredientes: `name`, `amount`, `unit` y, si procede, `category`) y con `clear_purchased` (purga lo ya comprado).
- Guarda recetas nuevas con `create_recipe`, cuyo `recipe` ha de traer `name`, `category` e `ingredients` no vacíos y **siempre** la preparación: el texto de elaboración en `instructions` y el tiempo de preparación en minutos en `prep_time_minutes`. Una receta guardada sin instrucciones es una ficha de la compra a medias: no la dejes así.
- `plan_week` y `clear_purchased` exigen aprobación explícita porque reemplazan el plan o destruyen lo comprado; `add_shopping_items` y `create_recipe` avisan.
- Los ids de receta se obtienen antes con `recent_recipes` o `search_recipes`: no los inventes.',
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
