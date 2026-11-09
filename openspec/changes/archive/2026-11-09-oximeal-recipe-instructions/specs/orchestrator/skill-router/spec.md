# orchestrator/skill-router — Delta de especificación (change oximeal-recipe-instructions)

## ADDED Requirements

### Requirement: El fragmento de la skill `menus` SHALL pedir las instrucciones de elaboración y el tiempo de preparación al guardar recetas

El fragmento de prompt de la skill `menus` (`SKILL_MENUS_PROMPT`, encabezado `# SKILL ACTIVA: MENUS`) SHALL indicar al modelo que, al guardar una receta con `create_recipe`, incluya las **instrucciones de elaboración** en el campo `instructions` del `recipe` y el **tiempo de preparación** en `prep_time_minutes` —además de `name`, `category` e `ingredients`—, de modo que las recetas no se guarden como fichas de la compra sin preparación. El fragmento SHALL seguir exigiendo que `recipe` traiga `name`, `category` e `ingredients` no vacíos.

#### Scenario: El fragmento sembrado pide las instrucciones y el tiempo
- **Given** el valor por defecto de `SKILL_MENUS_PROMPT` con su encabezado `# SKILL ACTIVA: MENUS`
- **When** se inspecciona su contenido
- **Then** menciona el campo `instructions` como parte de lo que debe llevar `recipe` en `create_recipe`
- **And** menciona `prep_time_minutes` como parte de lo que debe llevar `recipe`
- **And** mantiene la exigencia de `name`, `category` e `ingredients`

#### Scenario: El fragmento de la migración y el valor en la base de datos coinciden
- **Given** el literal de la migración que siembra `SKILL_MENUS_PROMPT` y el valor resultante en la base de datos
- **When** se comparan
- **Then** coinciden
- **And** el literal reconoce la versión anterior del fragmento para la guarda byte a byte
