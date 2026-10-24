## MODIFIED Requirements

### Requirement: Paginación hacia atrás con cursor

`list_all` SHALL soportar paginación hacia atrás mediante un cursor **opaco** que representa un **orden total sin empates** de la tabla de mensajes: la pareja `(created_at, id)`. `list_all` SHALL devolver los mensajes estrictamente anteriores al cursor en ese orden, devueltos en orden ascendente, y un nuevo cursor con la pareja del mensaje más antiguo del lote devuelto, o `None` si no hay más mensajes. El orden de la tabla —con cursor y sin él— SHALL ser `created_at` ascendente y, a igualdad de `created_at`, `id` ascendente, de modo que dos mensajes con la misma marca temporal (la columna `created_at` tiene resolución de segundo) queden en un orden determinista y ninguno se pierda entre páginas ni en un replay. El cursor SHALL NOT exponerse como una marca temporal suelta.

**Given** se ha cargado una página de mensajes
**When** se llama `list_all(pool, limit, Some(cursor))`
**Then** devuelve los mensajes anteriores al cursor, en orden cronológico ascendente
**And** si no hay más mensajes, `next_cursor` es None

#### Scenario: Dos páginas
**Given** una DB con 55 mensajes
**When** página 1: `list_all(pool, 50, None)` → 50 mensajes, cursor no nulo
**And** página 2: `list_all(pool, 50, Some(cursor))` → 5 mensajes
**Then** todos los 55 mensajes se han devuelto entre ambas páginas

#### Scenario: DB vacía
**Given** una DB sin mensajes
**When** se llama `list_all(pool, 50, None)`
**Then** devuelve `([], None)`

#### Scenario: Dos mensajes con la misma marca temporal no se pierden
**Given** una DB con dos mensajes persistidos en el mismo segundo
**When** se pide una página de un mensaje desde el principio y luego la página siguiente con el cursor devuelto
**Then** entre las dos páginas aparecen los dos mensajes
**And** ninguno se repite

#### Scenario: El orden es determinista a igualdad de marca temporal
**Given** una DB con dos mensajes persistidos en el mismo segundo
**When** se llama `list_all(pool, 50, None)` dos veces
**Then** los dos mensajes aparecen en el mismo orden en ambas llamadas

## ADDED Requirements

### Requirement: MessagesRepo::list_after SHALL devolver los mensajes posteriores a un cursor

`list_after(pool, cursor, limit)` SHALL devolver hasta `limit` mensajes **estrictamente posteriores** al cursor en el orden total `(created_at, id)`, en orden ascendente, junto al cursor del último mensaje devuelto. Cuando no haya mensajes posteriores SHALL devolver una lista vacía y `None`, de modo que quien llama conserve el cursor que ya tenía. Se usa para el replay del stream de eventos, que necesita la dirección opuesta a la de `list_all`.

#### Scenario: Devuelve los posteriores en orden ascendente
**Given** una DB con tres mensajes m1, m2 y m3 en orden de inserción
**And** el cursor de m1
**When** se llama `list_after(pool, cursor_de_m1, 50)`
**Then** devuelve m2 y m3, en ese orden
**And** el cursor devuelto es el de m3

#### Scenario: Sin posteriores devuelve vacío y conserva el cursor
**Given** una DB con dos mensajes m1 y m2
**And** el cursor de m2
**When** se llama `list_after(pool, cursor_de_m2, 50)`
**Then** devuelve una lista vacía
**And** el cursor devuelto es `None`

#### Scenario: Un límite acota el replay y encadena cursores
**Given** una DB con tres mensajes posteriores al cursor
**When** se llama `list_after(pool, cursor, 2)`
**Then** devuelve dos mensajes
**And** el cursor devuelto permite continuar y llegar al tercero

#### Scenario: Los empates de marca temporal no se pierden
**Given** una DB con dos mensajes persistidos en el mismo segundo, ambos posteriores al cursor
**When** se llama `list_after(pool, cursor, 50)`
**Then** devuelve los dos mensajes
**And** ninguno se repite con respecto a la llamada siguiente

---
