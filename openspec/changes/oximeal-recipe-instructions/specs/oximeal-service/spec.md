# oximeal-service — Delta de especificación (change oximeal-recipe-instructions)

## ADDED Requirements

### Requirement: El servicio SHALL transmitir sin alterar los campos opcionales de `RecipeInput` en `create_recipe`

La operación `create_recipe` del servicio OxiMeal SHALL enviar a `POST /api/agent/recipes`, como cuerpo, el objeto `RecipeInput` **tal cual** lo recibe, sin descartar ni transformar los campos opcionales que la Agent API admite —entre ellos `instructions`, `prep_time_minutes`, `servings`, `diet_type`, `source_url` y `tags`—. La operación SHALL seguir enviando `name`, `category` e `ingredients` y SHALL NOT exigir la presencia de `instructions`.

#### Scenario: Las instrucciones viajan en el cuerpo de la petición
- **Given** una llamada `create_recipe` con un `RecipeInput` que incluye `instructions: "Cortar y servir."`
- **When** se ejecuta
- **Then** el cuerpo enviado a `POST /api/agent/recipes` contiene `instructions: "Cortar y servir."`
- **And** el resto de campos enviados coincide con los recibidos

#### Scenario: Sin instrucciones la receta se sigue creando
- **Given** una llamada `create_recipe` con un `RecipeInput` sin `instructions`
- **When** se ejecuta
- **Then** el cuerpo enviado NO incluye `instructions`
- **And** la petición se completa igualmente
