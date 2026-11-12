## MODIFIED Requirements

### Requirement: El servicio SHALL exponer las seis operaciones de la Agent API

El servicio SHALL implementar exactamente estas operaciones, contra los endpoints de la Agent API de OxiMeal:

| Operación | Endpoint | Forma |
|---|---|---|
| `recent_recipes(days)` | `GET /api/agent/recent-recipes?days=<u32>` | `days` por defecto 7, acotado a `1..=365` |
| `search_recipes(query, category)` | `GET /api/agent/search-recipes?query=&category=` | devuelve como mucho 10 recetas |
| `set_weekly_plan(entries)` | `POST /api/agent/weekly-plan` | cuerpo `{"entries":[…]}` con `date`, `meal_type`, `recipe_id`, `role`, `servings` |
| `add_shopping_items(items)` | `POST /api/agent/shopping-items` | cuerpo `{"items":[…]}` con `name`, `amount`, `unit`, `category?` |
| `delete_completed()` | `DELETE /api/agent/shopping/completed` | — |
| `create_recipe(input)` | `POST /api/agent/recipes` | cuerpo `RecipeInput` (`name`, `category`, `ingredients` obligatorios) |

Un `400` del servidor SHALL traducirse a `InvalidInput` conservando el **mensaje humano** del servidor (que nunca contiene credenciales). Al extraer ese mensaje de un cuerpo de error en JSON, el servicio SHALL preferir los campos en el orden `message`, `error`, `detail`, `title`: `error` es a menudo un código máquina (`validation`, `not_found`) y NO SHALL desplazar a `message`; si el cuerpo no es JSON legible, SHALL usar el propio cuerpo (recortado) como detalle. Un `404` SHALL traducirse a `NotFound`; un `5xx` o un fallo de red SHALL traducirse a `Upstream`.

#### Scenario: La búsqueda de recetas usa query y categoría
- **Given** una llamada `search_recipes("pasta", "main")`
- **When** se ejecuta
- **Then** la petición va a `GET /api/agent/search-recipes` con `query=pasta` y `category=main`
- **And** la respuesta se devuelve como la lista de recetas recibida

#### Scenario: El plan semanal envía entradas estructuradas
- **Given** una llamada `set_weekly_plan` con una entrada `{date:"2026-11-09", meal_type:"dinner", recipe_id:"abc", role:"main", servings:4}`
- **When** se ejecuta
- **Then** el cuerpo enviado a `POST /api/agent/weekly-plan` es `{"entries":[{"date":"2026-11-09","meal_type":"dinner","recipe_id":"abc","role":"main","servings":4}]}`
- **And** la respuesta incluye `created` y `shopping_list`

#### Scenario: Un 400 del servidor conserva su mensaje
- **Given** la Agent API respondiendo `400` con `{"error":"validation","message":"datos inválidos: categoría inválida: foo"}`
- **When** se ejecuta `create_recipe` con esa categoría
- **Then** el error es `InvalidInput` con el mensaje humano del servidor (contiene `categoría inválida: foo`)
- **And** el detalle NO es el código máquina `validation`
- **And** el secreto NO aparece en el error

## ADDED Requirements

### Requirement: La decodificación de las listas de recetas SHALL tolerar las etiquetas objeto de OxiMeal

La decodificación de las listas de recetas (`recent_recipes` y `search_recipes`) SHALL ser tolerante a la forma real de las etiquetas de OxiMeal: cada receta puede traer `tags` como una lista de **objetos** `{id, name, slug, source}` (no de cadenas), y el servicio SHALL decodificarlas sin fallar. La representación interna SHALL quedarse con el **nombre** de cada etiqueta, y la salida de la herramienta SHALL seguir exponiendo una lista de nombres.

#### Scenario: Las recetas con etiquetas objeto se decodifican
- **Given** la Agent API devolviendo `{"recipes":[{"id":"abc","name":"Ensalada","category":"salad","diet_type":"mediterranean","prep_time_minutes":10,"servings":2,"tags":[{"id":"t1","name":"ligera","slug":"ligera","source":"auto"}]}]}`
- **When** se ejecuta `search_recipes` (o `recent_recipes`)
- **Then** la operación completa sin error
- **And** la etiqueta se conserva por su `name` (`ligera`)

#### Scenario: Una receta sin etiquetas se decodifica igual
- **Given** la Agent API devolviendo una receta sin el campo `tags` (o con `tags: []`)
- **When** se ejecuta `search_recipes` (o `recent_recipes`)
- **Then** la operación completa sin error
- **And** la receta aparece sin etiquetas
