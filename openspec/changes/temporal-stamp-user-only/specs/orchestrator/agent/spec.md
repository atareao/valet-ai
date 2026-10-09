## MODIFIED Requirements

### Requirement: Los mensajes conversacionales SHALL llevar su marca de tiempo

Cada mensaje `user` incluido en la petición SHALL ir prefijado con su marca de tiempo en la zona efectiva del turno, con el formato `[YYYY-MM-DD HH:MM]` y un espacio antes del contenido. Para los mensajes del historial la marca SHALL derivarse de su `created_at`; para el mensaje del turno actual, del instante efectivo del turno. Los mensajes `assistant` SHALL NOT prefijarse: el modelo imita el formato de sus propios turnos y lo reproduciría en sus respuestas. Al montar el historial, una marca `[YYYY-MM-DD HH:MM]` inicial heredada en el contenido de un `assistant` SHALL retirarse antes de enviarlo al LLM, de modo que el patrón no vuelva a primar al modelo aunque la fila almacenada la conserve; esa retirada SHALL NOT modificar la base de datos. Los mensajes `role:"tool"`, los de `role:"system"` y los de contenido vacío o solo espacios SHALL NOT prefijarse. La marca SHALL NOT persistirse en la base de datos —SHALL existir únicamente en la petición al LLM—, de modo que el contenido almacenado y el mostrado por la interfaz queden intactos. Una `created_at` ilegible SHALL dejar el mensaje sin prefijo, sin romper la petición.

**Given** un historial de conversación y un mensaje de turno
**When** el orquestador construye la petición
**Then** cada mensaje `user` con contenido SHALL llevar delante `[YYYY-MM-DD HH:MM]`
**And** los mensajes `assistant`, `tool` y los vacíos quedan sin prefijo
**And** el contenido persistido en la base de datos NO cambia

#### Scenario: El historial va prefijado
**Given** un mensaje `user` del historial con `created_at`
**When** se construye la petición
**Then** empieza por `[YYYY-MM-DD HH:MM] ` en la zona del turno

#### Scenario: Las respuestas del asistente viajan sin marca
**Given** un mensaje `assistant` del historial
**When** se construye la petición
**Then** su contenido viaja sin marca de tiempo

#### Scenario: Una marca heredada del asistente se retira
**Given** un mensaje `assistant` cuyo contenido almacenado empieza por `[YYYY-MM-DD HH:MM] `
**When** se construye la petición
**Then** ese prefijo heredado NO aparece en el contenido enviado al LLM
**And** el contenido almacenado en la base de datos NO se modifica

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

### Requirement: El mensaje de sistema SHALL incluir una sección temporal en cada turno

En **cada** construcción de la petición, el mensaje de sistema SHALL cerrar con una sección temporal compuesta en código (nunca desde un placeholder de `settings.system_prompt`) con el instante del turno formateado como `YYYY-MM-DD HH:MM:SS` seguido del día de la semana entre paréntesis, precedido de la etiqueta `Fecha y hora actual:` y seguido de la instrucción de evaluar «hoy», «ayer» y «mañana» respecto a ese timestamp. La sección SHALL indicar además que la marca `[YYYY-MM-DD HH:MM]` de los mensajes es metadato y SHALL NOT reproducirse en las respuestas. La sección SHALL estar presente aunque no haya `BrowserContext`, y un fallo al resolver el instante o la zona SHALL caer a un valor seguro (UTC) sin abortar ni omitir la sección.

**Given** cualquier turno
**When** el orquestador construye la petición
**Then** el mensaje de sistema SHALL terminar con la sección temporal
**And** la sección SHALL contener la etiqueta, el instante formateado con el día de la semana y la instrucción de evaluación relativa
**And** SHALL indicar que la marca de tiempo de los mensajes no debe reproducirse

#### Scenario: Formato de la sección temporal
**Given** un instante del turno y una zona conocidos
**When** se compone la sección temporal
**Then** el texto es `Fecha y hora actual: 2026-10-09 19:00:00 (viernes). Evalúa 'hoy', 'ayer' y 'mañana' respecto a este timestamp. No reproduzcas la marca '[YYYY-MM-DD HH:MM]' de los mensajes en tus respuestas.`

#### Scenario: Presente aunque no haya navegador
**Given** una invocación sin `BrowserContext`
**When** se construye la petición
**Then** la sección temporal sigue presente y cierra el mensaje
