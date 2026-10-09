## MODIFIED Requirements

### Requirement: La petición SHALL abrir con un único mensaje de sistema

La petición al LLM SHALL abrir con **un solo** mensaje `role:"system"`, que reúna en este
orden las secciones presentes: (1) el prompt de `settings.system_prompt`, (2) la sección con el
nombre del usuario, (3) la sección de memoria persistente si la hay, (4) la memoria episódica si
la hay, (5) la sección de ubicación si hay coordenadas, y (6) **la sección temporal, que SHALL
estar siempre presente y cerrar el mensaje**. El mensaje SHALL preceder al historial de
conversación. Las secciones presentes SHALL separarse con una línea en blanco, y ninguna sección
ausente SHALL dejar título, marcador, separador ni línea en blanco. La sección temporal SHALL NOT
considerarse ausente en ningún caso.

**Given** un mensaje del usuario
**When** el orquestador construye la petición al LLM
**Then** la cabecera de la petición SHALL contener exactamente un mensaje `role:"system"` antes del historial
**And** ese mensaje SHALL empezar por el contenido de `settings.system_prompt`
**And** SHALL incluir después, y solo si existen, las secciones de nombre de usuario, memoria persistente, memoria episódica y ubicación, en ese orden
**And** SHALL terminar con la sección temporal, que nunca falta
**And** ninguna sección ausente SHALL dejar título, marcador, separador ni línea en blanco

#### Scenario: Un solo mensaje de sistema
**Given** un orquestador con prompt, fichas inyectables y contexto de navegador
**When** se construye la petición
**Then** hay exactamente un mensaje `role:"system"` antes del historial
**And** ese mensaje contiene el prompt, la sección episódica, la ubicación y la sección temporal

#### Scenario: Orden de las secciones
**Given** un orquestador con prompt, nombre de usuario, memoria persistente no vacía, fichas inyectables y contexto de navegador con coordenadas
**When** se inspecciona el mensaje de sistema
**Then** el prompt aparece antes que la sección con el nombre del usuario
**And** la sección con el nombre del usuario aparece antes que la sección de memoria persistente
**And** la sección de memoria persistente aparece antes que la sección episódica
**And** la sección episódica aparece antes que la sección de ubicación
**And** la sección de ubicación aparece antes que la sección temporal
**And** la sección temporal es la última

#### Scenario: Secciones ausentes sin rastro
**Given** un orquestador sin nombre de usuario, sin fichas inyectables y sin contexto de navegador
**When** se construye la petición
**Then** el mensaje de sistema contiene el prompt y, a continuación, la sección temporal, sin nada intermedio
**And** no aparece ningún título, marcador ni línea en blanco de relleno

#### Scenario: La sección temporal no depende del navegador
**Given** una invocación sin `BrowserContext`
**When** se construye la petición
**Then** el mensaje de sistema termina igualmente con la sección temporal

## REMOVED Requirements

### Requirement: La sección de fecha, hora y ubicación SHALL cerrar el mensaje de sistema

## ADDED Requirements

### Requirement: El mensaje de sistema SHALL incluir una sección temporal en cada turno

En **cada** construcción de la petición, el mensaje de sistema SHALL cerrar con una sección temporal compuesta en código (nunca desde un placeholder de `settings.system_prompt`) con el instante del turno formateado como `YYYY-MM-DD HH:MM:SS` seguido del día de la semana entre paréntesis, precedido de la etiqueta `Fecha y hora actual:` y seguido de la instrucción de evaluar «hoy», «ayer» y «mañana» respecto a ese timestamp. La sección SHALL estar presente aunque no haya `BrowserContext`, y un fallo al resolver el instante o la zona SHALL caer a un valor seguro (UTC) sin abortar ni omitir la sección.

**Given** cualquier turno
**When** el orquestador construye la petición
**Then** el mensaje de sistema SHALL terminar con la sección temporal
**And** la sección SHALL contener la etiqueta, el instante formateado con el día de la semana y la instrucción de evaluación relativa

#### Scenario: Formato de la sección temporal
**Given** un instante del turno y una zona conocidos
**When** se compone la sección temporal
**Then** el texto es `Fecha y hora actual: 2026-10-09 19:00:00 (viernes). Evalúa 'hoy', 'ayer' y 'mañana' respecto a este timestamp.`

#### Scenario: Presente aunque no haya navegador
**Given** una invocación sin `BrowserContext`
**When** se construye la petición
**Then** la sección temporal sigue presente y cierra el mensaje

### Requirement: La sección de ubicación SHALL llevar solo la ubicación

