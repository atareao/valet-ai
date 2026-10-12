## ADDED Requirements

### Requirement: La migración SHALL añadir `location` y un índice único por hecho y día a `timeline_events`

Una migración nueva SHALL añadir a `timeline_events` la columna `location` —TEXT, admite `NULL`— y SHALL
crear un índice **único** sobre la pareja formada por el hecho normalizado (`lower(trim(fact))`) y el día
de su `timestamp` (`substr(timestamp, 1, 10)`). Con ese índice en pie, dos hechos con el mismo texto
normalizado el mismo día no pueden coexistir: la base de datos rechaza el segundo. Tras la migración,
`timeline_events` tiene las columnas `id`, `timestamp`, `category`, `fact`, `location`,
`source_message_id` y `created_at`. La tabla queda sin filas: la migración la vacía.

#### Scenario: La columna `location` se añade y admite nulos
- **Given** una base de datos con `timeline_events` ya creada
- **When** se ejecuta la migración
- **Then** la tabla tiene la columna `location`
- **And** un hecho sin lugar la guarda a `NULL`

#### Scenario: El índice único rechaza un hecho repetido el mismo día
- **Given** un hecho registrado con `fact = 'Cené mochis'` y fecha 2026-10-11
- **When** se intenta insertar otro con el mismo `fact` y la misma fecha
- **Then** la base de datos rechaza la segunda fila
- **And** la primera sigue intacta

#### Scenario: El mismo hecho en días distintos es un hecho distinto
- **Given** un hecho registrado con `fact = 'Cené mochis'` y fecha 2026-10-11
- **When** se inserta otro con el mismo `fact` y fecha 2026-10-12
- **Then** las dos filas coexisten

#### Scenario: La normalización ignora espacios y mayúsculas
- **Given** un hecho registrado con `fact = 'Corrí 9,34 km'` y fecha 2026-10-11
- **When** se inserta otro con `fact = '  corrí 9,34 km  '` y la misma fecha
- **Then** la base de datos rechaza la segunda fila

### Requirement: La migración SHALL vaciar `timeline_events` antes de crear el índice único

La migración SHALL eliminar **todas** las filas de `timeline_events`. Los hechos guardados hasta ahora
son derivados —los escribió el extractor, no el usuario—, sus fechas son las de la mención y no las del
acontecimiento, ninguno tiene lugar y más de un tercio son duplicados: no hay nada que conservar. El
material crudo sigue en `messages`, así que la tabla es reconstruible. El vaciado SHALL ejecutarse
**antes** de crear el índice único, que de otro modo fallaría sobre los duplicados existentes, y SHALL
NOT tocar ninguna otra tabla.

#### Scenario: La tabla queda vacía
- **Given** una base de datos con filas en `timeline_events`
- **When** se ejecuta la migración
- **Then** `timeline_events` no tiene ninguna fila

#### Scenario: El vaciado no toca los mensajes
- **Given** una base de datos con mensajes y con hechos
- **When** se ejecuta la migración
- **Then** `messages` conserva todas sus filas

#### Scenario: El vaciado deja el índice único creable
- **Given** una base de datos con hechos duplicados que harían fallar el índice único
- **When** se ejecuta la migración
- **Then** la migración termina sin error
- **And** el índice único existe

### Requirement: La migración SHALL actualizar el prompt del extractor solo si nadie lo ha personalizado

La migración SHALL reemplazar el valor de `timeline_prompt` y de `SKILL_TIMELINE_PROMPT` **solo** cuando
el valor vigente sea exactamente el sembrado por la migración `20261010000004_timeline_prompts.sql`. Si
el valor es cualquier otro —una personalización del usuario— SHALL dejarlo intacto. Una clave ausente
SHALL rellenarse con el valor nuevo.

#### Scenario: El prompt sin tocar se actualiza
- **Given** `timeline_prompt` con el valor sembrado por la migración anterior
- **When** se ejecuta la migración
- **Then** `timeline_prompt` pasa a ser el prompt nuevo

#### Scenario: Una personalización se respeta
- **Given** `timeline_prompt` con un valor distinto del sembrado
- **When** se ejecuta la migración
- **Then** `timeline_prompt` conserva su valor

#### Scenario: Una clave ausente se rellena
- **Given** un `settings` sin la clave `timeline_prompt`
- **When** se ejecuta la migración
- **Then** `timeline_prompt` existe con el prompt nuevo
