## ADDED Requirements

### Requirement: Una migración SHALL añadir a `SKILL_MENUS_PROMPT` la guía de búsqueda de recetas, con guarda byte a byte

Una migración nueva, aditiva e idempotente SHALL actualizar el valor de `SKILL_MENUS_PROMPT` para que indique que la búsqueda de recetas es por **subcadena contigua** y que conviene usar términos cortos y distintivos, y que el `recipe_id` de `plan_week` procede de `search_recipes` o de `create_recipe`. La migración SHALL conservar `# SKILL ACTIVA: MENUS` como **primera línea** y SHALL sustituir **solo** los textos conocidos de la versión anterior del fragmento (guarda **byte a byte**): si el valor guardado no coincide con ninguno de los textos reconocidos —porque el usuario lo editó—, la migración SHALL NOT pisarlo. No SHALL tocar ningún otro ajuste.

#### Scenario: La migración actualiza el fragmento por defecto
- **Given** `SKILL_MENUS_PROMPT` con el texto por defecto de la versión anterior
- **When** se aplica la migración
- **Then** el valor pasa al fragmento nuevo
- **And** conserva `# SKILL ACTIVA: MENUS` como primera línea

#### Scenario: La migración respeta una edición del usuario
- **Given** `SKILL_MENUS_PROMPT` con un texto distinto al por defecto (editado por el usuario)
- **When** se aplica la migración
- **Then** el valor no cambia