Cuando haya `BrowserContext` **con coordenadas**, la sección de ubicación SHALL contener únicamente `Ubicación: {nombre} ({lat}, {lon}).` cuando se resuelva el nombre, o `Coordenadas: ({lat}, {lon}).` cuando no, y SHALL NOT contener ninguna fecha ni hora. Cuando no haya coordenadas —haya o no `BrowserContext`— la sección SHALL omitirse por completo, sin dejar rastro.

**Given** una invocación con o sin `BrowserContext`
**When** el orquestador construye la petición
**Then** SHALL añadir la sección de ubicación solo si hay coordenadas
**And** SHALL omitirla por completo si no las hay

#### Scenario: Con coordenadas la sección lleva solo la ubicación
**Given** un `BrowserContext` con coordenadas (y, opcionalmente, nombre de lugar)
**When** se construye la petición
**Then** la sección de ubicación contiene la ubicación o las coordenadas
**And** NO contiene ninguna fecha ni hora

#### Scenario: Sin coordenadas no hay sección de ubicación
**Given** un `BrowserContext` con marca de tiempo y zona pero sin coordenadas
**When** se construye la petición
**Then** el mensaje de sistema NO contiene ninguna sección de ubicación

### Requirement: Los mensajes conversacionales SHALL llevar su marca de tiempo

Cada mensaje `user` y `assistant` incluido en la petición SHALL ir prefijado con su marca de tiempo en la zona efectiva del turno, con el formato `[YYYY-MM-DD HH:MM]` y un espacio antes del contenido. Para los mensajes del historial la marca SHALL derivarse de su `created_at`; para el mensaje del turno actual, del instante efectivo del turno. Los mensajes `role:"tool"`, los de `role:"system"` y los de contenido vacío o solo espacios SHALL NOT prefijarse. La marca SHALL NOT persistirse en la base de datos —SHALL existir únicamente en la petición al LLM—, de modo que el contenido almacenado y el mostrado por la interfaz queden intactos. Una `created_at` ilegible SHALL dejar el mensaje sin prefijo, sin romper la petición.

**Given** un historial de conversación y un mensaje de turno
**When** el orquestador construye la petición
**Then** cada mensaje `user`/`assistant` con contenido SHALL llevar delante `[YYYY-MM-DD HH:MM]`
**And** los mensajes `tool` y los vacíos quedan sin prefijo
**And** el contenido persistido en la base de datos NO cambia

#### Scenario: El historial va prefijado
**Given** un mensaje `user` del historial con `created_at` y un mensaje `assistant`
**When** se construye la petición
**Then** ambos empiezan por `[YYYY-MM-DD HH:MM] ` en la zona del turno

#### Scenario: Las herramientas no se prefijan
**Given** un mensaje `role:"tool"` del historial
**When** se construye la petición
**Then** su contenido viaja sin marca de tiempo

#### Scenario: El mensaje del turno va prefijado
**Given** un mensaje del usuario y un instante efectivo de turno
**When** se construye la petición
**Then** el mensaje del turno empieza por `[YYYY-MM-DD HH:MM] ` con el instante efectivo

#### Scenario: Una fecha ilegible no rompe
**Given** un mensaje del historial con `created_at` no parseable
**When** se construye la petición
**Then** ese mensaje viaja sin prefijo
**And** la petición se construye con normalidad

### Requirement: El instante y la zona horaria del turno SHALL resolverse una vez por turno

El instante efectivo del turno SHALL ser la marca de tiempo del `BrowserContext` cuando esté presente y sea parseable, y `Utc::now()` en caso contrario. La zona efectiva SHALL ser la zona del `BrowserContext` cuando esté presente y no vacía, si no `settings.timezone`, y si no `Europe/Madrid`; una zona inválida SHALL caer a UTC. Ambos SHALL resolverse una sola vez por turno y SHALL ser la misma fuente para la sección temporal y para las marcas del historial y del turno.

**Given** un turno con o sin `BrowserContext`
**When** el orquestador construye la petición
**Then** el instante y la zona efectivos quedan fijados
**And** la sección temporal y todas las marcas de tiempo usan esa misma zona

#### Scenario: El navegador manda el instante y la zona
**Given** un `BrowserContext` con timestamp y zona válidos
**When** se construye la petición
**Then** la sección temporal y las marcas se calculan con ese instante y esa zona

#### Scenario: Sin navegador, se usa el reloj del sistema y la zona guardada
**Given** una invocación sin `BrowserContext` y `settings.timezone = "Europe/Madrid"`
**When** se construye la petición
**Then** el instante es el actual del sistema
**And** la zona usada es `Europe/Madrid`

#### Scenario: Zona inválida cae a UTC
**Given** una zona efectiva inválida
**When** se formatea la marca de tiempo
**Then** se usa UTC sin abortar
