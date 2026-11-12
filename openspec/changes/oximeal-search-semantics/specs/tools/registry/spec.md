## ADDED Requirements

### Requirement: El esquema de `search_recipes` SHALL documentar la semántica de la consulta y la herramienta SHALL avisar cuando no hay coincidencias

La definición de la herramienta `oximeal` SHALL documentar, en la descripción del parámetro `query` de la operación `search_recipes`, que la búsqueda de OxiMeal coincide con una **subcadena contigua** del nombre o de las instrucciones (no con palabras sueltas ni con combinaciones), que Valet **combina varios términos** de la consulta, y que conviene usar fragmentos cortos y distintivos.

Cuando `search_recipes` no devuelve ninguna receta, la herramienta SHALL responder con un mensaje en español que explique la semántica de la búsqueda y sugiera un término más corto y distintivo, en lugar de una lista vacía sin explicación.

#### Scenario: El esquema de `search_recipes` documenta la semántica
- **Given** la definición de la herramienta `oximeal` que se ofrece al modelo
- **When** se inspecciona la descripción del parámetro `query`
- **Then** menciona que la búsqueda es por subcadena contigua
- **And** indica que varios términos se combinan

#### Scenario: Una búsqueda sin resultados explica cómo buscar
- **Given** una llamada `search_recipes` cuyo término no coincide con ninguna receta
- **When** se ejecuta
- **Then** el resultado es un mensaje que explica la semántica de la búsqueda
- **And** sugiere probar con un término más corto y distintivo
