# Spec Delta: db/repos

## ADDED Requirements

### Requirement: ProfilesRepo::get_by_id SHALL return the profile or None

El repositorio SHALL exponer `ProfilesRepo::get_by_id(pool, id)` que devuelve el perfil cuyo `id`
coincide, o `None` si no existe. NO SHALL insertar ni modificar filas.

#### Scenario: Perfil existente
**Given** la tabla `profiles` con una fila `id = "p1"` y `name = "Lorenzo"`  
**When** se llama `ProfilesRepo::get_by_id(pool, "p1")`  
**Then** devuelve `Some(profile)` con `name = "Lorenzo"`

#### Scenario: Perfil inexistente
**Given** la tabla `profiles` sin la fila `id = "nope"`  
**When** se llama `ProfilesRepo::get_by_id(pool, "nope")`  
**Then** devuelve `None`  
**And** no se inserta ninguna fila
