## MODIFIED Requirements

### Requirement: `timeline_get_events` SHALL consultar los hechos por rango de fechas y categoría

La herramienta `timeline_get_events` SHALL devolver los hechos registrados en `timeline_events` dentro
de un rango de fechas, opcionalmente acotados por categoría. Los parámetros `start` y `end` SHALL ser
fechas ISO 8601 e inclusivas; `category` SHALL ser uno de los siete valores del conjunto cerrado y
`limit` SHALL acotar el número de filas devueltas, con 50 por defecto. El resultado SHALL venir
ordenado de más reciente a más antiguo y cada hecho SHALL llevar su `id`, su `timestamp`, su
`category`, su `fact` y su `location`, que SHALL ser `null` cuando el hecho no tenga lugar.

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

#### Scenario: Cada hecho lleva su lugar, o nulo
- **Given** un timeline con un hecho con `location` y otro sin ella
- **When** se consultan
- **Then** el primero la devuelve con su valor
- **And** el segundo la devuelve a `null`

### Requirement: `timeline_add_event` SHALL registrar un hecho con la hora actual por defecto

La herramienta `timeline_add_event` SHALL insertar un hecho en `timeline_events` a partir de un `fact`
obligatorio, una `category` opcional —`lifestyle` por defecto—, un `timestamp` opcional que, si se
omite, SHALL ser la hora actual en UTC, y un `location` opcional que SHALL quedar a `NULL` cuando falte,
esté en blanco o pase de 120 caracteres. La herramienta SHALL validar que el `fact` no esté vacío ni en
blanco y SHALL rechazar una `category` fuera del conjunto cerrado. Cuando el hecho ya exista en el
timeline para ese mismo día, la inserción SHALL ignorarse sin fallar y la herramienta SHALL informar de
que el hecho ya estaba registrado, sin devolver un identificador nuevo. Una inserción correcta SHALL
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

#### Scenario: El lugar se guarda cuando viene
- **Given** una llamada con `fact` y `location`
- **When** se ejecuta
- **Then** el hecho se guarda con ese lugar

#### Scenario: Un lugar inservible queda a nulo
- **Given** una llamada cuyo `location` está en blanco o pasa de 120 caracteres
- **When** se ejecuta
- **Then** el hecho se guarda con `location` a `NULL`
- **And** el hecho no se pierde

#### Scenario: Un fact vacío es un argumento inválido
- **Given** una llamada cuyo `fact` está vacío o solo tiene espacios
- **When** se ejecuta
- **Then** devuelve un error de argumentos inválidos
- **And** no inserta ninguna fila

#### Scenario: Un hecho repetido el mismo día no se duplica
- **Given** un hecho ya registrado hoy
- **When** se llama a `timeline_add_event` con el mismo `fact` y el mismo día
- **Then** no se inserta una segunda fila
- **And** la herramienta informa de que el hecho ya estaba registrado
