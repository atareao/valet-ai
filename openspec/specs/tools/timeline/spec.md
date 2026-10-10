# tools/timeline Specification

## Purpose
Las tres herramientas del timeline de Valet —`timeline_get_events`, `timeline_add_event` y
`timeline_delete_event`—: consultan, anotan y borran los hechos cronológicos del usuario
(`timeline_events`), con aprobación explícita para el borrado.

## Requirements

### Requirement: `timeline_get_events` SHALL consultar los hechos por rango de fechas y categoría

La herramienta `timeline_get_events` SHALL devolver los hechos registrados en `timeline_events` dentro
de un rango de fechas, opcionalmente acotados por categoría. Los parámetros `start` y `end` SHALL ser
fechas ISO 8601 e inclusivas; `category` SHALL ser uno de los siete valores del conjunto cerrado y
`limit` SHALL acotar el número de filas devueltas, con 50 por defecto. El resultado SHALL venir
ordenado de más reciente a más antiguo y cada hecho SHALL llevar su `id`, su `timestamp`, su
`category` y su `fact`.

#### Scenario: Un rango devuelve los hechos del periodo
- **Given** un timeline con hechos de varios días
- **When** se llama a `timeline_get_events` con un `start` y un `end` que acotan un día
- **Then** solo se devuelven los hechos de ese día
- **And** vienen ordenados de más reciente a más antiguo

#### Scenario: El filtro por categoría acota el resultado
- **Given** un rango con hechos de varias categorías
- **When** se llama con `category` igual a `sport`
- **Then** solo se devuelven los hechos de esa categoría

#### Scenario: Una categoría fuera del conjunto es un argumento inválido
- **Given** una llamada con una `category` que no pertenece al conjunto cerrado
- **When** se ejecuta
- **Then** la herramienta devuelve un error de argumentos inválidos
- **And** no consulta la base de datos

#### Scenario: El límite acota y se aplica a lo más reciente
- **Given** un timeline con más hechos que el `limit` pedido y sin rango
- **When** se ejecuta
- **Then** se devuelven a lo sumo `limit` hechos
- **And** son los más recientes

### Requirement: `timeline_add_event` SHALL registrar un hecho con la hora actual por defecto

La herramienta `timeline_add_event` SHALL insertar un hecho en `timeline_events` a partir de un `fact`
obligatorio, una `category` opcional —`lifestyle` por defecto— y un `timestamp` opcional que, si se
omite, SHALL ser la hora actual en UTC. La herramienta SHALL validar que el `fact` no esté vacío ni en
blanco y SHALL rechazar una `category` fuera del conjunto cerrado. Una inserción correcta SHALL
devolver el identificador del hecho creado.

#### Scenario: Sin timestamp se usa la hora actual
- **Given** una llamada con solo el `fact`
- **When** se ejecuta
- **Then** se inserta con la categoría `lifestyle`
- **And** el `timestamp` es la hora actual en UTC
- **And** se devuelve el identificador del hecho

#### Scenario: El timestamp explícito se respeta
- **Given** una llamada con `fact`, `category` y `timestamp`
- **When** se ejecuta
- **Then** el hecho se guarda con esa fecha y esa categoría

#### Scenario: Un fact vacío es un argumento inválido
- **Given** una llamada cuyo `fact` está vacío o solo tiene espacios
- **When** se ejecuta
- **Then** devuelve un error de argumentos inválidos
- **And** no inserta ninguna fila

### Requirement: `timeline_delete_event` SHALL borrar un hecho por su id exigiendo aprobación

La herramienta `timeline_delete_event` SHALL borrar el hecho cuyo `id` se le pasa. El `id` SHALL ser
obligatorio. Cuando el id no corresponda a ningún hecho, la herramienta SHALL NOT fallar: SHALL
informar de que no existe. Al ser una operación destructiva, SHALL declarar
`Permission::ExplicitApproval`.

#### Scenario: Un id correcto borra el hecho
- **Given** un hecho existente con su identificador
- **When** se llama a `timeline_delete_event` con ese id y la acción se aprueba
- **Then** la fila desaparece de `timeline_events`

#### Scenario: Un id inexistente no es un error
- **Given** un identificador que no corresponde a ningún hecho
- **When** se ejecuta
- **Then** la herramienta informa de que no existe
- **And** no falla

#### Scenario: El borrado exige aprobación explícita
- **Given** una llamada a `timeline_delete_event`
- **When** se consulta su permiso
- **Then** es `Permission::ExplicitApproval`

### Requirement: Las tres herramientas del timeline SHALL describirse en español y declarar sus operaciones

Las descripciones de `timeline_get_events`, `timeline_add_event` y `timeline_delete_event` SHALL estar
redactadas en español, SHALL explicar qué devuelve cada una y SHALL enumerar los argumentos
obligatorios. Las tres SHALL exponer un esquema de parámetros de tipo objeto.

#### Scenario: Las descripciones están en español y citan los obligatorios
- **Given** las definiciones de las tres herramientas
- **When** se inspeccionan sus descripciones
- **Then** están en español
- **And** cada una menciona sus argumentos obligatorios
