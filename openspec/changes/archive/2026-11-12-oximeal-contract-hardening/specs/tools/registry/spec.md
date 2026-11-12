## ADDED Requirements

### Requirement: La definición de la herramienta `oximeal` SHALL documentar la procedencia del `recipe_id` de `plan_week`

La definición de la herramienta `oximeal` que se envía al modelo SHALL documentar, en el esquema del parámetro `entries` de la operación `plan_week`, que cada `recipe_id` SHALL ser el identificador de una receta **existente** obtenido de `search_recipes` o de `create_recipe` en el mismo turno, y NO un nombre inventado. La descripción SHALL nombrar explícitamente `search_recipes` y `create_recipe` como las fuentes del identificador.

#### Scenario: El esquema de `plan_week` documenta la procedencia del `recipe_id`
- **Given** la definición de la herramienta `oximeal` que se ofrece al modelo
- **When** se inspecciona el esquema de la propiedad `entries`
- **Then** su descripción nombra `search_recipes` y `create_recipe` como las fuentes del `recipe_id`
- **And** indica que el identificador NO se inventa
