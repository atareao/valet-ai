# tools/registry — Delta de especificación (change oximeal-recipe-instructions)

## ADDED Requirements

### Requirement: La definición de la herramienta `oximeal` SHALL documentar los campos opcionales de `RecipeInput` para `create_recipe`

La definición de la herramienta `oximeal` que se envía al modelo SHALL documentar, en el esquema del parámetro `recipe` de la operación `create_recipe`, además de los obligatorios `name`, `category` e `ingredients`, los campos **opcionales** que la Agent API de OxiMeal acepta en su `RecipeInput`: como mínimo `instructions` (instrucciones de elaboración), y también `prep_time_minutes`, `servings`, `diet_type`, `source_url` y `tags`. La descripción de `instructions` SHALL dejar claro que es texto libre con la preparación. La validación de argumentos de `create_recipe` SHALL seguir exigiendo `name` y `category` no vacíos y un array `ingredients` no vacío (con `name` no vacío y `amount` no negativo si viene), y SHALL **aceptar** `instructions` sin salir a la red.

#### Scenario: El esquema de `create_recipe` documenta las instrucciones
- **Given** la definición de la herramienta `oximeal` que se ofrece al modelo
- **When** se inspecciona el esquema de la propiedad `recipe`
- **Then** las propiedades incluyen `instructions` entre los campos opcionales
- **And** `name`, `category` e `ingredients` figuran como obligatorios

#### Scenario: La validación acepta instrucciones sin tocar la red
- **Given** una llamada `create_recipe` con `recipe` que trae `name`, `category`, `ingredients` y un `instructions` no vacío
- **When** se valida con la lógica de argumentos de la herramienta
- **Then** la validación pasa
- **And** no se realiza ninguna petición de red durante la validación

#### Scenario: La validación sigue rechazando lo que OxiMeal rechazaría
- **Given** una llamada `create_recipe` con `recipe` sin `ingredients` o con `ingredients` vacío
- **When** se valida con la lógica de argumentos de la herramienta
- **Then** la validación falla con un error de argumentos inválidos
- **And** no se realiza ninguna petición de red
