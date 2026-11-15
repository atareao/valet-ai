## ADDED Requirements

### Requirement: La búsqueda de recetas SHALL combinar los términos de la consulta

El servicio SHALL interpretar el parámetro `query` de `search_recipes` como una consulta **de términos**, no como una frase, porque la búsqueda de OxiMeal coincide con una **subcadena contigua** del nombre o de las instrucciones (FTS5 `trigram`, frase entre comillas). Con un único término, el servicio SHALL hacer **una** petición a `GET /api/agent/search-recipes`. Con varios términos, SHALL hacer **una petición por término** (aplicando el mismo `category` a todas) y combinar los resultados en memoria: SHALL devolver primero las recetas que coinciden con **todos** los términos (intersección por `id`) y, si esa intersección está vacía, las que coinciden con **más** términos, por orden descendente de coincidencias. El resultado SHALL deduplicarse por `id` y quedar acotado al máximo de la operación (10 recetas). El número de términos considerados SHALL estar acotado por una constante; los términos sobrantes SHALL ignorarse, sin fallar. La petición de cada término SHALL poder lanzarse en paralelo.

#### Scenario: Una consulta de un término sigue haciendo una sola petición
- **Given** una llamada `search_recipes("mozzarella", None)`
- **When** se ejecuta
- **Then** se hace **una** petición a `GET /api/agent/search-recipes` con `query=mozzarella`
- **And** se devuelve la lista recibida

#### Scenario: Una consulta de varios términos combina los resultados
- **Given** una llamada `search_recipes("tomate aguacate mozzarella", None)`
- **And** OxiMeal devuelve una receta `X` para cada uno de los tres términos, más otras recetas que solo coinciden con uno
- **When** se ejecuta
- **Then** se hace una petición por término
- **And** `X`, que coincide con los tres, aparece en primer lugar
- **And** el resultado no contiene ids duplicados
- **And** el resultado tiene como mucho 10 recetas

#### Scenario: Si ningún término coincide con todos, se ordena por número de coincidencias
- **Given** una llamada `search_recipes("salmón espárragos ahumado", None)`
- **And** ninguna receta coincide con los tres términos, pero una coincide con dos y otras con uno
- **When** se ejecuta
- **Then** la receta que coincide con dos términos aparece antes que las que coinciden con uno

#### Scenario: Los términos sobrantes se ignoran sin fallar
- **Given** una consulta con más términos que el máximo considerado
- **When** se ejecuta
- **Then** la operación completa sin error
- **And** solo se usan los primeros términos hasta el máximo
