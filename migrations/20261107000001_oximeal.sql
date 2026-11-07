-- Migración de la integración con OxiMeal (`oximeal-integration`).
--
-- Aditiva e idempotente, con el mismo idioma que las migraciones hermanas
-- (`20261027000001_opentodo.sql`, `20261026000001_zennotes_skill.sql`): solo se
-- rellena un valor ausente, vacío o NULL, así que una personalización del usuario
-- se conserva.
--
-- Why each block:
--
--   1) `oximeal_base_url`: la URL de la instancia de OxiMeal, con su valor por
--      defecto. Es un ajuste normal (no sensible): viaja y se edita por
--      `GET`/`PUT /api/settings`. Espeja el respaldo `OXIMEAL_BASE_URL`.
--
--   2) `oximeal_issuer_url`: se siembra **vacío** a propósito; el cliente M2M usa
--      el mismo IdP que Valet y, mientras esté vacío, cae a `AUTH_ISSUER_URL`.
--      Espeja el respaldo `OXIMEAL_ISSUER_URL`.
--
--   3) `oximeal_client_id`: se siembra **vacío** a propósito; el usuario lo pega
--      en Ajustes → Integraciones → OxiMeal. Espeja `OXIMEAL_CLIENT_ID`.
--
--   **NO** se siembra `oximeal_client_secret`: es material sensible
--   (`SENSITIVE_KEYS`) y su única puerta de escritura es
--   `PUT /api/oximeal/secret`, de modo que nunca acaba en el repositorio ni en el
--   historial de migraciones.
--
--   4) `SKILL_MENUS_PROMPT`: fragmento de la skill `menus`, con
--      `# SKILL ACTIVA: MENUS` como primera línea (el `prompt_heading` que usa el
--      ensamblador de fragmentos para no duplicarlo). Solo se rellena cuando falta,
--      está vacío o es NULL.

-- 1) URL base de OxiMeal. Solo se siembra cuando falta.
INSERT INTO settings (key, value, updated_at)
VALUES ('oximeal_base_url', 'https://oximeal.territoriolinux.es', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Emisor OIDC: vacío a propósito; cae a AUTH_ISSUER_URL.
INSERT INTO settings (key, value, updated_at)
VALUES ('oximeal_issuer_url', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 3) client_id: vacío a propósito; lo rellena el usuario en la UI.
INSERT INTO settings (key, value, updated_at)
VALUES ('oximeal_client_id', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 4) Fragmento de la skill `menus` (re-ejecutable; conserva una edición del usuario).
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_MENUS_PROMPT', '# SKILL ACTIVA: MENUS
- Gestiona el recetario y los menús de la casa con la herramienta `oximeal` y no mantengas una lista paralela.
- Lee recetas con `recent_recipes` (las planificadas hace poco) o `search_recipes` (por texto y/o `category`: main, salad, side, dessert, breakfast).
- Planifica la semana con `plan_week`: cada entrada lleva `date` (YYYY-MM-DD), `meal_type` (breakfast, lunch, dinner o snack) y `recipe_id`; `role` (main, salad, side) y `servings` son opcionales. Devuelve la lista de la compra reconstruida.
- Gestiona la compra con `add_shopping_items` (suma ingredientes: `name`, `amount`, `unit` y, si procede, `category`) y con `clear_purchased` (purga lo ya comprado).
- Guarda recetas nuevas con `create_recipe`, cuyo `recipe` ha de traer `name`, `category` e `ingredients` no vacíos.
- `plan_week` y `clear_purchased` exigen aprobación explícita porque reemplazan el plan o destruyen lo comprado; `add_shopping_items` y `create_recipe` avisan.
- Los ids de receta se obtienen antes con `recent_recipes` o `search_recipes`: no los inventes.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
